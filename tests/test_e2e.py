#!/usr/bin/env python3
"""End-to-end backend tests for house puzzle editor.

Run inside the Docker container while the server is running:
    python -m pytest tests/test_e2e.py -v
Or:
    python tests/test_e2e.py
"""

import json
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from helpers import (
    api_post,
    extract_load_snapshot,
    extract_merge_snapshot,
    compare_load,
    compare_merge,
    BASELINES_DIR,
    _brick_pos_key,
)

FILES = [f"_NY{i}" for i in range(1, 11)]
MERGE_PARAMS = {"target_count": 60, "min_border": 10, "seed": 42}


class TestE2E(unittest.TestCase):
    """Test each NY file against its baseline."""

    def _run_file(self, name):
        load_baseline_path = BASELINES_DIR / f"{name}_load.json"
        merge_baseline_path = BASELINES_DIR / f"{name}_merge.json"

        self.assertTrue(load_baseline_path.exists(), f"Missing baseline: {load_baseline_path}")
        self.assertTrue(merge_baseline_path.exists(), f"Missing baseline: {merge_baseline_path}")

        with open(load_baseline_path) as f:
            load_baseline = json.load(f)
        with open(merge_baseline_path) as f:
            merge_baseline = json.load(f)

        # Load
        load_resp = api_post("/api/load_pdf", {"path": f"in/{name}.ai", "canvas_height": 900})
        load_snap = extract_load_snapshot(load_resp, file_stem=name)
        load_diffs = compare_load(load_snap, load_baseline)
        self.assertEqual(load_diffs, [], f"{name} load diffs:\n" + "\n".join(load_diffs))

        # Build UUID -> position-key mapping for merge comparison
        uuid_to_pos = {b["id"]: _brick_pos_key(b) for b in load_resp["bricks"]}

        # Merge
        merge_resp = api_post("/api/merge", MERGE_PARAMS)
        merge_snap = extract_merge_snapshot(merge_resp, uuid_to_pos=uuid_to_pos)
        merge_diffs = compare_merge(merge_snap, merge_baseline)
        self.assertEqual(merge_diffs, [], f"{name} merge diffs:\n" + "\n".join(merge_diffs))


def _make_test(name):
    def test_method(self):
        self._run_file(name)
    test_method.__name__ = f"test_{name}"
    test_method.__doc__ = f"Test load + merge for {name}.ai"
    return test_method


# Generate one test method per file
for _name in FILES:
    setattr(TestE2E, f"test_{_name}", _make_test(_name))


if __name__ == "__main__":
    unittest.main()
