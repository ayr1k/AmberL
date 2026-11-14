param([switch]$Offline, [string]$SlintTarget = "local/ui-target/slint")
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
if (git status --porcelain --untracked-files=normal) { throw 'Package only a clean committed revision' }
$revision = (git rev-parse HEAD).Trim()
& ./scripts/build-ui-comparison.ps1 -Offline:$Offline -SlintTarget $SlintTarget
$package = Join-Path (Get-Location) ('artifacts/AmberL-ui-comparison-' + $revision.Substring(0,8))
if (Test-Path -LiteralPath $package) { throw 'Retain previous packages; choose a new revision' }
New-Item -ItemType Directory -Path $package,(Join-Path $package 'launcher'),(Join-Path $package 'slint'),(Join-Path $package 'winui'),(Join-Path $package 'scripts'),(Join-Path $package 'licenses') | Out-Null
Copy-Item (Join-Path $SlintTarget 'release/amberl.exe') -Destination (Join-Path $package 'launcher')
Copy-Item (Join-Path $SlintTarget 'release/amberl-slint-probe.exe') -Destination (Join-Path $package 'slint')
# Preserve all SDK runtime assets and subdirectories; exclude build/debug products.
Get-ChildItem local/ui-build/winui | Where-Object { $_.Name -notmatch '\.(pdb|lib|exp|ilk|iobj|ipdb|recipe)$' -and $_.Name -ne 'amberl-winui-probe.pri' } | Copy-Item -Destination (Join-Path $package 'winui') -Recurse
Copy-Item assets/instance-icons/LICENSE-lucide.txt -Destination (Join-Path $package 'licenses')
Copy-Item LICENSE,THIRD_PARTY_NOTICES.md,docs/UI-COMPARISON.md,docs/NATIVE-UI.md,docs/LOADER-VERIFICATION.md,docs/MULTIPLAYER.md,docs/CONTENT.md,docs/FIRST-VERSION.md,docs/INSTALLER.md,design-qa.md -Destination $package
Copy-Item scripts/make-fixture.ps1 -Destination (Join-Path $package 'scripts')
git -c core.autocrlf=false archive --format=zip "--output=$package/AmberL-source.zip" HEAD
if ($LASTEXITCODE) { throw 'Exact source archive failed' }
Set-Content -LiteralPath (Join-Path $package 'TEST-REVISION.txt') -Value $revision -Encoding utf8
foreach ($manifest in @('Cargo.toml','experiments/slint/Cargo.toml')) {
    $metadata = cargo metadata --manifest-path $manifest --locked --offline --format-version 1 --filter-platform x86_64-pc-windows-msvc | ConvertFrom-Json
    if ($LASTEXITCODE) { throw 'Cannot resolve dependency notices' }
    $resolved = @{}
    cargo tree --manifest-path $manifest --locked --offline --target x86_64-pc-windows-msvc --edges normal,build --prefix none --format '{p}' | ForEach-Object {
        if ($_ -match '^(\S+) v(\S+)') { $resolved[$Matches[1] + '@' + $Matches[2]] = $true }
    }
    if ($LASTEXITCODE) { throw 'Cannot resolve active dependency graph' }
    foreach ($dependency in $metadata.packages | Where-Object { $_.source -and $resolved.ContainsKey($_.name + '@' + $_.version) }) {
        $destination = Join-Path $package ('licenses/' + $dependency.name + '-' + $dependency.version)
        if (Test-Path -LiteralPath $destination) { continue }
        New-Item -ItemType Directory -Path $destination | Out-Null
        $directory = Split-Path $dependency.manifest_path -Parent
        $notices = @(Get-ChildItem -LiteralPath $directory | Where-Object { $_.Name -match '^(LICENSE|COPYING|NOTICE|UNLICENSE)' })
        if ($dependency.license_file) { $notices += Get-Item -LiteralPath (Join-Path $directory $dependency.license_file) }
        if (-not $notices) {
            $supplement = Join-Path 'third_party/ui-licenses' ($dependency.name + '-' + $dependency.version)
            if (Test-Path -LiteralPath $supplement) { $notices = @(Get-ChildItem -LiteralPath $supplement) }
        }
        if (-not $notices) { throw ('Missing notices: ' + $dependency.name) }
        $notices | Sort-Object FullName -Unique | Copy-Item -Destination $destination -Recurse
    }
}
$lock = Get-Content experiments/winui/packages.lock.json -Raw | ConvertFrom-Json
foreach ($dependency in $lock.dependencies.'native,Version=v0.0'.PSObject.Properties) {
    $name = $dependency.Name.ToLowerInvariant(); $version = $dependency.Value.resolved
    # BuildTools executes during compilation; its tools are not redistributed.
    if ($name -eq 'microsoft.windows.sdk.buildtools') { continue }
    $directory = Join-Path (Get-Location) ('local/nuget/' + $name + '/' + $version)
    $destination = Join-Path $package ('licenses/' + $name + '-' + $version)
    New-Item -ItemType Directory -Path $destination -Force | Out-Null
    $notices = @(Get-ChildItem -LiteralPath $directory -File | Where-Object { $_.Name -match 'license|notice|copying' })
    if (-not $notices) { throw ('Missing NuGet notices: ' + $name) }
    $notices | Copy-Item -Destination $destination
}
$rustRoot = (rustc --print sysroot).Trim()
Copy-Item -LiteralPath (Join-Path $rustRoot 'share/doc/rust/COPYRIGHT.html') -Destination (Join-Path $package 'licenses/Rust-COPYRIGHT.html')
$checksums = Get-ChildItem -LiteralPath $package -File -Recurse | Sort-Object FullName | ForEach-Object {
    '{0}  {1}' -f (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant(),[IO.Path]::GetRelativePath($package,$_.FullName).Replace('\','/')
}
Set-Content -LiteralPath (Join-Path $package 'SHA256SUMS.txt') -Value $checksums -Encoding utf8
Compress-Archive -LiteralPath $package -DestinationPath ($package + '.zip')
Get-FileHash -LiteralPath ($package + '.zip') -Algorithm SHA256
& ./scripts/package-installer.ps1 -PackageRoot $package
