param([string]$Executable)
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
if (-not $Executable) {
    $Executable = Join-Path $PSScriptRoot 'amberl-inspect.exe'
    if (-not (Test-Path -LiteralPath $Executable)) {
        $Executable = Join-Path (Split-Path $PSScriptRoot -Parent) 'target/release/amberl-inspect.exe'
    }
}
$Executable = (Resolve-Path -LiteralPath $Executable).Path
$fixture = Join-Path $env:TEMP ('AmberL-resource-plan-' + [guid]::NewGuid().ToString('N'))
$cache = Join-Path $fixture 'downloads-v1'
New-Item -ItemType Directory -Path $cache -Force | Out-Null
# Tiny synthetic bytes only. Seeded cache exercises the packaged CLI, not live HTTPS.
$abc = 'a9993e364706816aba3e25717850c26c9cd0d89d'
$empty = 'da39a3ee5e6b4b0d3255bfef95601890afd80709'
$indexHash = '99e81adb2e8fd2e5418c61e53e8aa5f17a83ad08'
$indexBytes = [Text.Encoding]::UTF8.GetBytes('{"objects":{"sound/a":{"hash":"a9993e364706816aba3e25717850c26c9cd0d89d","size":3}}}')
[IO.File]::WriteAllBytes((Join-Path $cache ('sha1-' + $indexHash)), $indexBytes)
[IO.File]::WriteAllBytes((Join-Path $cache ('sha1-' + $abc)), [Text.Encoding]::UTF8.GetBytes('abc'))
[IO.File]::WriteAllBytes((Join-Path $cache ('sha1-' + $empty)), [byte[]]@())
$metadata = @{
    id='test'; mainClass='net.minecraft.client.main.Main'
    downloads=@{client=@{url='https://example.invalid/client';sha1=$abc;size=3}}
    libraries=@(@{downloads=@{artifact=@{url='https://example.invalid/library';sha1=$empty;size=0;path='test/lib.jar'}}})
    assetIndex=@{id='test';url='https://example.invalid/index';sha1=$indexHash;size=84}
    arguments=@{jvm=@();game=@()}
}
$metadataPath = Join-Path $fixture 'version.json'
[IO.File]::WriteAllText($metadataPath, ($metadata | ConvertTo-Json -Depth 10))
$before = (Get-FileHash -LiteralPath $metadataPath -Algorithm SHA256).Hash
$planText = & $Executable plan-vanilla $metadataPath '10.0.26100'
if ($LASTEXITCODE -ne 0) { throw 'Version planning failed' }
$plan = ($planText -join "`n") | ConvertFrom-Json
if ($plan.files.Count -ne 3 -or $plan.classpath.Count -ne 2 -or $null -ne $plan.java_major) { throw 'Unexpected version plan' }
$indexPath = Join-Path $cache ('sha1-' + $indexHash)
$assetText = & $Executable plan-assets $indexPath
if ($LASTEXITCODE -ne 0) { throw 'Asset planning failed' }
$assets = ($assetText -join "`n") | ConvertFrom-Json
if ($assets.files.Count -ne 1) { throw 'Unexpected asset plan' }
$output = & $Executable prepare-vanilla $fixture $metadataPath '10.0.26100' 87 2> (Join-Path $fixture 'progress.log')
if ($LASTEXITCODE -ne 0) { throw 'Seeded-cache preparation failed' }
$prepared = ($output -join "`n") | ConvertFrom-Json
if ($prepared.verified_files -ne 3 -or $prepared.cache_hits -ne 3 -or $prepared.verified_bytes -ne 87) { throw 'Incorrect cache preparation summary' }
$null = & $Executable prepare-vanilla $fixture $metadataPath '10.0.26100' 86 2> (Join-Path $fixture 'budget.log')
if ($LASTEXITCODE -eq 0) { throw 'Explicit byte budget was ignored' }
if (Test-Path -LiteralPath (Join-Path $fixture 'versions')) { throw 'Unexpected game directory creation' }
if ((Get-FileHash -LiteralPath $metadataPath -Algorithm SHA256).Hash -ne $before) { throw 'Input metadata changed' }
[ordered]@{
    passed=$true
    executable_sha256=(Get-FileHash -LiteralPath $Executable -Algorithm SHA256).Hash.ToLowerInvariant()
    verified_files=3;verified_bytes=87;cache_hits=3;budget_rejection=$true;input_unchanged=$true
    scope='Synthetic metadata and seeded cache; not live HTTPS, game installation or launch'
} | ConvertTo-Json
