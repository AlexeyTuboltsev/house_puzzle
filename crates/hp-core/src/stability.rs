//! Gravity-stability validator for assembled houses.
//!
//! Builds a Rapier 2D world, places pieces wave-by-wave as rigid
//! bodies (one cuboid collider per brick — uses the brick's bounding
//! box for v1), steps the simulator long enough to let unstable
//! configurations fall over, and reports which pieces drifted past a
//! displacement threshold.
//!
//! Why this and not analytical static checks: the simple
//! "center-of-mass over support polygon" check works for a single
//! piece in isolation but misses *stack collapse* cases — wave 0
//! places a slightly tilty piece, wave 2 lands a heavy piece on it,
//! the whole stack collapses. PhysX (the Unity engine) catches that
//! by simulating; Rapier is the closest pure-Rust equivalent.
//!
//! This is NOT bit-exact to Unity. Rapier and PhysX have different
//! solvers, contact models, and friction implementations. The intent
//! is to catch the same falls Unity would qualitatively, not to
//! match every borderline jiggle.

use rapier2d::na::Vector2;
use rapier2d::prelude::*;
use std::collections::HashMap;

use crate::types::{Brick, PuzzlePiece};

/// One piece that didn't survive the simulation.
#[derive(Debug, Clone)]
pub struct UnstablePiece {
    pub piece_id: String,
    /// 1-based wave index the piece was placed in. 0 means "unassigned"
    /// (treated as the last wave for sim purposes).
    pub wave: usize,
    /// How far the piece drifted from its target position by the end
    /// of the simulation, in canvas pixels.
    pub displacement: f64,
}

/// Inputs to a stability run.
pub struct StabilityRequest<'a> {
    pub pieces: &'a [PuzzlePiece],
    pub bricks: &'a HashMap<String, Brick>,
    /// Wave order: `waves[i]` = list of piece IDs in wave i+1
    /// (wave 0 in the artist's vocab is the first to be placed).
    pub waves: &'a [Vec<String>],
    pub canvas_width: i32,
    pub canvas_height: i32,
    /// Unassigned pieces (not in any wave) get tacked on at the end
    /// of the sim. Same vec the editor's "unassigned" tray shows.
    pub unassigned_piece_ids: &'a [String],
}

/// Result of a stability run.
pub struct StabilityReport {
    pub unstable: Vec<UnstablePiece>,
    /// Aggregate timing for telemetry.
    pub sim_time_ms: u128,
}

/// How far a piece can drift before we call it "fell". 5 px at 300 DPI
/// is a fraction of a millimetre — comfortably above settling jitter,
/// well below "the piece moved noticeably".
const FALL_THRESHOLD_PX: f32 = 5.0;

/// How long to simulate after each wave is placed, in seconds of
/// simulated time. 1.5 s is enough for an unsupported piece to fall
/// far past the threshold under standard 9.81 m/s² gravity at our
/// pixel-to-metre scale.
const SETTLE_TIME_PER_WAVE_S: f32 = 1.5;

const TIMESTEP_S: f32 = 1.0 / 60.0;

