; SPDX-License-Identifier: MPL-2.0
#ifndef PackageRoot
  #error PackageRoot is required
#endif
#ifndef Revision
  #error Revision is required
#endif
[Setup]
AppId={{F9158F91-428D-4DA0-BDD1-9A81C2F7F4E0}
AppName=AmberL Preview
AppVersion=0.1.0-dev.{#Copy(Revision,1,8)}
AppPublisher=Project Amber
AppPublisherURL=https://github.com/ayr1k/AmberL
AppSupportURL=https://github.com/ayr1k/AmberL/issues
AppUpdatesURL=https://github.com/ayr1k/AmberL/pull/1
DefaultDirName={localappdata}\Programs\AmberL Preview
DefaultGroupName=AmberL Preview
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.17763
OutputDir={#OutputRoot}
OutputBaseFilename=AmberL-test-{#Copy(Revision,1,8)}-windows-x64
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
CloseApplications=no
RestartApplications=no
UninstallDisplayIcon={app}\amberl.exe
VersionInfoVersion=0.1.0.0
VersionInfoDescription=AmberL development test installer
SetupLogging=yes
DisableProgramGroupPage=yes
[Files]
Source: "{#PackageRoot}\launcher\amberl.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PackageRoot}\licenses\*"; DestDir: "{app}\licenses"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "{#PackageRoot}\AmberL-source.zip"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PackageRoot}\TEST-REVISION.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PackageRoot}\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PackageRoot}\THIRD_PARTY_NOTICES.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PackageRoot}\FIRST-VERSION.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PackageRoot}\INSTALLER.md"; DestDir: "{app}"; Flags: ignoreversion
[Tasks]
Name: desktopicon; Description: "Create a desktop shortcut"; Flags: unchecked
[Icons]
Name: "{group}\AmberL Preview"; Filename: "{app}\amberl.exe"
Name: "{autodesktop}\AmberL Preview"; Filename: "{app}\amberl.exe"; Tasks: desktopicon
[Run]
Filename: "{app}\amberl.exe"; Description: "Open AmberL Preview"; Flags: nowait postinstall skipifsilent
; Deliberately no UninstallDelete section: account/game/server/runtime data is retained.
