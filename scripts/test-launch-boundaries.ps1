param([string]$Executable)
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
if (-not $Executable) {
    $Executable = Join-Path $PSScriptRoot 'amberl-inspect.exe'
    if (-not (Test-Path -LiteralPath $Executable)) { $Executable = Join-Path (Split-Path $PSScriptRoot -Parent) 'target/release/amberl-inspect.exe' }
}
$Executable = (Resolve-Path -LiteralPath $Executable).Path
$temporary = Join-Path $env:TEMP ('AmberL-launch-smoke-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $temporary | Out-Null
$accounts = & $Executable accounts $temporary
if ($LASTEXITCODE -ne 0 -or (($accounts -join "`n") | ConvertFrom-Json).Count -ne 0) { throw 'Fresh account store must be empty' }
$output = & $Executable accounts $temporary verify ('0' * 32) 2>&1
if ($LASTEXITCODE -eq 0 -or ($output -join "`n") -notmatch 'Selected account no longer exists') { throw 'Missing account must fail without network or fallback' }
$output = & $Executable accounts $temporary remove ('0' * 32) 2>&1
if ($LASTEXITCODE -eq 0 -or ($output -join "`n") -notmatch 'Unknown account ID') { throw 'Unknown logout must not reset storage' }
$output = & $Executable check-launch $temporary $temporary test missing-java.exe ('0' * 32) 0 10.0.26100 2>&1
if ($LASTEXITCODE -eq 0 -or ($output -join "`n") -notmatch 'Memory must be explicitly selected') { throw 'Invalid launch must fail before executable inspection/network' }
if (Test-Path -LiteralPath (Join-Path $temporary 'accounts-v1.dpapi')) { throw 'Negative operations must not create account credentials' }
@{passed=$true;scope='Packaged account/launch CLI boundaries; synthetic input, no real login or game'} | ConvertTo-Json
exit 0
