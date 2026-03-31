#!/usr/bin/env bash
# Build script for macOS (and Linux)
# Run from the project root directory.
set -e

echo "=== House Puzzle Editor — macOS/Linux Build ==="
echo

echo "Installing dependencies..."
pip install -r requirements.txt pyinstaller
echo

echo "Building executable..."
pyinstaller house_puzzle.spec --noconfirm
echo

echo "Done! Output in dist/house_puzzle/"
echo "Run ./dist/house_puzzle/house_puzzle to start."
