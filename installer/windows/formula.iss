; Inno Setup script for Formula's Windows installer (Inno Setup 6.3 or newer).
; Normally run by build-installer.ps1, which first builds formula.exe and passes the version:
;   ISCC.exe /DAppVersion=0.0.5 [/DAppArch=x86] formula.iss
; AppArch: x64 (default, 64-bit Windows) or x86 (32-bit, runs on any Windows PC).
; The finished installer is written to the Output folder next to this file.

#ifndef AppVersion
  #define AppVersion "0.0.5"
#endif
#ifndef AppArch
  #define AppArch "x64"
#endif
#define AppName "Formula"
#define AppExe "formula.exe"
#define AppRoot "..\.."

[Setup]
AppId={{5E0C2B7A-6F1D-4C3E-9B8A-2D4F7A1C9E30}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher=Arkadiy Bazhin
AppComments=A desktop editor for chemical structures and reactions
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
; Installs for the current user without asking for administrator rights; the first page
; also offers "Install for all users".
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
#if AppArch == "x64"
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
#endif
MinVersion=10.0
LicenseFile={#AppRoot}\LICENSE
SetupIconFile=formula.ico
UninstallDisplayIcon={app}\{#AppExe}
UninstallDisplayName={#AppName}
OutputDir=Output
#if AppArch == "x64"
OutputBaseFilename=Formula-Setup-{#AppVersion}
#else
OutputBaseFilename=Formula-Setup-{#AppVersion}-{#AppArch}
#endif
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
ChangesAssociations=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "associate"; Description: "Open chemistry files (.cdx, .cdxml, .mol, .sdf, .rxn, .cml, .smi) with {#AppName}"; GroupDescription: "File types:"; Flags: unchecked

[Files]
Source: "{#AppRoot}\target\release\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "formula.ico"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#AppRoot}\README.md"; DestDir: "{app}"; DestName: "README.txt"; Flags: ignoreversion
Source: "{#AppRoot}\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion
Source: "{#AppRoot}\THIRD-PARTY.md"; DestDir: "{app}"; DestName: "THIRD-PARTY.txt"; Flags: ignoreversion

[Icons]
Name: "{group}\{#AppName}"; Filename: "{app}\{#AppExe}"; IconFilename: "{app}\formula.ico"
Name: "{group}\Uninstall {#AppName}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; IconFilename: "{app}\formula.ico"; Tasks: desktopicon

[Registry]
; One file type for all supported formats, used only when the "File types" box is ticked.
Root: HKA; Subkey: "Software\Classes\Formula.Document"; ValueType: string; ValueName: ""; ValueData: "Chemical structure"; Flags: uninsdeletekey; Tasks: associate
Root: HKA; Subkey: "Software\Classes\Formula.Document\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\formula.ico,0"; Tasks: associate
Root: HKA; Subkey: "Software\Classes\Formula.Document\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%1"""; Tasks: associate
Root: HKA; Subkey: "Software\Classes\.cdx\OpenWithProgids"; ValueType: string; ValueName: "Formula.Document"; ValueData: ""; Flags: uninsdeletevalue; Tasks: associate
Root: HKA; Subkey: "Software\Classes\.cdxml\OpenWithProgids"; ValueType: string; ValueName: "Formula.Document"; ValueData: ""; Flags: uninsdeletevalue; Tasks: associate
Root: HKA; Subkey: "Software\Classes\.mol\OpenWithProgids"; ValueType: string; ValueName: "Formula.Document"; ValueData: ""; Flags: uninsdeletevalue; Tasks: associate
Root: HKA; Subkey: "Software\Classes\.sdf\OpenWithProgids"; ValueType: string; ValueName: "Formula.Document"; ValueData: ""; Flags: uninsdeletevalue; Tasks: associate
Root: HKA; Subkey: "Software\Classes\.rxn\OpenWithProgids"; ValueType: string; ValueName: "Formula.Document"; ValueData: ""; Flags: uninsdeletevalue; Tasks: associate
Root: HKA; Subkey: "Software\Classes\.cml\OpenWithProgids"; ValueType: string; ValueName: "Formula.Document"; ValueData: ""; Flags: uninsdeletevalue; Tasks: associate
Root: HKA; Subkey: "Software\Classes\.smi\OpenWithProgids"; ValueType: string; ValueName: "Formula.Document"; ValueData: ""; Flags: uninsdeletevalue; Tasks: associate
; Lets "Open with" list Formula for these types even when the box is not ticked.
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}"; ValueType: string; ValueName: "FriendlyAppName"; ValueData: "{#AppName}"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%1"""

[Run]
; Makes "Formula Structure" objects known to Word (Insert > Object, and double-click to edit).
Filename: "{app}\{#AppExe}"; Parameters: "--register"; Flags: runhidden waituntilterminated
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent

[UninstallRun]
Filename: "{app}\{#AppExe}"; Parameters: "--unregister"; Flags: runhidden waituntilterminated; RunOnceId: "UnregisterObjects"
