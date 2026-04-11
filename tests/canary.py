#!/usr/bin/env python3
"""Cross-platform canary test for house puzzle editor.

Verifies the binary works end-to-end on the current platform:
  1. Start server
  2. Load _NY2.ai with deterministic IDs
  3. Compare JSON structure (brick count, positions, adjacency)
  4. Compare PNG pixel hashes (composite, outlines, brick images)
  5. Generate puzzle (merge)
  6. Compare merge JSON (piece count, assignments)
  7. Compare piece PNG pixel hashes

Usage:
    # Run canary test against baselines
    python tests/canary.py --binary ./hp-server --fixture in/_NY2.ai

    # Capture new baselines for this platform
    python tests/canary.py --binary ./hp-server --fixture in/_NY2.ai --capture

    # Use existing running server (for local dev)
    python tests/canary.py --fixture in/_NY2.ai --url http://localhost:5050

Baselines are stored per-platform in tests/baselines/canary/<platform>.json
Platform is auto-detected: linux-x86_64, macos-arm64, macos-x86_64, windows-x86_64
"""

import argparse
import hashlib
import json
import os
import platform
import signal
import subprocess
import sys
import time
import urllib.request
from pathlib import Path

BASELINES_DIR = Path(__file__).parent / "baselines" / "canary"
LOAD_PARAMS = {"canvas_height": 900, "deterministic_ids": True}
MERGE_PARAMS = {"target_count": 60, "min_border": 10, "seed": 42}
TIMEOUT = 120  # seconds for server startup
REQUEST_TIMEOUT = 600  # seconds for API requests


def detect_platform():
    """Detect platform string for baseline file naming."""
    system = platform.system().lower()
    machine = platform.machine().lower()
    if system == "darwin":
        if machine == "arm64":
            return "macos-arm64"
        return "macos-x86_64"
    elif system == "windows":
        return "windows-x86_64"
    else:
        return "linux-x86_64"


def api_post(base_url, path, data):
    """POST JSON to server, return parsed response."""
    req = urllib.request.Request(
        f"{base_url}{path}",
        data=json.dumps(data).encode(),
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=REQUEST_TIMEOUT) as resp:
        return json.loads(resp.read())


def api_get_bytes(base_url, path):
    """GET raw bytes from server."""
    with urllib.request.urlopen(f"{base_url}{path}", timeout=60) as resp:
        return resp.read()


def png_hash(data):
    """SHA256 hash of PNG bytes."""
    return hashlib.sha256(data).hexdigest()


def wait_for_server(base_url, timeout=TIMEOUT):
    """Wait until the server responds to /api/list_pdfs."""
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            urllib.request.urlopen(f"{base_url}/api/list_pdfs", timeout=5)
            return True
        except Exception:
            time.sleep(0.5)
    return False


