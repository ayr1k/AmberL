$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
$revision = (git rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0) { throw 'Commit the reviewed source before packaging.' }
if (git status --porcelain --untracked-files=normal) { throw 'Package only a clean source revision.' }
cargo build --workspace --release --locked
if ($LASTEXITCODE -ne 0) { throw 'Build of the pinned source failed.' }
$packageRoot = Join-Path (Get-Location) ('artifacts/AmberL-probe-' + $revision.Substring(0, 8))
if (Test-Path -LiteralPath $packageRoot) { throw 'Package already exists; retain earlier artifacts.' }
New-Item -ItemType Directory -Path $packageRoot | Out-Null
Copy-Item target/release/amberl-native-probe.exe,target/release/amberl-inspect.exe,LICENSE,README.md,THIRD_PARTY_NOTICES.md,Cargo.lock -Destination $packageRoot
Copy-Item docs -Destination $packageRoot -Recurse
Copy-Item scripts/test-download-network.ps1 -Destination $packageRoot
Copy-Item scripts/test-resource-plan.ps1 -Destination $packageRoot
$sourceArchive = Join-Path $packageRoot 'AmberL-source.zip'
git archive --format=zip "--output=$sourceArchive" HEAD
if ($LASTEXITCODE -ne 0) { throw 'Cannot archive the exact source revision.' }
Set-Content -LiteralPath (Join-Path $packageRoot 'TEST-REVISION.txt') -Value $revision -Encoding utf8
$metadata = cargo metadata --locked --offline --format-version 1 | ConvertFrom-Json
if ($LASTEXITCODE -ne 0) { throw 'Cannot resolve exact dependency notices.' }
$noticeRoot = Join-Path $packageRoot 'licenses'
New-Item -ItemType Directory -Path $noticeRoot | Out-Null
foreach ($package in $metadata.packages | Where-Object source) {
    $destination = Join-Path $noticeRoot ($package.name + '-' + $package.version)
    New-Item -ItemType Directory -Path $destination | Out-Null
    $files = Get-ChildItem -LiteralPath (Split-Path $package.manifest_path -Parent) -File |
        Where-Object { $_.Name -match '^(LICENSE|COPYING|NOTICE|UNLICENSE)' }
    if (-not $files) { throw ('Missing notices for ' + $package.name) }
    $files | Copy-Item -Destination $destination
}
$rustRoot = (rustc --print sysroot).Trim()
$rustNotice = Join-Path $rustRoot 'share/doc/rust/COPYRIGHT.html'
if (-not (Test-Path -LiteralPath $rustNotice)) { throw 'Rust copyright notice is missing; install the rust-docs component before packaging.' }
Copy-Item -LiteralPath $rustNotice -Destination (Join-Path $noticeRoot 'Rust-COPYRIGHT.html')
$checksums = Get-ChildItem -LiteralPath $packageRoot -Recurse -File | Sort-Object FullName | ForEach-Object {
    $relative = [IO.Path]::GetRelativePath($packageRoot, $_.FullName).Replace('\', '/')
    '{0}  {1}' -f (Get-FileHash -Algorithm SHA256 -LiteralPath $_.FullName).Hash.ToLowerInvariant(), $relative
}
Set-Content -LiteralPath (Join-Path $packageRoot 'SHA256SUMS.txt') -Value $checksums -Encoding utf8
$zip = $packageRoot + '.zip'
Compress-Archive -LiteralPath $packageRoot -DestinationPath $zip
Get-FileHash -Algorithm SHA256 -LiteralPath $zip | Select-Object Path, Hash
