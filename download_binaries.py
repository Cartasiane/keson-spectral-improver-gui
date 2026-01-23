import os
import urllib.request
import zipfile
import tarfile
import shutil
import platform
import py7zr

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

def extract_and_install(key, archive_path):
    print(f"Processing {key}...")
    config = URLS[key]
    
    try:
        if config["ext"] == "7z":
            with py7zr.SevenZipFile(archive_path, 'r') as z:
                extract_path = os.path.dirname(archive_path)
                z.extractall(path=extract_path)
                found = False
                for root, dirs, files in os.walk(extract_path):
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

        # Extract specific files
        elif "macos" in key:
             # Simple zip with single file at root
            with zipfile.ZipFile(archive_path, 'r') as zf:
                src_path = config["inner_file"]
                dest_path = os.path.join(BINARIES_DIR, config["target"])
                
                with zf.open(src_path) as source, open(dest_path, "wb") as target:
                    shutil.copyfileobj(source, target)
                os.chmod(dest_path, 0o755)
                print(f"Installed {config['target']}")

        elif config["ext"] == "7z":
            with py7zr.SevenZipFile(archive_path, 'r') as z:
                z.extractall(path=temp_dir)
                # Assuming flat extract or finding file
                # Search for the binary in extracted folder
                found = False
                for root, dirs, files in os.walk(temp_dir):
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
                for binary in ["ffmpeg.exe", "ffprobe.exe"]:
                    src_path = f"{config['inner_dir']}/{binary}"
                    dest_name = config["target_ffmpeg"] if "ffmpeg" in binary else config["target_ffprobe"]
                    dest_path = os.path.join(BINARIES_DIR, dest_name)
                    
                    with zf.open(src_path) as source, open(dest_path, "wb") as target:
                        shutil.copyfileobj(source, target)
                    print(f"Installed {dest_name}")
                    
        elif config["ext"] == "tar.xz":
            with tarfile.open(archive_path, 'r:xz') as tf:
                for binary in ["ffmpeg", "ffprobe"]:
                    src_path = f"{config['inner_dir']}/{binary}"
                    dest_name = config["target_ffmpeg"] if "ffmpeg" in binary else config["target_ffprobe"]
                    dest_path = os.path.join(BINARIES_DIR, dest_name)
                    
                    member = tf.getmember(src_path)
                    if member:
                        f = tf.extractfile(member)
                        with open(dest_path, "wb") as target:
                            shutil.copyfileobj(f, target)
                        # Set executable permissions
                        os.chmod(dest_path, 0o755)
                        print(f"Installed {dest_name}")

    except Exception as e:
        print(f"Error processing {key}: {e}")

def main():
    if not os.path.exists(BINARIES_DIR):
        os.makedirs(BINARIES_DIR)
        
    temp_dir = "temp_downloads"
    if not os.path.exists(temp_dir):
        os.makedirs(temp_dir)
        
    for key, config in URLS.items():
        filename = f"{key}.{config['ext']}"
        filepath = os.path.join(temp_dir, filename)
        
        if download_file(config["url"], filepath):
            extract_and_install(key, filepath)
            
    # Cleanup
    shutil.rmtree(temp_dir)
    print("Done!")

if __name__ == "__main__":
    main()