def start_server(binary_path, fixture_dir):
    """Start the server binary and return the process."""
    env = os.environ.copy()
    # Server looks for 'in/' in cwd — set cwd to fixture parent
    cwd = str(Path(fixture_dir).parent.parent) if "/" in fixture_dir or "\\" in fixture_dir else "."

    # Suppress browser auto-open
    env["BROWSER"] = ""
    env["DISPLAY"] = ""  # Prevent X11 browser open on Linux

    proc = subprocess.Popen(
        [str(binary_path)],
        cwd=cwd,
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    return proc


def stop_server(proc):
    """Stop the server process."""
    if proc is None:
        return
    try:
        if sys.platform == "win32":
            proc.terminate()
        else:
            proc.send_signal(signal.SIGTERM)
        proc.wait(timeout=10)
    except Exception:
        proc.kill()
        proc.wait(timeout=5)


def run_canary(base_url, fixture_path):
    """Run the canary test, return results dict."""
    results = {"platform": detect_platform(), "errors": [], "png_hashes": {}}

    # === LOAD ===
    print("  Loading AI file...")
    try:
        load_resp = api_post(base_url, "/api/load_pdf", {
            "path": str(fixture_path),
            **LOAD_PARAMS,
        })
    except Exception as e:
        results["errors"].append(f"load_pdf failed: {e}")
        return results

    key = load_resp.get("key", "")
    pfx = f"/api/s/{key}"
    results["load"] = {
        "canvas": load_resp["canvas"],
        "num_bricks": load_resp.get("num_bricks", len(load_resp["bricks"])),
        "render_dpi": load_resp.get("render_dpi", 0),
        "houseUnitsHigh": load_resp.get("houseUnitsHigh", 0),
        "brick_ids": sorted(b["id"] for b in load_resp["bricks"]),
        "brick_positions": {b["id"]: [b["x"], b["y"], b["width"], b["height"]]
                           for b in load_resp["bricks"]},
        "brick_neighbors": {b["id"]: sorted(b.get("neighbors", []))
                           for b in load_resp["bricks"]},
    }

    # PNG hashes for load artifacts
    print("  Fetching PNG hashes (composite, outlines, lights, background)...")
    for name in ["composite", "outlines", "lights", "background"]:
        try:
            data = api_get_bytes(base_url, f"{pfx}/{name}.png")
            if len(data) > 100:  # skip transparent 1x1 placeholder
                results["png_hashes"][name] = png_hash(data)
        except Exception:
            pass

    # Brick PNG hashes (sample: first 10 + last 10 by sorted ID)
    print("  Fetching brick PNG hashes (sample)...")
    sorted_ids = sorted(b["id"] for b in load_resp["bricks"])
    sample_ids = sorted_ids[:10] + sorted_ids[-10:]
    sample_ids = sorted(set(sample_ids))
    for bid in sample_ids:
        try:
            data = api_get_bytes(base_url, f"{pfx}/brick/{bid}.png")
            if len(data) > 100:
                results["png_hashes"][f"brick_{bid}"] = png_hash(data)
        except Exception:
            pass

    # === MERGE ===
    print("  Running merge...")
    try:
        merge_resp = api_post(base_url, f"{pfx}/merge", MERGE_PARAMS)
    except Exception as e:
        results["errors"].append(f"merge failed: {e}")
        return results

    results["merge"] = {
        "num_pieces": merge_resp["num_pieces"],
        "piece_ids": sorted(p["id"] for p in merge_resp["pieces"]),
        "piece_bricks": {p["id"]: sorted(p["brick_ids"])
                        for p in merge_resp["pieces"]},
        "piece_positions": {p["id"]: [p["x"], p["y"], p["width"], p["height"]]
                           for p in merge_resp["pieces"]},
    }

    # Piece PNG hashes (sample: first 5 pieces)
    print("  Fetching piece PNG hashes (sample)...")
    piece_ids = sorted(p["id"] for p in merge_resp["pieces"])
    for pid in piece_ids[:5]:
        try:
            data = api_get_bytes(base_url, f"{pfx}/piece/{pid}.png")
            if len(data) > 100:
                results["png_hashes"][f"piece_{pid}"] = png_hash(data)
        except Exception:
            pass
        try:
            data = api_get_bytes(base_url, f"{pfx}/piece_outline/{pid}.png")
            if len(data) > 100:
                results["png_hashes"][f"piece_outline_{pid}"] = png_hash(data)
        except Exception:
            pass

    return results


def compare_results(actual, baseline):
    """Compare actual results against baseline. Return list of diffs."""
    diffs = []

    # Load JSON comparison
    al = actual.get("load", {})
    bl = baseline.get("load", {})
    if al.get("canvas") != bl.get("canvas"):
        diffs.append(f"canvas: {al.get('canvas')} != {bl.get('canvas')}")
    if al.get("num_bricks") != bl.get("num_bricks"):
        diffs.append(f"num_bricks: {al.get('num_bricks')} != {bl.get('num_bricks')}")
    if al.get("render_dpi") != bl.get("render_dpi"):
        diffs.append(f"render_dpi: {al.get('render_dpi')} != {bl.get('render_dpi')}")
    if al.get("brick_ids") != bl.get("brick_ids"):
        a_set = set(al.get("brick_ids", []))
        b_set = set(bl.get("brick_ids", []))
        missing = b_set - a_set
        extra = a_set - b_set
        if missing:
            diffs.append(f"missing bricks: {sorted(missing)}")
        if extra:
            diffs.append(f"extra bricks: {sorted(extra)}")

    # Brick position comparison
    a_pos = al.get("brick_positions", {})
    b_pos = bl.get("brick_positions", {})
    for bid in sorted(set(a_pos) & set(b_pos)):
        if a_pos[bid] != b_pos[bid]:
            diffs.append(f"brick {bid} position: {a_pos[bid]} != {b_pos[bid]}")

    # Brick neighbor comparison
    a_nbr = al.get("brick_neighbors", {})
    b_nbr = bl.get("brick_neighbors", {})
    nbr_diffs = 0
    for bid in sorted(set(a_nbr) & set(b_nbr)):
        if a_nbr[bid] != b_nbr[bid]:
            nbr_diffs += 1
    if nbr_diffs > 0:
        diffs.append(f"{nbr_diffs} bricks have different neighbors")

    # Merge JSON comparison
    am = actual.get("merge", {})
    bm = baseline.get("merge", {})
    if am.get("num_pieces") != bm.get("num_pieces"):
        diffs.append(f"num_pieces: {am.get('num_pieces')} != {bm.get('num_pieces')}")
    if am.get("piece_ids") != bm.get("piece_ids"):
        diffs.append(f"piece_ids differ")
    a_pb = am.get("piece_bricks", {})
    b_pb = bm.get("piece_bricks", {})
    brick_diffs = 0
    for pid in sorted(set(a_pb) & set(b_pb)):
        if a_pb[pid] != b_pb[pid]:
            brick_diffs += 1
    if brick_diffs > 0:
        diffs.append(f"{brick_diffs} pieces have different brick assignments")

    # PNG hash comparison
    a_hashes = actual.get("png_hashes", {})
    b_hashes = baseline.get("png_hashes", {})
    all_keys = sorted(set(list(a_hashes.keys()) + list(b_hashes.keys())))
    png_mismatches = []
    png_missing = []
    for key in all_keys:
        ah = a_hashes.get(key)
        bh = b_hashes.get(key)
        if ah is None:
            png_missing.append(f"{key} (missing in actual)")
        elif bh is None:
            png_missing.append(f"{key} (missing in baseline)")
        elif ah != bh:
            png_mismatches.append(key)
    if png_missing:
        diffs.append(f"PNG missing: {', '.join(png_missing)}")
    if png_mismatches:
        diffs.append(f"PNG pixel mismatch: {', '.join(png_mismatches)}")

    return diffs


def main():
    parser = argparse.ArgumentParser(description="House Puzzle canary test")
    parser.add_argument("--binary", help="Path to hp-server binary")
    parser.add_argument("--fixture", required=True, help="Path to _NY2.ai test fixture")
    parser.add_argument("--url", default="http://localhost:5050", help="Server URL (if already running)")
    parser.add_argument("--capture", action="store_true", help="Capture baselines instead of comparing")
    parser.add_argument("--platform", help="Override platform detection")
    parser.add_argument("--port", type=int, default=5050, help="Port to use")
    args = parser.parse_args()

    plat = args.platform or detect_platform()
    baseline_path = BASELINES_DIR / f"{plat}.json"
    fixture_path = Path(args.fixture).resolve()

    if not fixture_path.exists():
        print(f"ERROR: Fixture not found: {fixture_path}")
        sys.exit(1)

    proc = None
    base_url = args.url

    try:
        # Start server if binary provided
        if args.binary:
            print(f"Starting server: {args.binary}")
            proc = start_server(args.binary, str(fixture_path))
            base_url = f"http://localhost:{args.port}"

            print(f"Waiting for server at {base_url}...")
            if not wait_for_server(base_url):
                stderr = proc.stderr.read().decode() if proc.stderr else ""
                print(f"ERROR: Server failed to start within {TIMEOUT}s")
                if stderr:
                    print(f"Server stderr:\n{stderr[:2000]}")
                sys.exit(1)
            print("Server is ready.")

        # Run canary
        print(f"Running canary test ({plat})...")
        results = run_canary(base_url, fixture_path)

        if results["errors"]:
            print(f"\nERRORS during test:")
            for e in results["errors"]:
                print(f"  - {e}")
            sys.exit(1)

        if args.capture:
            # Save baselines
            BASELINES_DIR.mkdir(parents=True, exist_ok=True)
            with open(baseline_path, "w") as f:
                json.dump(results, f, indent=2, sort_keys=True)
            print(f"\nBaseline captured: {baseline_path}")
            n_hashes = len(results.get("png_hashes", {}))
            n_bricks = results["load"]["num_bricks"]
            n_pieces = results["merge"]["num_pieces"]
            print(f"  {n_bricks} bricks, {n_pieces} pieces, {n_hashes} PNG hashes")
            sys.exit(0)

        # Compare against baseline
        if not baseline_path.exists():
            print(f"\nNo baseline found for {plat} at {baseline_path}")
            print("Auto-capturing baseline for first run.")
            BASELINES_DIR.mkdir(parents=True, exist_ok=True)
            with open(baseline_path, "w") as f:
                json.dump(results, f, indent=2, sort_keys=True)
            n_bricks = results["load"]["num_bricks"]
            n_pieces = results["merge"]["num_pieces"]
            n_hashes = len(results.get("png_hashes", {}))
            print(f"  Captured: {n_bricks} bricks, {n_pieces} pieces, {n_hashes} PNG hashes")
            print("  PASS (first run — baseline created, no comparison)")
            sys.exit(0)

        with open(baseline_path) as f:
            baseline = json.load(f)

        diffs = compare_results(results, baseline)

        if diffs:
            print(f"\nCANARY FAILED — {len(diffs)} differences:")
            for d in diffs:
                print(f"  - {d}")
            sys.exit(1)
        else:
            n_hashes = len(results.get("png_hashes", {}))
            print(f"\nCANARY PASSED — all checks OK ({n_hashes} PNG hashes matched)")
            sys.exit(0)

    finally:
        if proc:
            print("Stopping server...")
            stop_server(proc)
            # Print server output for debugging
            stderr = proc.stderr.read().decode() if proc.stderr else ""
            if stderr:
                print(f"Server log:\n{stderr[:3000]}")


if __name__ == "__main__":
    main()
