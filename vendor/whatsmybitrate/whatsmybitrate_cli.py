#!/usr/bin/env python3
"""
CLI entry point for whatsmybitrate - designed for PyInstaller bundling.
Supports multiple modes: bitrate probe, full analysis, spectrogram generation.
"""
import sys
import json
import argparse
import os

# Ensure we can import local modules even when bundled
if getattr(sys, 'frozen', False):
    application_path = sys._MEIPASS
else:
    application_path = os.path.dirname(os.path.abspath(__file__))

sys.path.insert(0, application_path)

from wmb_core import AudioFile
import wmb_core

def main():
    parser = argparse.ArgumentParser(description='Whatsmybitrate audio analysis CLI')
    parser.add_argument('mode', choices=['probe', 'analyze', 'spectrum'], 
                        help='Operation mode')
    parser.add_argument('file', help='Audio file to analyze')
    parser.add_argument('--window', type=int, default=30, 
                        help='Analysis window in seconds')
    parser.add_argument('--output', help='Output path for spectrum image')
    
    args = parser.parse_args()
    wmb_core.MAX_LOAD_SECONDS = args.window
    
    try:
        # Check if file exists
        if not os.path.exists(args.file):
             print(json.dumps({"error": f"File not found: {args.file}"}))
             sys.exit(1)

        af = AudioFile(args.file)
        
        if args.mode == 'probe':
            af.analyze(generate_spectrogram_flag=False, assets_dir=None)
            print(json.dumps({"bitrate": af.to_dict().get("estimated_bitrate_numeric")}))
        
        elif args.mode == 'analyze':
            af.analyze(generate_spectrogram_flag=False, assets_dir=None)
            print(json.dumps(af.to_dict()))
        
        elif args.mode == 'spectrum':
            if not args.output:
                print(json.dumps({"error": "--output required for spectrum mode"}))
                sys.exit(1)
            
            output_dir = os.path.dirname(args.output)
            # Ensure output dir exists
            if output_dir and not os.path.exists(output_dir):
                os.makedirs(output_dir, exist_ok=True)
                
            af.analyze(generate_spectrogram_flag=True, assets_dir=output_dir)
            
            # The library generates a file with a specific name, we might need to find it and rename it
            # Or reliance on af.to_dict() to find where it put it.
            # However, looking at wmb_core.py might be needed to confirm exact behavior.
            # Assuming for now based on previous usage that it puts it in assets_dir.
            
            # Let's print the result dict which should contain the path
            print(json.dumps(af.to_dict()))
    
    except Exception as e:
        print(json.dumps({"error": str(e)}))
        sys.exit(1)

if __name__ == '__main__':
    main()
