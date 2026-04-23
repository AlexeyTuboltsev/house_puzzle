//! Bezier-aware polygon representation.
//!
//! Stores cubic bezier segments as-is (no tessellation) so that vector
//! operations can run at full AI-source precision. Tessellation / scaling
//! happens only at the final output stage.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Segment {
    /// Straight line from the previous endpoint to `to`.
    Line { to: [f64; 2] },
    /// Cubic bezier from the previous endpoint to `to` via control points.
    Cubic { cp1: [f64; 2], cp2: [f64; 2], to: [f64; 2] },
}

impl Segment {
    pub fn end(&self) -> [f64; 2] {
        match *self {
            Segment::Line { to } | Segment::Cubic { to, .. } => to,
        }
    }
}

/// A closed bezier polygon. The ring is closed implicitly: the last segment's
/// `to` is expected to equal `start` (or be close to it within endpoint snap
/// tolerance).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BezierPath {
    pub start: [f64; 2],
    pub segments: Vec<Segment>,
}

impl BezierPath {
    /// All segment endpoints in order: `[start, seg0.to, seg1.to, ...]`.
    pub fn vertices(&self) -> Vec<[f64; 2]> {
        let mut v = Vec::with_capacity(self.segments.len() + 1);
        v.push(self.start);
        for seg in &self.segments {
            v.push(seg.end());
        }
        v
    }

    /// Flatten the ring to a closed polyline. Cubics are sampled at
    /// `samples_per_curve` intermediate t values (1/N .. (N-1)/N plus the endpoint).
    pub fn tessellate(&self, samples_per_curve: usize) -> Vec<[f64; 2]> {
        let mut out = Vec::with_capacity(self.segments.len() * samples_per_curve + 1);
        out.push(self.start);
        let mut prev = self.start;
        for seg in &self.segments {
            match *seg {
                Segment::Line { to } => {
                    out.push(to);
                    prev = to;
                }
                Segment::Cubic { cp1, cp2, to } => {
                    for i in 1..=samples_per_curve {
                        let t = i as f64 / samples_per_curve as f64;
                        let u = 1.0 - t;
                        let x = u.powi(3) * prev[0]
                            + 3.0 * u.powi(2) * t * cp1[0]
                            + 3.0 * u * t.powi(2) * cp2[0]
                            + t.powi(3) * to[0];
                        let y = u.powi(3) * prev[1]
                            + 3.0 * u.powi(2) * t * cp1[1]
                            + 3.0 * u * t.powi(2) * cp2[1]
                            + t.powi(3) * to[1];
                        out.push([x, y]);
                    }
                    prev = to;
                }
            }
        }
        out
    }

    /// Apply an affine transform: `p' = (p + offset) * scale`.
    /// Used to go from AI pymu coords to canvas coords.
    pub fn transform(&self, offset: [f64; 2], scale: f64) -> BezierPath {
        let tx = |p: [f64; 2]| [(p[0] + offset[0]) * scale, (p[1] + offset[1]) * scale];
        BezierPath {
            start: tx(self.start),
            segments: self
                .segments
                .iter()
                .map(|s| match *s {
                    Segment::Line { to } => Segment::Line { to: tx(to) },
                    Segment::Cubic { cp1, cp2, to } => Segment::Cubic {
                        cp1: tx(cp1),
                        cp2: tx(cp2),
                        to: tx(to),
                    },
                })
                .collect(),
        }
    }

    /// Emit as an SVG `d` attribute: `M x,y L|C ... Z`.
    pub fn to_svg_d(&self) -> String {
        let mut s = format!("M{:.3},{:.3}", self.start[0], self.start[1]);
        for seg in &self.segments {
            match *seg {
                Segment::Line { to } => s.push_str(&format!(" L{:.3},{:.3}", to[0], to[1])),
                Segment::Cubic { cp1, cp2, to } => s.push_str(&format!(
                    " C{:.3},{:.3} {:.3},{:.3} {:.3},{:.3}",
                    cp1[0], cp1[1], cp2[0], cp2[1], to[0], to[1]
                )),
            }
        }
        s.push_str(" Z");
        s
    }
}
