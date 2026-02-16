# Luna Installation Guide

Luna is a camera report generator for DITs. This guide covers installation on Windows and macOS.

## System Requirements

| Platform | Minimum Version | Architecture |
|----------|-----------------|--------------|
| Windows  | Windows 10 (1809+) | x64 |
| macOS    | macOS 12 Monterey+ | Apple Silicon (M1/M2/M3) |

## Download

Download the latest release from the [GitHub Releases page](https://github.com/shakedex/LunaApp/releases/latest).

| Platform | File |
|----------|------|
| Windows | `Luna-X.X.X-win-x64-Setup.exe` |
| macOS ARM | `Luna-X.X.X-osx-arm64.dmg` |

---

## Windows Installation

### Step 1: Download and Run Installer

1. Download `Luna-X.X.X-win-x64-Setup.exe` from the releases page
2. Double-click the installer to run it

### Step 2: Bypass Windows SmartScreen

Since Luna is not code-signed, Windows SmartScreen will show a warning. This is normal for indie software.

1. When you see **"Windows protected your PC"**, click **"More info"**
2. Click **"Run anyway"**

![SmartScreen Bypass](https://docs.microsoft.com/en-us/windows/security/threat-protection/microsoft-defender-smartscreen/images/smartscreen-unknown-app.png)

### Step 3: Complete Installation

Follow the installation wizard to complete setup. Luna will be installed and a desktop shortcut will be created.

### Enabling Bundled Tools (ARRI ART CLI)

Luna bundles the ARRI ART CLI for ARRI camera metadata extraction. To allow Windows to run this tool:

1. Open Luna's installation folder (typically `%LOCALAPPDATA%\Luna`)
2. Navigate to `tools\arri\win-x64\`
3. Right-click `art-cmd.exe` → **Properties**
4. At the bottom, check **"Unblock"** if present
5. Click **Apply** and **OK**

Alternatively, run this in PowerShell as Administrator:
```powershell
Get-ChildItem -Path "$env:LOCALAPPDATA\Luna\tools" -Recurse -Include *.exe,*.dll | Unblock-File
```

---

## macOS Installation (Apple Silicon)

### Step 1: Download and Mount DMG

1. Download `Luna-X.X.X-osx-arm64.dmg` from the releases page
2. Double-click to mount the disk image
3. Drag **Luna** to the **Applications** folder

### Step 2: Bypass macOS Gatekeeper

Since Luna is not notarized with Apple, macOS Gatekeeper will block the first launch.

**Method 1: Right-Click Open (Recommended)**
1. Open **Finder** → **Applications**
2. **Right-click** (or Control-click) on **Luna**
3. Select **"Open"** from the context menu
4. In the dialog, click **"Open"**

**Method 2: System Preferences**
1. Try opening Luna normally (it will be blocked)
2. Go to **System Preferences** → **Privacy & Security**
3. Scroll down and click **"Open Anyway"** next to the Luna message

### Step 3: Enable Bundled Tools (Critical)

Luna bundles ARRI and FFmpeg tools that macOS quarantines. **You must run this command** to enable them:

```bash
xattr -cr /Applications/Luna.app
```

This removes the quarantine flag from all bundled executables.

**Verify the tools work:**
```bash
/Applications/Luna.app/Contents/MacOS/tools/arri/osx-arm64/art-cmd --version
```

If you see version output, the tools are working correctly.

---

## Automatic Updates

Luna checks for updates on startup using Velopack. When an update is available:

1. A notification will appear in the app
2. Click to download and install
3. Luna will restart with the new version

Updates are downloaded from [GitHub Releases](https://github.com/shakedex/LunaApp/releases) automatically.

---

## Troubleshooting

### Windows: "The app you're trying to install isn't a Microsoft-verified app"

1. Go to **Settings** → **Apps** → **Apps & features**
2. Under "Choose where to get apps", select **"Anywhere"**

### Windows: ARRI tools fail to run

Run this command in PowerShell to unblock all tools:
```powershell
Get-ChildItem -Path "$env:LOCALAPPDATA\Luna\tools" -Recurse | Unblock-File
```

### macOS: "Luna is damaged and can't be opened"

This means the quarantine flag wasn't removed. Run:
```bash
xattr -cr /Applications/Luna.app
```

### macOS: ARRI tools return "Permission denied"

Ensure the tools have execute permissions:
```bash
chmod +x /Applications/Luna.app/Contents/MacOS/tools/arri/osx-arm64/*
chmod +x /Applications/Luna.app/Contents/MacOS/tools/ffmpeg/osx-arm64/*
```

### App crashes on startup

Check the logs at:
- **Windows:** `%APPDATA%\Luna\logs\`
- **macOS:** `~/Library/Application Support/Luna/logs/`

---

## Uninstallation

### Windows
1. Go to **Settings** → **Apps** → **Apps & features**
2. Find **Luna** and click **Uninstall**

### macOS
1. Drag **Luna** from **Applications** to the **Trash**
2. Optionally remove settings: `rm -rf ~/Library/Application\ Support/Luna`

---

## Building from Source

If you prefer to build Luna yourself:

```bash
git clone https://github.com/shakedex/LunaApp.git
cd LunaApp
dotnet restore
dotnet build -c Release
dotnet run
```

See [build.ps1](../build.ps1) for creating distributable packages.
