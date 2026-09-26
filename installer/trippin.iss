; Inno Setup script for Trippin. Built by .github/workflows/release.yml, or
; locally:  iscc /DAppVersion=0.1.0 installer\trippin.iss
; Expects a release build in target\release and the shaders\ + dancers\ folders.

#ifndef AppVersion
  #define AppVersion "0.0.0-dev"
#endif

[Setup]
AppId={{6E0C6B58-3E8B-4C1B-9E4F-7A7B2C9D1E21}
AppName=Trippin
AppVersion={#AppVersion}
AppPublisher=jimmyeao
AppComments=Live music-reactive visuals and shadow dancers for DJ sets
DefaultDirName={autopf}\Trippin
DefaultGroupName=Trippin
UninstallDisplayIcon={app}\trippin.exe
SetupIconFile=..\logo.ico
OutputDir=..\dist
OutputBaseFilename=Trippin-Setup-{#AppVersion}
Compression=lzma2/max
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
WizardStyle=modern
PrivilegesRequiredOverridesAllowed=dialog

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "..\target\release\trippin.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\shaders\*"; DestDir: "{app}\shaders"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "..\dancers\*"; DestDir: "{app}\dancers"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion isreadme

[Icons]
Name: "{group}\Trippin"; Filename: "{app}\trippin.exe"; WorkingDir: "{app}"
Name: "{group}\Uninstall Trippin"; Filename: "{uninstallexe}"
Name: "{autodesktop}\Trippin"; Filename: "{app}\trippin.exe"; WorkingDir: "{app}"; Tasks: desktopicon

[Run]
Filename: "{app}\trippin.exe"; Description: "{cm:LaunchProgram,Trippin}"; Flags: nowait postinstall skipifsilent
