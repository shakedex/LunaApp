# Luna Build Script for Velopack
# Usage: .\build.ps1 [-Configuration Release] [-Version 1.0.0] [-Runtime win-x64]
# Supported runtimes: win-x64, osx-x64, osx-arm64, linux-x64

param(
    [string]$Configuration = "Release",
    [string]$Version = "1.0.0",
    [string]$Runtime = "",
    [string]$OutputDir = ".\publish",
    [switch]$All
)

$ErrorActionPreference = "Stop"

# Define supported runtimes
$Runtimes = @("win-x64", "osx-x64", "osx-arm64", "linux-x64")

# If -All flag is set, build all platforms
if ($All) {
    $RuntimesToBuild = $Runtimes
} elseif ($Runtime -eq "") {
    # Auto-detect current platform
    if ($IsWindows -or $env:OS -eq "Windows_NT") {
        $RuntimesToBuild = @("win-x64")
    } elseif ($IsMacOS) {
        $arch = uname -m
        if ($arch -eq "arm64") {
            $RuntimesToBuild = @("osx-arm64")
        } else {
            $RuntimesToBuild = @("osx-x64")
        }
    } else {
        $RuntimesToBuild = @("linux-x64")
    }
} else {
    $RuntimesToBuild = @($Runtime)
}

Write-Host "=== Luna Build Script ===" -ForegroundColor Cyan
Write-Host "Configuration: $Configuration"
Write-Host "Version: $Version"
Write-Host "Runtimes: $($RuntimesToBuild -join ', ')"
Write-Host "Output: $OutputDir"
Write-Host ""

# Clean previous builds
Write-Host "Cleaning previous builds..." -ForegroundColor Yellow
if (Test-Path $OutputDir) {
    Remove-Item -Recurse -Force $OutputDir
}

# Restore dependencies
Write-Host "Restoring dependencies..." -ForegroundColor Yellow
dotnet restore

# Build the application
Write-Host "Building application..." -ForegroundColor Yellow
dotnet build -c $Configuration /p:Version=$Version

foreach ($rid in $RuntimesToBuild) {
    Write-Host ""
    Write-Host "=== Building for $rid ===" -ForegroundColor Cyan
    
    $publishPath = "$OutputDir\$rid"
    
    # Publish self-contained
    Write-Host "Publishing for $rid..." -ForegroundColor Yellow
    dotnet publish -c $Configuration -r $rid --self-contained -o $publishPath /p:Version=$Version /p:PublishSingleFile=false
    
    # Determine main executable name
    if ($rid.StartsWith("win")) {
        $mainExe = "Luna.exe"
    } else {
        $mainExe = "Luna"
    }
    
    # Create installer with Velopack (Windows only for now)
    if ($rid.StartsWith("win")) {
        Write-Host "Creating installer with Velopack..." -ForegroundColor Yellow
        $releasesDir = "$OutputDir\releases\$rid"
        
        # Check if vpk is installed
        $vpkInstalled = Get-Command vpk -ErrorAction SilentlyContinue
        if (-not $vpkInstalled) {
            Write-Host "Installing Velopack CLI..." -ForegroundColor Yellow
            dotnet tool install -g vpk
        }
        
        vpk pack `
            --packId "Luna" `
            --packVersion $Version `
            --packDir $publishPath `
            --mainExe $mainExe `
            --outputDir $releasesDir
        
        Write-Host "Installer created at: $releasesDir" -ForegroundColor Green
    } else {
        Write-Host "Skipping installer for $rid (Velopack Windows only)" -ForegroundColor Yellow
        Write-Host "Distributable at: $publishPath" -ForegroundColor Green
    }
}

Write-Host ""
Write-Host "=== Build Complete ===" -ForegroundColor Green

# List created files
foreach ($rid in $RuntimesToBuild) {
    $releaseDir = "$OutputDir\releases\$rid"
    if (Test-Path $releaseDir) {
        Write-Host ""
        Write-Host "Installer files for $rid`:" -ForegroundColor Cyan
        Get-ChildItem $releaseDir | ForEach-Object {
            Write-Host "  - $($_.Name)" -ForegroundColor White
        }
    }
}
