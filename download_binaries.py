import os
import urllib.request
import zipfile
import tarfile
import shutil
import platform
import sys

try:
    import py7zr
except ImportError:
    print("Error: py7zr is not installed. Please install it using 'pip install py7zr'")
    sys.exit(1)

# Configuration
BINARIES_DIR = os.path.abspath("src-tauri/binaries")
URLS = {
    "win64": {
        "url": "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip",
        "ext": "zip",
        "target_ffmpeg": "ffmpeg-x86_64-pc-windows-msvc.exe",
        "target_ffprobe": "ffprobe-x86_64-pc-windows-msvc.exe",
        "inner_dir": "ffmpeg-master-latest-win64-gpl/bin"
    },
    "linux64": {
        "url": "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-linux64-gpl.tar.xz",
        "ext": "tar.xz",
        "target_ffmpeg": "ffmpeg-x86_64-unknown-linux-gnu",
        "target_ffprobe": "ffprobe-x86_64-unknown-linux-gnu",
        "inner_dir": "ffmpeg-master-latest-linux64-gpl/bin"
    },
    "macos_intel_ffmpeg": {
        "url": "https://evermeet.cx/ffmpeg/getrelease/zip",
        "ext": "zip",
        "target": "ffmpeg-x86_64-apple-darwin",
        "inner_file": "ffmpeg"
    },
    "macos_intel_ffprobe": {
        "url": "https://evermeet.cx/ffmpeg/ffprobe-122467-gc3d3377fe1.7z",
        "ext": "7z",
        "target": "ffprobe-x86_64-apple-darwin",
        "inner_file": "ffprobe"
    }
}

def download_file(url, dest):
    print(f"Downloading {url}...")
    try:
        urllib.request.urlretrieve(url, dest)
        return True
    except Exception as e:
        print(f"Error downloading {url}: {e}")
        return False

def extract_and_install(key, archive_path, temp_dir):
    print(f"Processing {key}...")
    config = URLS[key]
    
    try:
        if config["ext"] == "7z":
            with py7zr.SevenZipFile(archive_path, 'r') as z:
                z.extractall(path=temp_dir)
                
                # Check directly in temp_dir or recursively
                found = False
                for root, _, files in os.walk(temp_dir):
                    if config["inner_file"] in files:
                        src_path = os.path.join(root, config["inner_file"])
                        dest_path = os.path.join(BINARIES_DIR, config["target"])
                        shutil.copy(src_path, dest_path)
                        os.chmod(dest_path, 0o755)
                        print(f"Installed {config['target']}")
                        found = True
                        break
                if not found:
                    print(f"Error: {config['inner_file']} not found in 7z archive")

        elif config["ext"] == "zip":
            with zipfile.ZipFile(archive_path, 'r') as zf:
                # Handle Windows FFmpeg (nested in inner_dir)
                if "win64" in key:
                    for binary in ["ffmpeg.exe", "ffprobe.exe"]:
                        src_path = f"{config['inner_dir']}/{binary}"
                        dest_name = config["target_ffmpeg"] if "ffmpeg" in binary else config["target_ffprobe"]
                        dest_path = os.path.join(BINARIES_DIR, dest_name)
                        
                        try:
                            with zf.open(src_path) as source, open(dest_path, "wb") as target:
                                shutil.copyfileobj(source, target)
                            print(f"Installed {dest_name}")
                        except KeyError:
                            print(f"Error: {src_path} not found in zip")
                            
                # Handle MacOS FFmpeg (single file at root or slightly different)
                elif "macos" in key:
                    src_path = config["inner_file"]
                    dest_path = os.path.join(BINARIES_DIR, config["target"])
                    try:
                         # MacOS ffmpeg zip from evermeet usually has the binary at root
                        with zf.open(src_path) as source, open(dest_path, "wb") as target:
                            shutil.copyfileobj(source, target)
                        os.chmod(dest_path, 0o755)
                        print(f"Installed {config['target']}")
                    except KeyError:
                        print(f"Error: {src_path} not found in zip")

        elif config["ext"] == "tar.xz":
            with tarfile.open(archive_path, 'r:xz') as tf:
                for binary in ["ffmpeg", "ffprobe"]:
                    src_path = f"{config['inner_dir']}/{binary}"
                    dest_name = config["target_ffmpeg"] if "ffmpeg" in binary else config["target_ffprobe"]
                    dest_path = os.path.join(BINARIES_DIR, dest_name)
                    
                    try:
                        member = tf.getmember(src_path)
                        if member:
                            f = tf.extractfile(member)
                            if f:
                                with open(dest_path, "wb") as target:
                                    shutil.copyfileobj(f, target)
                                os.chmod(dest_path, 0o755)
                                print(f"Installed {dest_name}")
                    except KeyError:
                         print(f"Error: {src_path} not found in tar")

    except Exception as e:
        print(f"Error processing {key}: {e}")

def main():
    if not os.path.exists(BINARIES_DIR):
        os.makedirs(BINARIES_DIR)
        
    temp_dir = "temp_downloads"
    if not os.path.exists(temp_dir):
        os.makedirs(temp_dir)
    
    current_os = platform.system()
    keys_to_process = []
    
    if current_os == "Windows":
        keys_to_process = ["win64"]
    elif current_os == "Linux":
        keys_to_process = ["linux64"]
    elif current_os == "Darwin":
        keys_to_process = ["macos_intel_ffmpeg", "macos_intel_ffprobe"]
    else:
        print(f"Unsupported OS: {current_os}")
        return

    print(f"Detected OS: {current_os}. Processing keys: {keys_to_process}")

    for key in keys_to_process:
        config = URLS[key]
        filename = f"{key}.{config['ext']}"
        filepath = os.path.join(temp_dir, filename)
        
        if download_file(config["url"], filepath):
            extract_and_install(key, filepath, temp_dir)
            
    # Cleanup
    try:
        shutil.rmtree(temp_dir)
    except Exception as e:
        print(f"Warning: Could not remove temp dir: {e}")
        
    print("Done!")

if __name__ == "__main__":
    main()
