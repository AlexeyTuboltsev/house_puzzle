# -*- mode: python ; coding: utf-8 -*-
# PyInstaller spec for House Puzzle Editor
#
# Build:  pyinstaller house_puzzle.spec
# Result: dist/house_puzzle/house_puzzle.exe (Windows) or dist/house_puzzle/house_puzzle (Linux)
#
# No external dependencies required (pure Python TIF parsing).

import sys
from pathlib import Path

block_cipher = None

a = Analysis(
    ['server.py'],
    pathex=[],
    binaries=[],
    datas=[
        ('templates', 'templates'),
        ('static', 'static'),
        ('presets', 'presets'),
        ('VERSION', '.'),
    ],
    hiddenimports=[
        'tif_parser',
        'puzzle_engine',
        'numpy',
        'PIL',
        'PIL.Image',
        'PIL.ImageDraw',
    ],
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=[],
    win_no_prefer_redirects=False,
    win_private_assemblies=False,
    cipher=block_cipher,
    noarchive=False,
)

pyz = PYZ(a.pure, a.zipped_data, cipher=block_cipher)

exe = EXE(
    pyz,
    a.scripts,
    [],
    exclude_binaries=True,
    name='house_puzzle',
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,
    console=True,
)

coll = COLLECT(
    exe,
    a.binaries,
    a.zipfiles,
    a.datas,
    strip=False,
    upx=True,
    upx_exclude=[],
    name='house_puzzle',
)
