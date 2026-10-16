; Luna's Windows installer. Built by `cargo xtask installer`, which passes:
;   AppVersion  workspace version     SourceDir  the packaged dist\Luna folder
;   OutputDir   where the setup goes  RepoDir    the repository root

#ifndef AppVersion
  #error Build with `cargo xtask installer`
#endif

[Setup]
AppId={{1CD08BD5-ECEF-41A7-94A3-7267F9CB698D}
AppName=Luna
AppVersion={#AppVersion}
AppVerName=Luna {#AppVersion}
AppPublisher=Shaked Lipszyc
AppPublisherURL=https://shaked.co
DefaultDirName={autopf}\Luna
DisableProgramGroupPage=yes
LicenseFile={#SourceDir}\LICENSES.txt
OutputDir={#OutputDir}
OutputBaseFilename=Luna-{#AppVersion}-windows-x64-setup
SetupIconFile={#RepoDir}\Assets\luna-logo.ico
UninstallDisplayIcon={app}\luna.ico
UninstallDisplayName=Luna
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
PrivilegesRequiredOverridesAllowed=dialog
Compression=lzma2/max
SolidCompression=yes
; Luna is dark: the dark wizard, Luna's sky and Moon on the welcome and
; finish pages, the logo on the others (art/make.py; one image per DPI).
WizardStyle=modern dark
WizardImageFile={#RepoDir}\packaging\windows\art\wizard-164x314.png,{#RepoDir}\packaging\windows\art\wizard-205x393.png,{#RepoDir}\packaging\windows\art\wizard-246x471.png,{#RepoDir}\packaging\windows\art\wizard-287x550.png,{#RepoDir}\packaging\windows\art\wizard-328x628.png,{#RepoDir}\packaging\windows\art\wizard-369x707.png,{#RepoDir}\packaging\windows\art\wizard-410x785.png
WizardSmallImageFile={#RepoDir}\packaging\windows\art\small-58x58.png,{#RepoDir}\packaging\windows\art\small-73x73.png,{#RepoDir}\packaging\windows\art\small-87x87.png,{#RepoDir}\packaging\windows\art\small-102x102.png,{#RepoDir}\packaging\windows\art\small-116x116.png,{#RepoDir}\packaging\windows\art\small-131x131.png,{#RepoDir}\packaging\windows\art\small-145x145.png
WizardImageBackColor=#0c0f14
; Updates run this installer silently (WinSparkle); close a running Luna.
CloseApplications=force

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#SourceDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs
Source: "{#RepoDir}\Assets\luna-logo.ico"; DestDir: "{app}"; DestName: "luna.ico"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Luna"; Filename: "{app}\Luna.exe"; IconFilename: "{app}\luna.ico"
Name: "{autodesktop}\Luna"; Filename: "{app}\Luna.exe"; IconFilename: "{app}\luna.ico"; Tasks: desktopicon

[Run]
Filename: "{app}\Luna.exe"; Description: "{cm:LaunchProgram,Luna}"; Flags: nowait postinstall skipifsilent
; After a silent update, reopen Luna as the user, not elevated (an elevated
; window can't take drag-and-drop from Explorer).
Filename: "{app}\Luna.exe"; Flags: nowait skipifnotsilent runasoriginaluser
