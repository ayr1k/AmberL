param([Parameter(Mandatory=$true)][string]$PackageRoot)
$ErrorActionPreference='Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
if (git status --porcelain --untracked-files=normal) { throw 'Package only a clean committed revision' }
$revision=(git rev-parse HEAD).Trim()
$package=(Resolve-Path -LiteralPath $PackageRoot).Path
if ((Get-Content -LiteralPath (Join-Path $package 'TEST-REVISION.txt') -Raw).Trim() -ne $revision) { throw 'Package/source revision mismatch' }
$toolRoot=Join-Path (Get-Location) 'local/tools'
$compilerRoot=Join-Path $toolRoot 'inno-7.1.1'
$compiler=Join-Path $compilerRoot 'ISCC.exe'
if (-not (Test-Path -LiteralPath $compiler)) {
    New-Item -ItemType Directory -Path $toolRoot -Force | Out-Null
    $download=Join-Path $toolRoot 'innosetup-7.1.1-x64.exe'
    Invoke-WebRequest 'https://github.com/jrsoftware/issrc/releases/download/is-7_1_1/innosetup-7.1.1-x64.exe' -OutFile $download
    if ((Get-FileHash -LiteralPath $download -Algorithm SHA256).Hash -ne '8A5A6711C3766A6E5FB74F280BA0E3551E719CE94538D901FDBD1F8F9920B3F0') { throw 'Compiler installer checksum mismatch' }
    $signature=Get-AuthenticodeSignature -LiteralPath $download
    if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch 'CN=Pyrsys B.V.') { throw 'Compiler installer signature mismatch' }
    $process=Start-Process -FilePath $download -ArgumentList @('/PORTABLE=1','/VERYSILENT','/CURRENTUSER','/SUPPRESSMSGBOXES','/NORESTART','/NOICONS',('/DIR="'+$compilerRoot+'"')) -WindowStyle Hidden -Wait -PassThru
    if ($process.ExitCode -ne 0) { throw 'Compiler extraction failed' }
}
$signature=Get-AuthenticodeSignature -LiteralPath $compiler
if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch 'CN=Pyrsys B.V.') { throw 'Compiler signature mismatch' }
$output=Join-Path (Get-Location) 'artifacts'
& $compiler "/DPackageRoot=$package" "/DRevision=$revision" "/DOutputRoot=$output" 'installer/amberl-preview.iss'
if ($LASTEXITCODE) { throw 'Test installer compilation failed' }
$installer=Join-Path $output ('AmberL-test-'+$revision.Substring(0,8)+'-windows-x64.exe')
$hash=(Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash.ToLowerInvariant()
Set-Content -LiteralPath ($installer+'.sha256') -Encoding utf8 -Value ($hash+'  '+[IO.Path]::GetFileName($installer))
Write-Output $installer
