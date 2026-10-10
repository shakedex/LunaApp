# Builds Luna and assembles a self-contained folder: dist\Luna\Luna.exe plus the
# libraries it loads at run time. Run from the repo root:
#   powershell -ExecutionPolicy Bypass -File tools\package-windows.ps1
#
# Optional R3D add-on (closed, separate licence): set LUNA_R3D_ADDON to the built
# add-on DLL and LUNA_R3D_SDK_DIR to RED's redistributable folder; both are copied
# into dist\Luna\addons. Without them, R3D clips report "add-on isn't installed".

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$dist = Join-Path $root 'dist\Luna'

cargo build --release -p luna-app
if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }

if (Test-Path $dist) { Remove-Item -Recurse -Force $dist }
New-Item -ItemType Directory -Force $dist | Out-Null
Copy-Item (Join-Path $root 'target\release\luna-app.exe') (Join-Path $dist 'Luna.exe')

# Blackmagic RAW SDK libraries (redistributable under the SDK licence, 1.1(d)).
$braw = $env:BRAW_SDK_DIR
if (-not $braw) { $braw = 'C:\Program Files (x86)\Blackmagic Design\Blackmagic RAW\Blackmagic RAW SDK' }
$brawLibs = Join-Path $braw 'Win\Libraries'
$vendor = Join-Path $dist 'vendor\braw'
New-Item -ItemType Directory -Force $vendor | Out-Null
foreach ($dll in 'BlackmagicRawAPI.dll', 'InstructionSetServicesAVX.dll', 'InstructionSetServicesAVX2.dll') {
    Copy-Item (Join-Path $brawLibs $dll) $vendor
}

if ($env:LUNA_R3D_ADDON -and $env:LUNA_R3D_SDK_DIR) {
    $addons = Join-Path $dist 'addons'
    New-Item -ItemType Directory -Force $addons | Out-Null
    Copy-Item $env:LUNA_R3D_ADDON (Join-Path $addons 'luna_r3d.dll')
    foreach ($dll in 'REDR3D-x64.dll', 'REDDecoder-x64.dll') {
        Copy-Item (Join-Path $env:LUNA_R3D_SDK_DIR $dll) $addons
    }
}

Copy-Item (Join-Path $root 'LICENSE'), (Join-Path $root 'NOTICE'), (Join-Path $root 'DISCLAIMER.md') $dist
Write-Host "Packaged: $dist"
