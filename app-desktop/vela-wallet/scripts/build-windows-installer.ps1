[CmdletBinding()]
param(
    [ValidateSet('x64', 'arm64', 'all')]
    [string]$Architecture = 'x64',

    [switch]$SkipBuild
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if ($Architecture -eq 'all') {
    foreach ($targetArchitecture in @('x64', 'arm64')) {
        Write-Host "Building $targetArchitecture Windows installer..."
        & $PSCommandPath -Architecture $targetArchitecture -SkipBuild:$SkipBuild
        if ($LASTEXITCODE -ne 0) {
            throw "Installer build for $targetArchitecture failed with exit code $LASTEXITCODE."
        }
    }
    return
}

$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$manifestPath = Join-Path $projectRoot 'Cargo.toml'
$manifest = Get-Content -LiteralPath $manifestPath -Raw
if ($manifest -notmatch '(?m)^version\s*=\s*"([^"]+)"') {
    throw "Could not determine the application version from $manifestPath."
}
$appVersion = $Matches[1]

$targetByArchitecture = @{
    x64 = 'x86_64-pc-windows-msvc'
    arm64 = 'aarch64-pc-windows-msvc'
}
$redistByArchitecture = @{
    x64 = @{
        FileName = 'vc_redist.x64.exe'
        Uri = 'https://aka.ms/vc14/vc_redist.x64.exe'
        InstallerArchitecture = 'x64compatible'
    }
    arm64 = @{
        FileName = 'vc_redist.arm64.exe'
        Uri = 'https://aka.ms/vc14/vc_redist.arm64.exe'
        InstallerArchitecture = 'arm64'
    }
}
$targetTriple = $targetByArchitecture[$Architecture]
$redist = $redistByArchitecture[$Architecture]
$releaseExe = if ($Architecture -eq 'x64') {
    Join-Path $projectRoot 'target\release\vela-wallet.exe'
}
else {
    Join-Path $projectRoot ("target\$targetTriple\release\vela-wallet.exe")
}

function Get-Arm64VsDevCmd {
    $vswhereCandidates = @(
        (Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'),
        (Join-Path $env:ProgramFiles 'Microsoft Visual Studio\Installer\vswhere.exe')
    )
    $vswhere = $vswhereCandidates |
        Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } |
        Select-Object -First 1
    if ($null -eq $vswhere) {
        throw 'Visual Studio Installer (vswhere.exe) was not found. Install Visual Studio Build Tools with the ARM64 C++ tools component.'
    }

    $installationPath = @(
        & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.ARM64 -property installationPath
    ) | Select-Object -First 1
    if ([string]::IsNullOrWhiteSpace($installationPath)) {
        throw 'Visual Studio ARM64 C++ tools were not found. In Visual Studio Installer, add "MSVC v143 - VS 2022 C++ ARM64 build tools".'
    }

    $vsDevCmd = Join-Path $installationPath.Trim() 'Common7\Tools\VsDevCmd.bat'
    if (-not (Test-Path -LiteralPath $vsDevCmd -PathType Leaf)) {
        throw "Visual Studio developer command script was not found: $vsDevCmd"
    }

    return $vsDevCmd
}

# The oldest Visual C++ runtime the app runs on: the version of the MSVC
# toolset that linked it, which link.exe writes into the PE optional header
# (MajorLinkerVersion.MinorLinkerVersion, e.g. 14.51). A per-user install
# compares the machine's runtime against this, never against the bundled
# redistributable, whose version is whatever aka.ms served when the cache was
# filled (spec 083 H9 review).
function Get-VCRuntimeMinimum([string]$ExePath) {
    $stream = [System.IO.File]::OpenRead($ExePath)
    try {
        $reader = New-Object System.IO.BinaryReader($stream)
        $stream.Position = 0x3C
        $peOffset = $reader.ReadInt32()
        $stream.Position = $peOffset
        if ($reader.ReadUInt32() -ne 0x00004550) {
            throw "Not a PE image: $ExePath"
        }
        # The optional header follows the 4-byte signature and the 20-byte
        # COFF header; the linker version is its bytes 2 and 3.
        $stream.Position = $peOffset + 24 + 2
        $major = [int]$reader.ReadByte()
        $minor = [int]$reader.ReadByte()
    }
    finally {
        $stream.Dispose()
    }
    # Runtime 14.x serves every toolset since Visual Studio 2015. Anything else
    # is not link.exe's number (lld-link writes its own), and would make the
    # per-user check meaningless.
    if ($major -ne 14) {
        throw "$ExePath was linked by linker version $major.$minor, not an MSVC 14.x link.exe; the installer cannot tell which Visual C++ runtime it needs."
    }
    return [version]::new($major, $minor, 0, 0)
}

if (-not $SkipBuild) {
    Push-Location $projectRoot
    try {
        $installedTargets = @(& rustup target list --installed)
        if ($LASTEXITCODE -ne 0) {
            throw "rustup target list --installed failed with exit code $LASTEXITCODE."
        }
        if ($installedTargets -notcontains $targetTriple) {
            throw "Rust target $targetTriple is not installed. Run: rustup target add $targetTriple"
        }

        if ($Architecture -eq 'arm64') {
            $vsDevCmd = Get-Arm64VsDevCmd
            $buildCommand = 'call "' + $vsDevCmd + '" -no_logo -host_arch=x64 -arch=arm64 >nul && cargo build --release --locked --target ' + $targetTriple
            & cmd.exe /d /s /c $buildCommand
        }
        else {
            & cargo build --release --locked
        }
        if ($LASTEXITCODE -ne 0) {
            throw "cargo build --release for $Architecture failed with exit code $LASTEXITCODE."
        }
    }
    finally {
        Pop-Location
    }
}

if (-not (Test-Path -LiteralPath $releaseExe -PathType Leaf)) {
    throw "Release binary not found: $releaseExe"
}

$outputDir = Join-Path $projectRoot 'dist\windows'
$cacheDir = Join-Path $projectRoot 'dist\.cache'
New-Item -ItemType Directory -Force -Path $outputDir, $cacheDir | Out-Null

$redistPath = Join-Path $cacheDir $redist.FileName
if (-not (Test-Path -LiteralPath $redistPath -PathType Leaf)) {
    Write-Host "Downloading the current Microsoft Visual C++ $Architecture Redistributable..."
    Invoke-WebRequest -Uri $redist.Uri -OutFile $redistPath
}

$signature = Get-AuthenticodeSignature -LiteralPath $redistPath
if (
    $signature.Status -ne 'Valid' -or
    $null -eq $signature.SignerCertificate -or
    $signature.SignerCertificate.Subject -notmatch 'CN=Microsoft Corporation'
) {
    throw "Refusing to package an untrusted VC++ Redistributable: $redistPath"
}

$vcRuntimeMin = Get-VCRuntimeMinimum $releaseExe
$redistVersion = (Get-Item -LiteralPath $redistPath).VersionInfo.FileVersionRaw
if ($redistVersion -lt $vcRuntimeMin) {
    throw "The cached VC++ Redistributable $redistPath is $redistVersion, older than the runtime $vcRuntimeMin the app was linked against. Delete it to download the current one."
}
Write-Host "Visual C++ runtime: needs $vcRuntimeMin or newer; bundling $redistVersion"

$innoCandidates = @(
    (Join-Path $env:LOCALAPPDATA 'Programs\Inno Setup 6\ISCC.exe'),
    (Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6\ISCC.exe'),
    (Join-Path $env:ProgramFiles 'Inno Setup 6\ISCC.exe')
)
$iscc = $innoCandidates | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } | Select-Object -First 1
if ($null -eq $iscc) {
    throw 'Inno Setup 6 was not found. Install it with: winget install --id JRSoftware.InnoSetup --exact'
}

$installerScript = Join-Path $projectRoot 'installer\VelaWallet.iss'
& $iscc `
    "/DMyAppVersion=$appVersion" `
    "/DMyAppExe=$releaseExe" `
    "/DMyVCRedist=$redistPath" `
    "/DMyVCRedistName=$($redist.FileName)" `
    "/DMyVCRuntimeMin=$vcRuntimeMin" `
    "/DMyArchitecture=$Architecture" `
    "/DMyArchitecturesAllowed=$($redist.InstallerArchitecture)" `
    "/DMyOutputDir=$outputDir" `
    $installerScript
if ($LASTEXITCODE -ne 0) {
    throw "Inno Setup compilation failed with exit code $LASTEXITCODE."
}

$installer = Join-Path $outputDir "VelaWallet-Setup-$appVersion-$Architecture.exe"
if (-not (Test-Path -LiteralPath $installer -PathType Leaf)) {
    throw "Inno Setup did not produce the expected installer: $installer"
}

$hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $installer).Hash
Write-Host "Installer: $installer"
Write-Host "SHA-256:  $hash"
