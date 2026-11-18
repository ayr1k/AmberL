param([string]$Executable)
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
if (-not $Executable) {
    $Executable = Join-Path $PSScriptRoot 'amberl-inspect.exe'
    if (-not (Test-Path -LiteralPath $Executable)) { $Executable = Join-Path (Split-Path $PSScriptRoot -Parent) 'target/release/amberl-inspect.exe' }
}
$Executable = (Resolve-Path -LiteralPath $Executable).Path
$root = Join-Path $env:TEMP ('AmberL-install-cli-' + [guid]::NewGuid().ToString('N'))
$app = Join-Path $root 'app'
$cache = Join-Path $app 'downloads-v1'
$games = Join-Path $root 'games'
New-Item -ItemType Directory -Path $cache,$games | Out-Null
$abc = 'a9993e364706816aba3e25717850c26c9cd0d89d'
$indexBytes = [Text.Encoding]::UTF8.GetBytes('{"objects":{}}')
$indexHash = [Convert]::ToHexString([Security.Cryptography.SHA1]::HashData($indexBytes)).ToLowerInvariant()
[IO.File]::WriteAllBytes((Join-Path $cache ('sha1-' + $abc)), [Text.Encoding]::UTF8.GetBytes('abc'))
[IO.File]::WriteAllBytes((Join-Path $cache ('sha1-' + $indexHash)), $indexBytes)
$metadata = @{id='test'; mainClass='net.minecraft.client.main.Main';libraries=@();arguments=@{jvm=@();game=@()}
    downloads=@{client=@{url='https://example.invalid/client';sha1=$abc;size=3}}
    assetIndex=@{id='test';url='https://example.invalid/index';sha1=$indexHash;size=$indexBytes.Length}}
$metadataPath = Join-Path $root 'metadata.json'
[IO.File]::WriteAllText($metadataPath, ($metadata | ConvertTo-Json -Depth 10))
$before = (Get-FileHash -LiteralPath $metadataPath -Algorithm SHA256).Hash
$raw = & $Executable install-vanilla $app $metadataPath '10.0.26100' $games 'CLI test 中文' 65536 2> (Join-Path $root 'install.log')
if ($LASTEXITCODE -ne 0) { throw 'Packaged installation failed' }
$installed = ($raw -join "`n") | ConvertFrom-Json
if ($installed.launch_verified -or $installed.materialized_files -ne 3) { throw 'Incorrect installation result' }
$jar = Join-Path $installed.game_directory 'versions/test/test.jar'
if ((Get-FileHash -LiteralPath $jar -Algorithm SHA1).Hash -ine $abc) { throw 'Installed resource mismatch' }
$null = & $Executable install-vanilla $app $metadataPath '10.0.26100' $games 'CLI test 中文' 65536 2> (Join-Path $root 'conflict.log')
if ($LASTEXITCODE -eq 0) { throw 'Existing instance was not rejected' }
if ((Get-FileHash -LiteralPath $jar -Algorithm SHA1).Hash -ine $abc) { throw 'Existing instance changed' }
if ((Get-FileHash -LiteralPath $metadataPath -Algorithm SHA256).Hash -ne $before) { throw 'Source metadata changed' }
[ordered]@{passed=$true;new_directory_install=$true;existing_directory_preserved=$true;unicode_path=$true;launch_verified=$false;
    scope='Synthetic CLI materialization; not actual Minecraft login or launch'} | ConvertTo-Json
exit 0
