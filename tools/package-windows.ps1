# Builds Luna and assembles a self-contained folder: dist\Luna\Luna.exe plus
# dist\Luna\lib, which holds every library Luna loads at run time (Blackmagic
# RAW, the R3D add-on and RED's library). Run from the repo root:
#   powershell -ExecutionPolicy Bypass -File tools\package-windows.ps1
#
# Inputs (each has a default; any missing input fails the build):
#   BRAW_SDK_DIR    Blackmagic RAW SDK install
#   LUNA_R3D_REPO   the r3d-sdk checkout (default: ..\decoders\r3d-sdk), whose
#                   reference\ folder holds RED's SDK

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$dist = Join-Path $root 'dist\Luna'
$lib = Join-Path $dist 'lib'

function Require($path) {
    if (-not (Test-Path $path)) { throw "Missing: $path" }
    $path
}

$braw = $env:BRAW_SDK_DIR
if (-not $braw) { $braw = 'C:\Program Files (x86)\Blackmagic Design\Blackmagic RAW\Blackmagic RAW SDK' }
$brawLibs = Require (Join-Path $braw 'Win\Libraries')

$r3dRepo = $env:LUNA_R3D_REPO
if (-not $r3dRepo) { $r3dRepo = Join-Path (Split-Path -Parent $root) 'decoders\r3d-sdk' }
$r3dSdk = Require (Join-Path $r3dRepo 'reference')

cargo build --release -p luna-app
if ($LASTEXITCODE -ne 0) { throw 'cargo build (Luna) failed' }

$env:R3D_SDK_DIR = $r3dSdk
cargo build --release --manifest-path (Require (Join-Path $r3dRepo 'Cargo.toml'))
if ($LASTEXITCODE -ne 0) { throw 'cargo build (R3D add-on) failed' }

if (Test-Path $dist) { Remove-Item -Recurse -Force $dist }
New-Item -ItemType Directory -Force $lib | Out-Null
Copy-Item (Require (Join-Path $root 'target\release\luna-app.exe')) (Join-Path $dist 'Luna.exe')

# Blackmagic RAW SDK libraries (redistributable under the SDK licence, 1.1(d)).
foreach ($dll in 'BlackmagicRawAPI.dll', 'InstructionSetServicesAVX.dll', 'InstructionSetServicesAVX2.dll') {
    Copy-Item (Require (Join-Path $brawLibs $dll)) $lib
}

# The R3D add-on (closed, own EULA) and RED's library. CPU decode only: RED's
# GPU libraries (REDDecoder, REDCuda, REDOpenCL) never load.
Copy-Item (Require (Join-Path $r3dRepo 'target\release\r3d.dll')) (Join-Path $lib 'luna_r3d.dll')
Copy-Item (Require (Join-Path $r3dSdk 'Redistributable\win\REDR3D-x64.dll')) $lib

Copy-Item (Join-Path $root 'LICENSE'), (Join-Path $root 'NOTICE'), (Join-Path $root 'DISCLAIMER.md') $dist
Write-Host "Packaged: $dist"
