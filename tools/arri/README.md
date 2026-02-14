# ARRI ART CLI Tools

This folder contains the ARRI Reference Tool (ART) CLI binaries for metadata extraction.

## Required Files

### Windows (tools/arri/win-x64/)
- `art-cmd.exe`
- Any required DLLs that ship with ART

### macOS ARM64 (tools/arri/osx-arm64/)
- `art-cmd`
- Any required dylibs that ship with ART

## How to Obtain

The ART CLI binaries can be obtained from ARRI. These are proprietary tools
and must be licensed appropriately for distribution.

## Usage

The Chappie engine automatically detects ARRI clips and uses `art-cmd` to:
1. Export a 1-second segment with metadata
2. Parse the `metadata.json` output
3. Extract camera, lens, and exposure information

## Command Used

```
art-cmd.exe --mode export --input <clip.mxf> --output <temp_dir/> --duration 1 --skip-audio
```
