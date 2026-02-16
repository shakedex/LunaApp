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
foreach ($rid in $RuntimesToBuild) {
    dotnet restore LunaApp.csproj -r $rid
}

# Build the application
Write-Host "Building application..." -ForegroundColor Yellow
dotnet build LunaApp.csproj -c $Configuration /p:Version=$Version

foreach ($rid in $RuntimesToBuild) {
    Write-Host ""
    Write-Host "=== Building for $rid ===" -ForegroundColor Cyan
    
    $publishPath = "$OutputDir\$rid"
    
    # Publish self-contained with trimming
    Write-Host "Publishing for $rid (self-contained + trimmed)..." -ForegroundColor Yellow
    dotnet publish LunaApp.csproj -c $Configuration -r $rid --self-contained true -o $publishPath /p:Version=$Version /p:PublishTrimmed=true /p:TrimMode=partial /p:PublishSingleFile=false
    
    # Verify runtime is bundled
    $runtimeDll = Join-Path $publishPath "System.Private.CoreLib.dll"
    if (Test-Path $runtimeDll) {
        Write-Host "✓ .NET runtime bundled successfully" -ForegroundColor Green
    } else {
        Write-Error "✗ Runtime NOT bundled - self-contained build failed!"
        exit 1
    }
    
    # Determine main executable name
    if ($rid.StartsWith("win")) {
        $mainExe = "Luna.exe"
    } else {
        $mainExe = "Luna"
    }
    
    # Create installer with Velopack
    if ($rid.StartsWith("win") -or $rid.StartsWith("osx")) {
        Write-Host "Creating installer with Velopack..." -ForegroundColor Yellow
        $releasesDir = "$OutputDir\releases\$rid"
        
        # Check if vpk is installed
        $vpkInstalled = Get-Command vpk -ErrorAction SilentlyContinue
        if (-not $vpkInstalled) {
            Write-Host "Installing Velopack CLI..." -ForegroundColor Yellow
            dotnet tool install -g vpk
        }
        
        # Set executable permissions on macOS before packaging
        if ($rid.StartsWith("osx") -and (-not $IsWindows)) {
            Write-Host "Setting executable permissions..." -ForegroundColor Yellow
            chmod +x "$publishPath/Luna"
            chmod +x "$publishPath/tools/arri/osx-arm64/art-cmd" 2>$null
            chmod +x "$publishPath/tools/arri/osx-arm64/look-builder" 2>$null
            Get-ChildItem "$publishPath/tools/ffmpeg/osx-arm64" -ErrorAction SilentlyContinue | ForEach-Object {
                chmod +x $_.FullName
            }
        }
        
        vpk pack `
            --packId "Luna" `
            --packVersion $Version `
            --packDir $publishPath `
            --mainExe $mainExe `
            --outputDir $releasesDir `
            --packAuthors "Luna" `
            --packTitle "Luna - Camera Report Generator"
        
        Write-Host "Installer created at: $releasesDir" -ForegroundColor Green
    } else {
        Write-Host "Skipping installer for $rid (Linux not supported yet)" -ForegroundColor Yellow
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
