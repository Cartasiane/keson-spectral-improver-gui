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
        # GUI toolkits (not needed with Agg backend)
        'tkinter', '_tkinter', 'Tkinter',
        'PyQt5', 'PyQt6', 'PySide2', 'PySide6',
        'wx', 'wxPython',
        'gi', 'gtk',
        # Large unused libraries
        'pandas',
        'IPython', 'ipython',
        'notebook', 'jupyter',
        'pytest', 'unittest',
        # Unused scipy submodules
        'scipy.io',
        'scipy.sparse',
        'scipy.spatial',
        'scipy.optimize',
        'scipy.integrate',
        'scipy.interpolate',
        'scipy.stats',
        'scipy.cluster',
        'scipy.odr',
        'scipy.misc',
        # Unused matplotlib backends
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
    a.binaries,
    a.zipfiles,
    a.datas,
    [],
    name='whatsmybitrate',
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,
    upx_exclude=[],
    runtime_tmpdir=None,
    console=True,
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
)
