; The Windows app's installer, made with Inno Setup 6 by the windows job in
; .github/workflows/apps-release.yml:
;
;   iscc /DVERSION=0.1.1 /DNUMERIC=0.1.1 /DARCH=x64 /DSOURCE=<published app> /DNOTICES=<third-party> /O<out> kaeri-app.iss
;
; ARCH is x64 or arm64. SOURCE is `dotnet publish`'s folder, self-contained: the .NET runtime and
; the Windows App SDK's runtime are inside it, so nothing else is installed first. NOTICES is the
; folder of licences the job gathers. It writes kaeri-<VERSION>-windows-<ARCH>-setup.exe.
;
; It installs for the current user, with no administrator prompt, into
; %LOCALAPPDATA%\Programs\kaeri-app, apart from the terminal app's
; %LOCALAPPDATA%\Programs\kaeri. It adds the Start menu entry, the uninstaller in Settings ›
; Apps, and the kaeri:// links that sign-ins come back by and invites arrive as. Uninstalling
; removes all three and the app's files, and leaves the user's library, history and settings,
; which live in %APPDATA%\kaeri.
;
; The installer is not code-signed, so SmartScreen warns before it runs.

#ifndef VERSION
  #define VERSION "0.0.0"
#endif
#ifndef NUMERIC
  #define NUMERIC VERSION
#endif
#ifndef ARCH
  #define ARCH "x64"
#endif
#ifndef SOURCE
  #error SOURCE names the published app's folder
#endif
#ifndef NOTICES
  #error NOTICES names the folder of third-party licences
#endif

[Setup]
; One id for every version and both architectures, so a new installer replaces the old install.
AppId={{002C5B79-DD92-4D47-B871-D02D4B83AB37}
AppName=Kaeri
AppVersion={#VERSION}
AppVerName=Kaeri {#VERSION}
AppPublisher=kaeri
AppPublisherURL=https://kaeri.app
AppSupportURL=https://github.com/bitknox/kaeri/issues
AppUpdatesURL=https://github.com/bitknox/kaeri/releases
VersionInfoVersion={#NUMERIC}.0
VersionInfoDescription=kaeri installer
UninstallDisplayName=Kaeri app
UninstallDisplayIcon={app}\kaeri.exe
DefaultDirName={autopf}\kaeri-app
DisableProgramGroupPage=yes
DisableDirPage=auto
DisableReadyPage=yes
PrivilegesRequired=lowest
#if ARCH == "arm64"
ArchitecturesAllowed=arm64
ArchitecturesInstallIn64BitMode=arm64
#else
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
#endif
; Windows 10 version 2004, the app's TargetPlatformMinVersion.
MinVersion=10.0.19041
OutputBaseFilename=kaeri-{#VERSION}-windows-{#ARCH}-setup
SetupIconFile=..\..\assets\icon\kaeri.ico
LicenseFile={#NOTICES}\LICENSE-kaeri.txt
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
; A running copy is closed before its files are replaced, and opened again afterwards.
CloseApplications=yes
RestartApplications=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Files]
Source: "{#SOURCE}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "{#NOTICES}\*"; DestDir: "{app}\third-party"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\kaeri"; Filename: "{app}\kaeri.exe"

[Registry]
; kaeri:// opens the app with the link as its argument.
Root: HKA; Subkey: "Software\Classes\kaeri"; ValueType: string; ValueName: ""; ValueData: "URL:kaeri"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\kaeri"; ValueType: string; ValueName: "URL Protocol"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\kaeri\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: """{app}\kaeri.exe"",0"
Root: HKA; Subkey: "Software\Classes\kaeri\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\kaeri.exe"" ""%1"""
; magnet: links and .torrent files open the app, which asks what they are before playing.
Root: HKA; Subkey: "Software\Classes\magnet"; ValueType: string; ValueName: ""; ValueData: "URL:magnet"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\magnet"; ValueType: string; ValueName: "URL Protocol"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\magnet\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\kaeri.exe"" ""%1"""
Root: HKA; Subkey: "Software\Classes\kaeri.torrent"; ValueType: string; ValueName: ""; ValueData: "Torrent file"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\kaeri.torrent\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\kaeri.exe"" ""%1"""
Root: HKA; Subkey: "Software\Classes\.torrent\OpenWithProgids"; ValueType: string; ValueName: "kaeri.torrent"; ValueData: ""; Flags: uninsdeletevalue

[Run]
Filename: "{app}\kaeri.exe"; Description: "Open Kaeri"; Flags: nowait postinstall skipifsilent
