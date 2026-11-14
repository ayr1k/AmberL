param([switch]$Offline, [string]$SlintTarget = "local/ui-target/slint")
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
$env:CARGO_TARGET_DIR = Join-Path (Get-Location) 'target'
cargo build -p amberl-ui-bridge --release --locked
if ($LASTEXITCODE) { throw 'Rust bridge build failed' }
$env:CARGO_TARGET_DIR = Join-Path (Get-Location) $SlintTarget
$env:SLINT_STYLE = 'fluent'
$cargoArgs = @('build','--manifest-path','experiments/slint/Cargo.toml','--release','--locked')
if ($Offline) { $cargoArgs += '--offline' }
& cargo @cargoArgs
if ($LASTEXITCODE) { throw 'Slint build failed' }
$env:CARGO_TARGET_DIR = Join-Path (Get-Location) 'target'
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
$vs = (& $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath | Select-Object -First 1)
if (-not $vs) { throw 'Visual Studio C++ v143 build tools and Windows SDK 10.0.26100.0 are required' }
$msbuild = Join-Path $vs 'MSBuild/Current/Bin/MSBuild.exe'
$buildArgs = @('experiments/winui/AmberL.WinUI.vcxproj','/p:Configuration=Release','/p:Platform=x64','/p:RestoreLockedMode=true','/verbosity:minimal','/nologo')
if (-not $Offline) { $buildArgs += '/restore' }
& $msbuild @buildArgs
if ($LASTEXITCODE) { throw 'WinUI build failed' }
if (-not (Test-Path local/ui-build/winui/resources.pri)) { throw 'WinUI resource index missing' }

