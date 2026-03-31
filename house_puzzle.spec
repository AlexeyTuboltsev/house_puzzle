# -*- mode: python ; coding: utf-8 -*-
# PyInstaller spec for House Puzzle Editor
#
# Build (from project root):
#   Linux/macOS:  pyinstaller house_puzzle.spec --noconfirm
#   Windows:      pyinstaller house_puzzle.spec --noconfirm
#
# Output: dist/house_puzzle/house_puzzle(.exe on Windows)
# The dist/house_puzzle/ folder is the distributable — zip it up.
# Place AI files in dist/house_puzzle/in/ before running.

import sys
from pathlib import Path

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
        # PyMuPDF
        'fitz',
        'fitz._fitz',
        'fitz.table',
        'fitz.utils',
        # Shapely
        'shapely',
        'shapely.geometry',
        'shapely.geometry.polygon',
        'shapely.ops',
        'shapely.prepared',
        # zstandard
        'zstandard',
        # Pillow
        'PIL',
        'PIL.Image',
        'PIL.ImageDraw',
        'PIL.ImageFilter',
        # numpy
        'numpy',
        'numpy.core',
        # flask internals sometimes missed
        'flask',
        'werkzeug',
        'werkzeug.serving',
        'werkzeug.debug',
        'jinja2',
        'jinja2.ext',
    ],
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=[
        'tkinter',
        'matplotlib',
        'scipy',
        'IPython',
        'jupyter',
        'PyQt5',
        'PyQt6',
        'wx',
    ],
    noarchive=False,
)

pyz = PYZ(a.pure, a.zipped_data)

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
