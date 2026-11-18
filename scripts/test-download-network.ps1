param([string]$Executable)
$ErrorActionPreference = 'Stop'
if (-not $Executable) {
    $Executable = Join-Path $PSScriptRoot 'amberl-inspect.exe'
    if (-not (Test-Path -LiteralPath $Executable)) {
        $Executable = Join-Path (Split-Path $PSScriptRoot -Parent) 'target/release/amberl-inspect.exe'
    }
}
$Executable = (Resolve-Path -LiteralPath $Executable).Path
# Public license fixture pinned to a commit. No login, game files or OS changes.
$url = 'https://raw.githubusercontent.com/ayr1k/amber-launcher/3af52c7a3fbd5e301ed12d204b3e8fa3d9ae69b2/LICENSE'
$digest = 'fab3dd6bdab226f1c08630b1dd917e11fcb4ec5e1e020e2c16f83a0a13863e85'
$directory = Join-Path $env:TEMP ('AmberL-network-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $directory | Out-Null
$result = & $Executable fetch $directory $url ('sha256:' + $digest) 16726
if ($LASTEXITCODE -ne 0) {
    Write-Output 'Live HTTPS download did not pass. Check the reported network/verification error; no successful download is claimed.'
    exit 2
}
$download = ($result -join "`n") | ConvertFrom-Json
if ($download.cache_hit -or (Get-FileHash -LiteralPath $download.path -Algorithm SHA256).Hash -ine $digest) {
    throw 'Downloaded fixture does not match expected result'
}
$result = & $Executable fetch $directory $url ('sha256:' + $digest) 16726
if ($LASTEXITCODE -ne 0 -or -not (($result -join "`n") | ConvertFrom-Json).cache_hit) { throw 'Verified cache reuse failed' }
[ordered]@{passed=$true;bytes=16726;sha256=$digest;cache_reused=$true;scope='Public license only; not Minecraft install/login acceptance'} | ConvertTo-Json
