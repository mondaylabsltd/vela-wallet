; Windows 10/11 x64 or ARM64 installer for the GPUI desktop application.
; Build it through scripts\build-windows-installer.ps1 so the release binary
; and the Microsoft-signed VC++ Redistributable are supplied consistently.

#ifndef MyAppVersion
  #error MyAppVersion must be supplied by the build script.
#endif
#ifndef MyAppExe
  #error MyAppExe must be supplied by the build script.
#endif
#ifndef MyVCRedist
  #error MyVCRedist must be supplied by the build script.
#endif
#ifndef MyVCRedistName
  #error MyVCRedistName must be supplied by the build script.
#endif
#ifndef MyArchitecture
  #error MyArchitecture must be supplied by the build script.
#endif
#ifndef MyArchitecturesAllowed
  #error MyArchitecturesAllowed must be supplied by the build script.
#endif
#ifndef MyOutputDir
  #error MyOutputDir must be supplied by the build script.
#endif

#define MyAppName "Vela Wallet"
#define MyAppPublisher "Vela Wallet"
#define MyAppExeName "vela-wallet.exe"
; The runtime the bundled redistributable carries, e.g. "14.51.36247.0" — what
; a per-user install compares the machine's runtime against (spec 083 H9).
#define MyVCRedistVersion GetVersionNumbersString(MyVCRedist)

[Setup]
AppId={{6B7B5D7C-E7B7-47FB-94A6-5CCB2216A6CC}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
DefaultDirName={autopf}\Vela Wallet
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
UninstallDisplayIcon={app}\{#MyAppExeName}
OutputDir={#MyOutputDir}
OutputBaseFilename=VelaWallet-Setup-{#MyAppVersion}-{#MyArchitecture}
; The icon for setup.exe itself. Relative to this .iss file. Without it the
; installer ships with Inno Setup's stock icon, which is the first thing a user
; sees of the product. The application's own icon is embedded into
; vela-wallet.exe by build.rs, so the shortcuts and UninstallDisplayIcon below
; pick it up from there.
SetupIconFile=..\packaging\icons\app.getvela.VelaWallet.ico
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed={#MyArchitecturesAllowed}
ArchitecturesInstallIn64BitMode={#MyArchitecturesAllowed}
MinVersion=10.0
; Spec 083 H9: per-machine by default, so an existing install upgrades where it
; is (UsePreviousPrivileges, on by default, keeps a found install's mode without
; asking). A person may choose "Install for me only" in the dialog, or pass
; /CURRENTUSER: no administrator prompt, {autopf} becomes
; %LOCALAPPDATA%\Programs and HKA becomes HKCU. /ALLUSERS forces per-machine.
PrivilegesRequired=admin
PrivilegesRequiredOverridesAllowed=dialog commandline
CloseApplications=yes
SetupLogging=yes

[Tasks]
Name: "desktopicon"; Description: "Create a &desktop shortcut"; GroupDescription: "Additional shortcuts:"

[Files]
Source: "{#MyAppExe}"; DestDir: "{app}"; DestName: "{#MyAppExeName}"; Flags: ignoreversion
Source: "{#MyVCRedist}"; DestDir: "{tmp}"; DestName: "{#MyVCRedistName}"; Flags: deleteafterinstall

[Icons]
Name: "{autoprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; WorkingDir: "{app}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; WorkingDir: "{app}"; Tasks: desktopicon

[Registry]
; Spec 076: the Trusted Signer's answer comes back as a navigation to
; `velawallet://sign-result`, because the published signing page carries
; `default-src 'none'` in its hashed bytes and cannot open a socket at all.
; Windows routes a scheme by these keys; without them the browser reports the
; navigation cancelled and the wallet waits out its timeout.
;
; HKA (spec 083 H9): the install's own root — HKLM for a per-machine install,
; which every account on the machine starts the app from, and HKCU for a
; per-user one. HKCU under an administrator install wrote the key into the
; hive of whoever approved the elevation, which is not always the person
; installing (Inno's "UsedUserAreasWarning"). Keys an older install wrote to
; HKCU point at the same exe and win for that account, so they keep working.
; The same scheme the phones register; it is not exclusive, which is why the
; callback carries a one-time token and the core verifies the assertion itself.
Root: HKA; Subkey: "Software\Classes\velawallet"; ValueType: string; ValueName: ""; ValueData: "URL:Vela Wallet"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\velawallet"; ValueType: string; ValueName: "URL Protocol"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\velawallet\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\{#MyAppExeName},0"
Root: HKA; Subkey: "Software\Classes\velawallet\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"" ""%1"""

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "Launch {#MyAppName}"; Flags: nowait postinstall skipifsilent

[Code]
procedure InstallVCRedist();
var
  ResultCode: Integer;
begin
  if not Exec(
    ExpandConstant('{tmp}\{#MyVCRedistName}'),
    '/install /quiet /norestart',
    '',
    SW_HIDE,
    ewWaitUntilTerminated,
    ResultCode
  ) then begin
    RaiseException('The Microsoft Visual C++ Runtime could not be started.');
  end;

  { 0 = installed, 1638 = a newer version is already present,
    3010 = installed and Windows requests a restart. }
  if (ResultCode <> 0) and (ResultCode <> 1638) and (ResultCode <> 3010) then begin
    RaiseException(
      'The Microsoft Visual C++ Runtime installation failed (exit code ' +
      IntToStr(ResultCode) + ').'
    );
  end;
end;

{ Whether the runtime the redistributable registers under RootKey is the
  bundled one or newer. }
function VCRuntimeIn(RootKey: Integer; Bundled: Int64): Boolean;
var
  Key: String;
  Installed, Major, Minor, Bld, Rbld: Cardinal;
begin
  Result := False;
  Key := 'SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\{#MyArchitecture}';
  if not RegQueryDWordValue(RootKey, Key, 'Installed', Installed) or (Installed <> 1) then
    Exit;
  if RegQueryDWordValue(RootKey, Key, 'Major', Major) and
     RegQueryDWordValue(RootKey, Key, 'Minor', Minor) and
     RegQueryDWordValue(RootKey, Key, 'Bld', Bld) and
     RegQueryDWordValue(RootKey, Key, 'Rbld', Rbld) then
    Result := ComparePackedVersion(PackVersionComponents(Major, Minor, Bld, Rbld), Bundled) >= 0;
end;

{ Spec 083 H9: the redistributable installs for the whole machine, so in a
  per-user install it would raise the administrator prompt the person chose
  to avoid. There it runs only when this machine lacks the runtime or has an
  older one — Windows then asks once, for Microsoft's runtime alone. It lives
  in either registry view depending on the redistributable's build. }
function VCRuntimeCurrent(): Boolean;
var
  Bundled: Int64;
begin
  Result := StrToVersion('{#MyVCRedistVersion}', Bundled) and
    (VCRuntimeIn(HKLM64, Bundled) or VCRuntimeIn(HKLM32, Bundled));
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then begin
    if IsAdminInstallMode() or not VCRuntimeCurrent() then
      InstallVCRedist();
  end;
end;
