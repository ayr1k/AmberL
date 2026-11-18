param(
    [string]$Executable,
    [string[]]$Versions = @('1.21.4'),
    [long]$MaxBytes = 2147483648,
    [string]$OutputDirectory
)
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
if (-not $Executable) {
    $Executable = Join-Path $PSScriptRoot 'amberl-inspect.exe'
    if (-not (Test-Path -LiteralPath $Executable)) { $Executable = Join-Path (Split-Path $PSScriptRoot -Parent) 'target/release/amberl-inspect.exe' }
}
$Executable = (Resolve-Path -LiteralPath $Executable).Path
if (-not $OutputDirectory) { $OutputDirectory = Join-Path $env:TEMP ('AmberL-live-install-' + [guid]::NewGuid().ToString('N')) }
if (Test-Path -LiteralPath $OutputDirectory) { throw 'Use a new disposable output directory' }
$app = Join-Path $OutputDirectory 'app'
$games = Join-Path $OutputDirectory 'games'
New-Item -ItemType Directory -Path $app,$games | Out-Null
$results = @()
foreach ($version in $Versions) {
    if ($version -notmatch '^[a-zA-Z0-9._-]+$') { throw 'Invalid test version' }
    $raw = & $Executable resolve-vanilla $app $version
    if ($LASTEXITCODE -ne 0) { throw ('Official metadata resolution failed: ' + $version) }
    $resolved = ($raw -join "`n") | ConvertFrom-Json
    $metadataHash = (Get-FileHash -LiteralPath $resolved.metadata_path -Algorithm SHA1).Hash.ToLowerInvariant()
    if ($metadataHash -ne $resolved.metadata_sha1) { throw 'Metadata SHA-1 mismatch' }
    $raw = & $Executable prepare-vanilla $app $resolved.metadata_path '10.0.26100' $MaxBytes 2> (Join-Path $OutputDirectory ($version + '-prepare.log'))
    if ($LASTEXITCODE -ne 0) { throw ('Actual Ambrum resource preparation failed: ' + $version) }
    $prepared = ($raw -join "`n") | ConvertFrom-Json
    $raw = & $Executable prepare-vanilla $app $resolved.metadata_path '10.0.26100' $MaxBytes 2> (Join-Path $OutputDirectory ($version + '-reuse.log'))
    if ($LASTEXITCODE -ne 0) { throw 'Cache reuse preparation failed' }
    $reused = ($raw -join "`n") | ConvertFrom-Json
    if ($reused.cache_hits -ne $reused.verified_files) { throw 'Second preparation did not reuse all resources' }
    $raw = & $Executable install-vanilla $app $resolved.metadata_path '10.0.26100' $games $version $MaxBytes 2> (Join-Path $OutputDirectory ($version + '-install.log'))
    if ($LASTEXITCODE -ne 0) { throw ('New-directory installation failed: ' + $version) }
    $installed = ($raw -join "`n") | ConvertFrom-Json
    if ($installed.launch_verified) { throw 'Installation must not claim game launch acceptance' }
    # Independently inspect every committed file against the receipt (including natives).
    $receipt = Get-Content -LiteralPath (Join-Path $installed.game_directory 'installation.ready.json') -Raw | ConvertFrom-Json
    foreach ($file in $receipt.files) {
        $path = Join-Path $installed.game_directory $file.path
        if ((Get-Item -LiteralPath $path).Length -ne $file.bytes -or (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash -ine $file.sha256) {
            throw ('Installed receipt mismatch: ' + $file.path)
        }
    }
    $metadata = Get-Content -LiteralPath $resolved.metadata_path -Raw | ConvertFrom-Json
    $client = Join-Path $installed.game_directory ('versions/' + $version + '/' + $version + '.jar')
    if ((Get-FileHash -LiteralPath $client -Algorithm SHA1).Hash -ine $metadata.downloads.client.sha1) { throw 'Installed client differs from official descriptor' }
    $results += [ordered]@{
        version=$version;metadata_sha1=$metadataHash;verified_files=$prepared.verified_files
        verified_bytes=$prepared.verified_bytes;second_prepare_cache_hits=$reused.cache_hits
        materialized_files=$installed.materialized_files;materialized_bytes=$installed.materialized_bytes
        extracted_native_files=$installed.extracted_native_files;receipt_files_checked=$receipt.files.Count
        official_client_sha1_verified=$true;launch_verified=$false
    }
    $results[-1] | ConvertTo-Json
}
$evidence = [ordered]@{
    passed=$true;executable_sha256=(Get-FileHash -LiteralPath $Executable -Algorithm SHA256).Hash.ToLowerInvariant()
    scope='Actual Ambrum HTTPS, official metadata/resources, verified cache reuse and new-directory materialization; no login/game execution'
    versions=$results
}
$evidence | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $OutputDirectory 'result.json') -Encoding utf8
$evidence | ConvertTo-Json -Depth 8
exit 0
