# Builds Formula-Setup-<version>.exe on a Windows 10/11 (64-bit) PC.
# Run it by double-clicking build-installer.bat, or from PowerShell:
#   powershell -ExecutionPolicy Bypass -File build-installer.ps1 [-Arch x64|x86]
# -Arch x64 (default) builds for 64-bit Windows; -Arch x86 builds a 32-bit program that also
# runs on 32-bit Windows.
# Missing tools (Rust, the Visual Studio C++ build tools, Inno Setup) are offered for
# installation with winget, which comes with Windows.

param([ValidateSet("x64", "x86")][string]$Arch = "x64")
$ErrorActionPreference = "Stop"
$Triple = if ($Arch -eq "x86") { "i686-pc-windows-msvc" } else { "x86_64-pc-windows-msvc" }
$Here = Split-Path -Parent $MyInvocation.MyCommand.Path
$App = Resolve-Path (Join-Path $Here "..\..")

function Step($text) { Write-Host ""; Write-Host "== $text" -ForegroundColor Cyan }
function Refresh-Path {
    $env:Path = [Environment]::GetEnvironmentVariable("Path", "Machine") + ";" +
                [Environment]::GetEnvironmentVariable("Path", "User") + ";" +
                (Join-Path $env:USERPROFILE ".cargo\bin")
}
function Ask($question) {
    $answer = Read-Host "$question [Y/n]"
    return ($answer -eq "" -or $answer -match "^[Yy]")
}
function Winget-Install($id, $extra) {
    if (-not (Get-Command winget -ErrorAction SilentlyContinue)) {
        throw "winget is not available. Install '$id' by hand (see README.md in this folder), then run this again."
    }
    $wargs = @("install", "--id", $id, "-e", "--accept-package-agreements", "--accept-source-agreements") + $extra
    & winget @wargs
    Refresh-Path
}

Refresh-Path

# 1. Visual Studio C++ build tools (the linker Rust uses on Windows).
Step "Checking the C++ build tools"
$vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
$haveVc = $false
if (Test-Path $vswhere) {
    $vc = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    $haveVc = [bool]$vc
}
if (-not $haveVc) {
    if (Ask "The Visual Studio C++ build tools are missing (about 2-6 GB). Install them now?") {
        Winget-Install "Microsoft.VisualStudio.2022.BuildTools" @("--override", "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended")
    } else { throw "The C++ build tools are needed to build Formula." }
}

# 2. Rust.
Step "Checking Rust"
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    if (Ask "Rust is missing. Install it now?") {
        Winget-Install "Rustlang.Rustup" @()
        & rustup default stable-msvc
    } else { throw "Rust is needed to build Formula." }
}
& rustup target add $Triple | Out-Null

# 3. Inno Setup (makes the installer).
Step "Checking Inno Setup"
function Find-Iscc {
    $cmd = Get-Command ISCC.exe -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    foreach ($dir in @("${env:ProgramFiles(x86)}\Inno Setup 6", "$env:ProgramFiles\Inno Setup 6", "$env:LOCALAPPDATA\Programs\Inno Setup 6")) {
        $p = Join-Path $dir "ISCC.exe"
        if (Test-Path $p) { return $p }
    }
    return $null
}
$iscc = Find-Iscc
if (-not $iscc) {
    if (Ask "Inno Setup is missing. Install it now?") {
        Winget-Install "JRSoftware.InnoSetup" @()
        $iscc = Find-Iscc
    }
    if (-not $iscc) { throw "Inno Setup 6 is needed to make the installer." }
}

# 4. Build formula.exe.
Step "Building Formula (the first build takes several minutes)"
Push-Location $App
try {
    & cargo build --release --target $Triple
    if ($LASTEXITCODE -ne 0) { throw "The build failed; see the messages above." }
    New-Item -ItemType Directory -Force -Path "target\release" | Out-Null
    Copy-Item "target\$Triple\release\formula.exe" "target\release\formula.exe" -Force
} finally { Pop-Location }

# 5. Make the installer, using the version from Cargo.toml.
$version = (Select-String -Path (Join-Path $App "Cargo.toml") -Pattern '^version\s*=\s*"([^"]+)"' |
            Select-Object -First 1).Matches[0].Groups[1].Value
Step "Making the $Arch installer for version $version"
Push-Location $Here
try {
    & $iscc "/DAppVersion=$version" "/DAppArch=$Arch" "formula.iss"
    if ($LASTEXITCODE -ne 0) { throw "Inno Setup failed; see the messages above." }
} finally { Pop-Location }

$name = if ($Arch -eq "x64") { "Formula-Setup-$version.exe" } else { "Formula-Setup-$version-$Arch.exe" }
$out = Join-Path $Here "Output\$name"
Step "Done: $out"
