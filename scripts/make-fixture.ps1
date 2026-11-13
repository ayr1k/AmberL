param([string]$Destination = 'local/fixture', [ValidateRange(4, 20000)][int]$Count = 6)
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
if (Test-Path -LiteralPath $Destination) { throw 'Choose a new destination; existing data is never replaced.' }
$root = [IO.Path]::GetFullPath($Destination)
New-Item -ItemType Directory -Path $root | Out-Null
for ($i = 0; $i -lt $Count; $i++) {
    $name = '测试实例-{0:D5}' -f $i
    $directory = Join-Path $root ('versions/' + $name)
    New-Item -ItemType Directory -Path $directory | Out-Null
    $metadata = @{id=$name;mainClass='net.minecraft.client.main.Main';libraries=@();patches=@(@{id='game';version='1.21.4'})}
    if ($i % 3 -eq 1) { $metadata.libraries=@(@{name='net.fabricmc:fabric-loader:0.16.10'}) }
    Set-Content -LiteralPath (Join-Path $directory ($name + '.json')) -Value ($metadata | ConvertTo-Json -Depth 5) -Encoding utf8
    switch ($i % 4) {
        0 { } # deliberately unknown
        1 { Set-Content -LiteralPath (Join-Path $directory 'hmclversion.cfg') -Value '{"usesGlobal":false,"gameDirType":1}' -Encoding utf8 }
        2 { Set-Content -LiteralPath (Join-Path $directory 'hmclversion.cfg') -Value '{"usesGlobal":false,"gameDirType":0}' -Encoding utf8 }
        3 {
            Set-Content -LiteralPath (Join-Path $directory 'hmclversion.cfg') -Value '{"usesGlobal":false,"gameDirType":0}' -Encoding utf8
            New-Item -ItemType Directory -Path (Join-Path $directory 'PCL') | Out-Null
            Set-Content -LiteralPath (Join-Path $directory 'PCL/Setup.ini') -Value 'VersionArgumentIndieV2:True' -Encoding utf8
        }
    }
}
Write-Output $root
