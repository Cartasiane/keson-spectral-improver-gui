# -*- mode: python ; coding: utf-8 -*-
block_cipher = None

a = Analysis(
    ['whatsmybitrate_cli.py'],
    pathex=[],
    binaries=[],
    datas=[],
    hiddenimports=[
        'librosa',
        'soundfile',
        'audioread',
        'numpy',
        'scipy.signal',
    ],
    hookspath=[],
    hooksconfig={
        'matplotlib': {
            'backends': ['Agg']
        }
    },
    runtime_hooks=[],
    excludes=[
        'tkinter', '_tkinter', 'Tkinter',
        'PyQt5', 'PyQt6', 'PySide2', 'PySide6',
        'wx', 'wxPython',
        'gi', 'gtk',
        'pandas',
        'IPython', 'ipython',
        'notebook', 'jupyter',
        'pytest',
        'matplotlib.backends.backend_qt5agg',
        'matplotlib.backends.backend_qt5',
        'matplotlib.backends.backend_qt4agg',
        'matplotlib.backends.backend_qt4',
        'matplotlib.backends.backend_gtk3agg',
        'matplotlib.backends.backend_gtk3',
        'matplotlib.backends.backend_gtk4agg',
        'matplotlib.backends.backend_wx',
        'matplotlib.backends.backend_wxagg',
        'matplotlib.backends.backend_tkagg',
        'matplotlib.backends.backend_tk',
    ],
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
    name='whatsmybitrate',
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=False,
    exclude_binaries=True,
    upx_exclude=[],
    runtime_tmpdir=None,
    console=False,
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,
    codesign_identity='-',
    entitlements_file='entitlements.plist',
)

coll = COLLECT(
    exe,
    a.binaries,
    a.zipfiles,
    a.datas,
    strip=False,
    upx=False,
    upx_exclude=[],
    name='whatsmybitrate',
)
