#!/usr/bin/env python3
"""Capture baseline responses for all NY files.

Run inside the Docker container while the server is running:
    python tests/capture_baselines.py
"""

import json
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from helpers import (
    api_post,
    extract_load_snapshot,
    extract_merge_snapshot,
    BASELINES_DIR,
    _brick_pos_key,
)

FILES = [f"_NY{i}" for i in range(1, 11)]
MERGE_PARAMS = {"target_count": 60, "min_border": 10, "seed": 42}


def main():
    BASELINES_DIR.mkdir(parents=True, exist_ok=True)
    total_start = time.time()

    for name in FILES:
        path = f"in/{name}.ai"
        print(f"  {name}: loading...", end="", flush=True)
        t0 = time.time()
        load_resp = api_post("/api/load_pdf", {"path": path, "canvas_height": 900})
        t1 = time.time()
        print(f" {t1 - t0:.1f}s, {len(load_resp['bricks'])} bricks", end="", flush=True)

        load_snap = extract_load_snapshot(load_resp, file_stem=name)
        with open(BASELINES_DIR / f"{name}_load.json", "w") as f:
            json.dump(load_snap, f, indent=2, sort_keys=True)

        # Build UUID -> position-key mapping for merge snapshot
        uuid_to_pos = {b["id"]: _brick_pos_key(b) for b in load_resp["bricks"]}

        print(", merging...", end="", flush=True)
        t2 = time.time()
        merge_resp = api_post("/api/merge", MERGE_PARAMS)
        t3 = time.time()
        print(f" {t3 - t2:.1f}s, {merge_resp['num_pieces']} pieces")

        merge_snap = extract_merge_snapshot(merge_resp, uuid_to_pos=uuid_to_pos)
        with open(BASELINES_DIR / f"{name}_merge.json", "w") as f:
            json.dump(merge_snap, f, indent=2, sort_keys=True)

    print(f"\nDone in {time.time() - total_start:.0f}s. Baselines in {BASELINES_DIR}/")


if __name__ == "__main__":
    main()
