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
WizardStyle=modern
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
; After a silent update, reopen Luna.
Filename: "{app}\Luna.exe"; Flags: nowait skipifnotsilent
