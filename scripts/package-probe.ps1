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
Copy-Item scripts/test-install.ps1,scripts/test-install-network.ps1 -Destination $packageRoot
$sourceArchive = Join-Path $packageRoot 'AmberL-source.zip'
git -c core.autocrlf=false archive --format=zip "--output=$sourceArchive" HEAD
if ($LASTEXITCODE -ne 0) { throw 'Cannot archive the exact source revision.' }
# Git archive respects core.autocrlf. Check actual Git blob identities so a host
# preference cannot silently change source bytes in the delivered snapshot.
$expectedBlobs = [Collections.Generic.Dictionary[string,string]]::new([StringComparer]::Ordinal)
git ls-tree -r '--format=%(objectname) %(path)' HEAD | ForEach-Object {
    if ($_ -notmatch '^([0-9a-f]{40}) (.+)$') { throw 'Unexpected source tree entry' }
    $expectedBlobs.Add($Matches[2], $Matches[1])
}
if ($LASTEXITCODE -ne 0) { throw 'Cannot enumerate source tree.' }
$sourceZip = [IO.Compression.ZipFile]::OpenRead($sourceArchive)
try {
    $checkedBlobs = 0
    foreach ($entry in $sourceZip.Entries | Where-Object { -not $_.FullName.EndsWith('/') }) {
        if (-not $expectedBlobs.ContainsKey($entry.FullName)) { throw 'Unexpected source archive file' }
        $stream = $entry.Open()
        $memory = [IO.MemoryStream]::new()
        try { $stream.CopyTo($memory); $bytes = $memory.ToArray() }
        finally { $stream.Dispose(); $memory.Dispose() }
        $header = [Text.Encoding]::UTF8.GetBytes(('blob ' + $bytes.Length + [char]0))
        $blobHash = [Convert]::ToHexString([Security.Cryptography.SHA1]::HashData([byte[]]($header + $bytes))).ToLowerInvariant()
        if ($blobHash -ne $expectedBlobs[$entry.FullName]) { throw ('Source blob mismatch: ' + $entry.FullName) }
        $checkedBlobs++
    }
    if ($checkedBlobs -ne $expectedBlobs.Count) { throw 'Source archive is incomplete' }
} finally { $sourceZip.Dispose() }
Set-Content -LiteralPath (Join-Path $packageRoot 'TEST-REVISION.txt') -Value $revision -Encoding utf8
$hostTarget = ((rustc -vV | Select-String '^host: ').Line -replace '^host: ', '').Trim()
if (-not $hostTarget) { throw 'Cannot identify the package build target.' }
$metadata = cargo metadata --locked --offline --format-version 1 --filter-platform $hostTarget | ConvertFrom-Json
if ($LASTEXITCODE -ne 0) { throw 'Cannot resolve exact dependency notices.' }
$resolvedPackages = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
foreach ($node in $metadata.resolve.nodes) { [void]$resolvedPackages.Add($node.id) }
$noticeRoot = Join-Path $packageRoot 'licenses'
New-Item -ItemType Directory -Path $noticeRoot | Out-Null
foreach ($package in $metadata.packages | Where-Object { $_.source -and $resolvedPackages.Contains($_.id) }) {
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
