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
#ifndef MyVCRuntimeMin
  #error MyVCRuntimeMin must be supplied by the build script.
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
; MyVCRuntimeMin (spec 083 H9): the oldest Visual C++ runtime the app runs on,
; e.g. "14.51.0.0" — the version of the toolset that linked vela-wallet.exe,
; which the build script reads from the exe itself. Not the bundled
; redistributable's version: that is whatever aka.ms served when the cache was
; filled, and a newer one would send every per-user upgrade to the
; administrator prompt for a runtime the app does not need.

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
{ The Microsoft Visual C++ Runtime, installed for the whole machine. Required:
  a failure stops Setup, as it always has for a per-machine install. Not
  required (a per-user install, spec 083 H9): the files are in place and the
  person chose no administrator, so a refused or failed runtime is a warning
  that says what to do, never a failed install. }
procedure InstallVCRedist(Required: Boolean);
var
  ResultCode: Integer;
  Problem: String;
begin
  Problem := '';
  if not Exec(
    ExpandConstant('{tmp}\{#MyVCRedistName}'),
    '/install /quiet /norestart',
    '',
    SW_HIDE,
    ewWaitUntilTerminated,
    ResultCode
  ) then
    Problem := 'The Microsoft Visual C++ Runtime could not be started.'
  { 0 = installed, 1638 = a newer version is already present,
    3010 = installed and Windows requests a restart. 1602 and 1223 are the
    administrator prompt declined or dismissed. }
  else if (ResultCode <> 0) and (ResultCode <> 1638) and (ResultCode <> 3010) then
    Problem := 'The Microsoft Visual C++ Runtime installation failed (exit code ' +
      IntToStr(ResultCode) + ').';

  if Problem = '' then
    Exit;
  if Required then
    RaiseException(Problem);
  Log(Problem + ' Per-user install: continuing without it.');
  { Not "run Setup again for all users": a later run keeps this install's
    mode without asking. }
  SuppressibleMsgBox(
    '{#MyAppName} is installed, but it needs the Microsoft Visual C++ Runtime ' +
    '{#MyVCRuntimeMin} or newer. That runtime installs for every account on ' +
    'this computer, so it needs an administrator.' + #13#10#13#10 +
    'Ask an administrator to install it from ' +
    'https://aka.ms/vc14/{#MyVCRedistName}' + #13#10#13#10 + Problem,
    mbError, MB_OK, IDOK);
end;

{ Whether the runtime the redistributable registers under RootKey is Minimum
  or newer. }
function VCRuntimeIn(RootKey: Integer; Minimum: Int64): Boolean;
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
    Result := ComparePackedVersion(PackVersionComponents(Major, Minor, Bld, Rbld), Minimum) >= 0;
end;

{ Spec 083 H9: whether this machine has the runtime the app was linked
  against. It lives in either registry view depending on the
  redistributable's build. }
function VCRuntimeCurrent(): Boolean;
var
  Minimum: Int64;
begin
  Result := StrToVersion('{#MyVCRuntimeMin}', Minimum) and
    (VCRuntimeIn(HKLM64, Minimum) or VCRuntimeIn(HKLM32, Minimum));
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep <> ssPostInstall then
    Exit;
  { Per-machine: as always, and a failure fails Setup. }
  if IsAdminInstallMode() then
    InstallVCRedist(True)
  { Per-user (spec 083 H9): the redistributable installs for the whole machine,
    so it would raise the administrator prompt the person chose to avoid. Only
    when the runtime the app needs is missing; and never unattended, where
    nobody is there to answer the prompt and a silent upgrade would wait on it
    and then fail. }
  else if VCRuntimeCurrent() then
    Log('Visual C++ runtime {#MyVCRuntimeMin} or newer present; not installed (per-user).')
  else if WizardSilent() then
    Log('Visual C++ runtime older than {#MyVCRuntimeMin} or missing; not installed: ' +
      'per-user and silent, and it needs an administrator.')
  else
    InstallVCRedist(False);
end;

{ The velawallet:// handler in the current account's hive, before an
  uninstall runs its log (spec 083 H9 review). An install first made by 0.9.5
  or older logged its handler under HKCU, and an upgrade appends to that log;
  uninstalled from an account that has since installed "for me only", that
  entry deleted the other copy's handler, and Trusted Signer answers stopped
  reaching it. A handler that opens another copy is put back once the log has
  run. }
const
  HandlerKey = 'Software\Classes\velawallet';

var
  KeptHandlerName, KeptHandlerIcon, KeptHandlerCommand: String;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then begin
    if RegQueryStringValue(HKCU, HandlerKey + '\shell\open\command', '', KeptHandlerCommand) and
       (Pos(Lowercase(ExpandConstant('{app}\{#MyAppExeName}')), Lowercase(KeptHandlerCommand)) = 0) then begin
      RegQueryStringValue(HKCU, HandlerKey, '', KeptHandlerName);
      RegQueryStringValue(HKCU, HandlerKey + '\DefaultIcon', '', KeptHandlerIcon);
      Log('velawallet:// in this account opens another copy; kept: ' + KeptHandlerCommand);
    end else
      KeptHandlerCommand := '';
  end else if (CurUninstallStep = usPostUninstall) and (KeptHandlerCommand <> '') then begin
    if not RegKeyExists(HKCU, HandlerKey + '\shell\open\command') then begin
      RegWriteStringValue(HKCU, HandlerKey, '', KeptHandlerName);
      RegWriteStringValue(HKCU, HandlerKey, 'URL Protocol', '');
      if KeptHandlerIcon <> '' then
        RegWriteStringValue(HKCU, HandlerKey + '\DefaultIcon', '', KeptHandlerIcon);
      RegWriteStringValue(HKCU, HandlerKey + '\shell\open\command', '', KeptHandlerCommand);
      Log('velawallet:// for the other copy put back.');
    end;
  end;
end;
