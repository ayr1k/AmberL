param([Parameter(Mandatory=$true)][string]$Destination, [Parameter(Mandatory=$true)][string]$Licenses)
$ErrorActionPreference='Stop'
# Only release redistributables from the installed official VS toolchain, never System32.
$vswhere=Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
$vs=(& $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath | Select-Object -First 1)
if (-not $vs) { throw 'Visual C++ redistributable source is unavailable' }
$version=(Get-Content -LiteralPath (Join-Path $vs 'VC/Auxiliary/Build/Microsoft.VCRedistVersion.default.txt') -Raw).Trim()
if ($version -notmatch '^\d+\.\d+\.\d+$') { throw 'Invalid VC redistributable version' }
$root=Join-Path $vs ('VC/Redist/MSVC/'+$version+'/x64')
$crt=@(Get-ChildItem -LiteralPath $root -Directory -Filter 'Microsoft.VC*.CRT')
if ($crt.Count -ne 1) { throw 'Ambiguous VC redistributable directory' }
$files=@(Get-ChildItem -LiteralPath $crt[0].FullName -File -Filter '*.dll')
if (-not ($files.Name -contains 'vcruntime140.dll')) { throw 'Missing VCRUNTIME140 runtime' }
$destination=(Resolve-Path -LiteralPath $Destination).Path
$notice=Join-Path $Licenses ('microsoft-vc-runtime-'+$version)
New-Item -ItemType Directory -Path $notice -Force | Out-Null
foreach($file in $files) {
    $signature=Get-AuthenticodeSignature -LiteralPath $file.FullName
    if($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch 'O=Microsoft Corporation') { throw ('Invalid Microsoft runtime signature: '+$file.Name) }
    Copy-Item -LiteralPath $file.FullName -Destination $destination
}
$notices=@(Get-ChildItem -LiteralPath (Join-Path $vs 'Licenses') -Recurse -File | Where-Object {$_.Name -in @('Redist.txt','ThirdPartyNotices.txt')})
if (-not $notices) { throw 'Visual Studio redistribution notices are missing' }
foreach($file in $notices) {
    $relative=[IO.Path]::GetRelativePath((Join-Path $vs 'Licenses'),$file.FullName)
    $target=Join-Path $notice $relative
    New-Item -ItemType Directory -Path (Split-Path $target -Parent) -Force | Out-Null
    Copy-Item -LiteralPath $file.FullName -Destination $target
}
@("Microsoft Visual C++ runtime $version, x64, unmodified release redistributables.",
  'https://learn.microsoft.com/en-us/visualstudio/releases/2022/redistribution',
  'https://learn.microsoft.com/en-us/cpp/windows/redistributing-visual-cpp-files',
  'App-local files are serviced by AmberL package updates. No system runtime is installed or changed.') |
    Set-Content -LiteralPath (Join-Path $notice 'SOURCE.txt') -Encoding utf8
