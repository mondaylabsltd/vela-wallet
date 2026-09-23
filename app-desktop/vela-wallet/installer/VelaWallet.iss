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
PrivilegesRequired=admin
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
; Spec 076: the Clear Signer's answer comes back as a navigation to
; `velawallet://sign-result`, because the published signing page carries
; `default-src 'none'` in its hashed bytes and cannot open a socket at all.
; Windows routes a scheme by these keys; without them the browser reports the
; navigation cancelled and the wallet waits out its timeout.
;
; Per-user (HKCU), so no elevation is needed and two accounts on one machine
; keep their own wallet. The same scheme the phones register; it is not
; exclusive, which is why the callback carries a one-time token and the core
; verifies the assertion itself.
Root: HKCU; Subkey: "Software\Classes\velawallet"; ValueType: string; ValueName: ""; ValueData: "URL:Vela Wallet"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\velawallet"; ValueType: string; ValueName: "URL Protocol"; ValueData: ""
Root: HKCU; Subkey: "Software\Classes\velawallet\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\{#MyAppExeName},0"
Root: HKCU; Subkey: "Software\Classes\velawallet\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"" ""%1"""

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

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then begin
    InstallVCRedist();
  end;
end;