/// Run the stability check. Pieces in earlier waves are placed first
/// and allowed to settle before the next wave drops in. Pieces that
/// drift more than `FALL_THRESHOLD_PX` from their target position by
/// the end are flagged.
pub fn validate_stability(req: &StabilityRequest) -> StabilityReport {
    let t0 = std::time::Instant::now();

    let canvas_h = req.canvas_height as f32;

    // ── world setup ─────────────────────────────────────────────────
    let mut rigid_bodies = RigidBodySet::new();
    let mut colliders = ColliderSet::new();
    let mut impulse_joints = ImpulseJointSet::new();
    let mut multibody_joints = MultibodyJointSet::new();

    let gravity = Vector2::new(0.0, -9.81);
    let integration_parameters = IntegrationParameters {
        dt: TIMESTEP_S,
        ..Default::default()
    };
    let mut physics_pipeline = PhysicsPipeline::new();
    let mut island_manager = IslandManager::new();
    let mut broad_phase = DefaultBroadPhase::new();
    let mut narrow_phase = NarrowPhase::new();
    let mut ccd_solver = CCDSolver::new();
    let mut query_pipeline = QueryPipeline::new();

    // Ground: fixed body, a long horizontal plate just below the
    // canvas bottom. Long enough that pieces near the edges of the
    // canvas don't slip off into the void.
    let ground_handle = rigid_bodies.insert(
        RigidBodyBuilder::fixed()
            .translation(Vector2::new(req.canvas_width as f32 / 2.0, -10.0))
            .build(),
    );
    colliders.insert_with_parent(
        ColliderBuilder::cuboid(req.canvas_width as f32 * 2.0, 10.0)
            .friction(0.8)
            .build(),
        ground_handle,
        &mut rigid_bodies,
    );

    // ── piece bodies ────────────────────────────────────────────────
    // Build one dynamic rigid body per piece. Each brick attaches as
    // a cuboid collider at its brick-bbox dimensions. Initial position
    // is the piece's centroid in Rapier coords (Y-up, ground at 0).
    let pieces_by_id: HashMap<&str, &PuzzlePiece> =
        req.pieces.iter().map(|p| (p.id.as_str(), p)).collect();

    // Centroids are needed twice: once to set the initial position
    // and again to compute displacement at the end.
    let mut piece_handles: HashMap<String, RigidBodyHandle> = HashMap::new();
    let mut piece_initial_pos: HashMap<String, Vector2<f32>> = HashMap::new();

    for piece in req.pieces {
        let center_x = (piece.x as f32) + (piece.width as f32) / 2.0;
        let center_y_canvas = (piece.y as f32) + (piece.height as f32) / 2.0;
        let center_y_rapier = canvas_h - center_y_canvas;
        let initial = Vector2::new(center_x, center_y_rapier);

        // Sleep new bodies — they get woken up when their wave is
        // "released" via the per-wave step below. Without this, every
        // piece would fall the moment we add it; we want them to fall
        // only when their wave starts.
        let body = RigidBodyBuilder::dynamic()
            .translation(initial)
            .sleeping(true)
            .ccd_enabled(false)
            .build();
        let body_handle = rigid_bodies.insert(body);
        piece_handles.insert(piece.id.clone(), body_handle);
        piece_initial_pos.insert(piece.id.clone(), initial);

        // Attach one cuboid collider per brick. Position is given
        // relative to the piece body's centre.
        for brick_id in &piece.brick_ids {
            let brick = match req.bricks.get(brick_id) {
                Some(b) => b,
                None => continue,
            };
            let bw = brick.width.max(1) as f32;
            let bh = brick.height.max(1) as f32;
            let brick_center_x_canvas = brick.x as f32 + bw / 2.0;
            let brick_center_y_canvas = brick.y as f32 + bh / 2.0;
            let brick_center_y_rapier = canvas_h - brick_center_y_canvas;
            // Offset relative to the piece body's origin (the piece's
            // own centroid in world coords).
            let dx = brick_center_x_canvas - center_x;
            let dy = brick_center_y_rapier - center_y_rapier;
            let collider = ColliderBuilder::cuboid(bw / 2.0, bh / 2.0)
                .translation(Vector2::new(dx, dy))
                .friction(0.8)
                .density(1.0)
                .build();
            colliders.insert_with_parent(collider, body_handle, &mut rigid_bodies);
        }
    }

    // ── per-wave release + settle ────────────────────────────────────
    // Process waves in order. For each wave, wake the pieces in that
    // wave so they start being affected by gravity, then step the
    // simulator long enough to let any unstable config fall over.
    let steps_per_wave = (SETTLE_TIME_PER_WAVE_S / TIMESTEP_S) as i32;

    let physics_hooks = ();
    let event_handler = ();

    let mut released: std::collections::HashSet<String> = std::collections::HashSet::new();

    let mut release_and_step = |wave_piece_ids: &[String],
                                 rigid_bodies: &mut RigidBodySet,
                                 colliders: &mut ColliderSet| {
        // Wake the bodies in this wave so gravity starts acting on them.
        for pid in wave_piece_ids {
            if let Some(h) = piece_handles.get(pid) {
                if let Some(body) = rigid_bodies.get_mut(*h) {
                    body.wake_up(true);
                }
                released.insert(pid.clone());
            }
        }
        for _ in 0..steps_per_wave {
            physics_pipeline.step(
                &gravity,
                &integration_parameters,
                &mut island_manager,
                &mut broad_phase,
                &mut narrow_phase,
                rigid_bodies,
                colliders,
                &mut impulse_joints,
                &mut multibody_joints,
                &mut ccd_solver,
                Some(&mut query_pipeline),
                &physics_hooks,
                &event_handler,
            );
        }
    };

    for wave_piece_ids in req.waves {
        release_and_step(wave_piece_ids, &mut rigid_bodies, &mut colliders);
    }
    // Unassigned pieces — release as a "final wave" so the report
    // covers them too.
    if !req.unassigned_piece_ids.is_empty() {
        let unassigned: Vec<String> = req
            .unassigned_piece_ids
            .iter()
            .filter(|id| pieces_by_id.contains_key(id.as_str()))
            .cloned()
            .collect();
        release_and_step(&unassigned, &mut rigid_bodies, &mut colliders);
    }

    // ── displacement check ──────────────────────────────────────────
    let mut unstable = Vec::new();
    for piece in req.pieces {
        let handle = match piece_handles.get(&piece.id) {
            Some(h) => *h,
            None => continue,
        };
        let initial = match piece_initial_pos.get(&piece.id) {
            Some(p) => *p,
            None => continue,
        };
        let body = match rigid_bodies.get(handle) {
            Some(b) => b,
            None => continue,
        };
        let now = *body.translation();
        let displacement = ((now - initial).norm()) as f64;
        if displacement > FALL_THRESHOLD_PX as f64 {
            // Wave index: 1-based for the user; 0 if the piece was
            // unassigned and ran in the synthetic final wave.
            let wave_idx = req
                .waves
                .iter()
                .position(|w| w.contains(&piece.id))
                .map(|i| i + 1)
                .unwrap_or(0);
            unstable.push(UnstablePiece {
                piece_id: piece.id.clone(),
                wave: wave_idx,
                displacement,
            });
        }
    }

    StabilityReport {
        unstable,
        sim_time_ms: t0.elapsed().as_millis(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn brick(id: &str, x: i32, y: i32, w: i32, h: i32) -> Brick {
        Brick {
            id: id.to_string(),
            x,
            y,
            width: w,
            height: h,
            brick_type: "test".to_string(),
        }
    }

    fn piece(id: &str, brick_ids: &[&str], bricks: &HashMap<String, Brick>) -> PuzzlePiece {
        // Derive piece bbox from the bricks it contains.
        let bs: Vec<&Brick> = brick_ids.iter().filter_map(|i| bricks.get(*i)).collect();
        let x = bs.iter().map(|b| b.x).min().unwrap_or(0);
        let y = bs.iter().map(|b| b.y).min().unwrap_or(0);
        let r = bs.iter().map(|b| b.x + b.width).max().unwrap_or(0);
        let b = bs.iter().map(|b| b.y + b.height).max().unwrap_or(0);
        PuzzlePiece {
            id: id.to_string(),
            brick_ids: brick_ids.iter().map(|s| s.to_string()).collect(),
            x,
            y,
            width: r - x,
            height: b - y,
        }
    }

    /// A single brick sitting on the ground should NOT fall.
    #[test]
    fn single_brick_on_ground_stable() {
        let mut bricks = HashMap::new();
        bricks.insert("b1".to_string(), brick("b1", 100, 980, 20, 20)); // y=980, canvas_h=1000 → near ground
        let p = piece("p1", &["b1"], &bricks);

        let waves = vec![vec!["p1".to_string()]];
        let req = StabilityRequest {
            pieces: &[p],
            bricks: &bricks,
            waves: &waves,
            canvas_width: 1000,
            canvas_height: 1000,
            unassigned_piece_ids: &[],
        };
        let report = validate_stability(&req);
        assert!(report.unstable.is_empty(), "expected stable, got {:?}", report.unstable);
    }

    /// A brick floating in mid-air with nothing below it should fall.
    #[test]
    fn floating_brick_falls() {
        let mut bricks = HashMap::new();
        bricks.insert("b1".to_string(), brick("b1", 100, 100, 20, 20)); // far above ground
        let p = piece("p1", &["b1"], &bricks);

        let waves = vec![vec!["p1".to_string()]];
        let req = StabilityRequest {
            pieces: &[p],
            bricks: &bricks,
            waves: &waves,
            canvas_width: 1000,
            canvas_height: 1000,
            unassigned_piece_ids: &[],
        };
        let report = validate_stability(&req);
        assert_eq!(report.unstable.len(), 1, "expected 1 unstable, got {:?}", report.unstable);
        assert_eq!(report.unstable[0].piece_id, "p1");
    }
}
