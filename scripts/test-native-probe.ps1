param([string]$Executable = 'target/release/amberl-native-probe.exe')
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
$executablePath = (Resolve-Path -LiteralPath $Executable).Path
$testRoot = Join-Path (Get-Location) ('local/ui-' + [guid]::NewGuid().ToString('N'))
$dataDirectory = Join-Path $testRoot 'app'
$fixture = Join-Path $testRoot 'fixture'
New-Item -ItemType Directory -Path $dataDirectory -Force | Out-Null
& "$PSScriptRoot/make-fixture.ps1" -Destination $fixture -Count 120 | Out-Null
function Snapshot {
    @(Get-ChildItem -LiteralPath $fixture -File -Recurse | Sort-Object FullName | ForEach-Object {
        $_.FullName + ':' + (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash
    }) -join "`n"
}
$before = Snapshot
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class AmberLProbeWindow {
    delegate bool EnumProc(IntPtr hwnd, IntPtr state);
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc callback, IntPtr state);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint process);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetClassName(IntPtr hwnd, StringBuilder text, int size);
    [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr hwnd, int id);
    [DllImport("user32.dll")] public static extern bool IsWindowEnabled(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr hwnd, int x, int y, int w, int h, bool repaint);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern IntPtr SendMessageTimeoutW(IntPtr h, uint m, IntPtr w, IntPtr l, uint flags, uint timeout, out IntPtr result);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern IntPtr SendMessageTimeoutW(IntPtr h, uint m, IntPtr w, string l, uint flags, uint timeout, out IntPtr result);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern IntPtr SendMessageTimeoutW(IntPtr h, uint m, IntPtr w, StringBuilder l, uint flags, uint timeout, out IntPtr result);
    public static IntPtr Find(uint pid) {
        IntPtr found = IntPtr.Zero;
        EnumWindows((h,s) => { uint owner; GetWindowThreadProcessId(h, out owner); var c = new StringBuilder(256); GetClassName(h,c,256);
            if(owner == pid && c.ToString() == "AmberL.NativeProbe") { found=h; return false; } return true; }, IntPtr.Zero);
        return found;
    }
    public static long Send(IntPtr h, uint msg, long w=0, long l=0) {
        IntPtr result;
        if(SendMessageTimeoutW(h,msg,new IntPtr(w),new IntPtr(l),2,2000,out result)==IntPtr.Zero) throw new Exception("Window did not respond");
        return result.ToInt64();
    }
    public static void SetText(IntPtr h, string text) {
        IntPtr result;
        if(SendMessageTimeoutW(h,12,IntPtr.Zero,text,2,2000,out result)==IntPtr.Zero) throw new Exception("Text update timed out");
    }
    public static string Text(IntPtr h) {
        var text = new StringBuilder(32768); IntPtr result;
        if(SendMessageTimeoutW(h,13,new IntPtr(text.Capacity),text,2,2000,out result)==IntPtr.Zero) throw new Exception("Text read timed out");
        return text.ToString();
    }
}
'@
function Wait-Until([scriptblock]$Condition, [string]$Description) {
    $deadline = [DateTime]::UtcNow.AddSeconds(12)
    while (-not (& $Condition)) {
        if ($script:probe.HasExited) { throw "Probe exited during $Description with $($script:probe.ExitCode)" }
        if ([DateTime]::UtcNow -gt $deadline) { throw "Timed out: $Description" }
        Start-Sleep -Milliseconds 40
    }
}
function Control([int]$Id) { [AmberLProbeWindow]::GetDlgItem($script:window, $Id) }
function Click([int]$Id) { [AmberLProbeWindow]::Send((Control $Id), 245) | Out-Null }
function Launch {
    $start = [Diagnostics.ProcessStartInfo]::new($executablePath)
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.WindowStyle = [Diagnostics.ProcessWindowStyle]::Hidden
    $start.ArgumentList.Add('--automation-hidden')
    $start.ArgumentList.Add('--data-dir')
    $start.ArgumentList.Add($dataDirectory)
    $script:probe = [Diagnostics.Process]::Start($start)
    Wait-Until { $script:window = [AmberLProbeWindow]::Find($script:probe.Id); $script:window -ne [IntPtr]::Zero } 'window creation'
    Wait-Until { (Control 124) -ne [IntPtr]::Zero -and [AmberLProbeWindow]::Send((Control 124), 326) -ge 1 } 'control initialization'
}
function Close-Probe {
    [AmberLProbeWindow]::Send($script:window, 16) | Out-Null
    if (-not $script:probe.WaitForExit(5000)) { throw 'Owned probe did not close normally' }
    if ($script:probe.ExitCode -ne 0) { throw "Probe crashed: $($script:probe.ExitCode)" }
}
$checks = [Collections.Generic.List[string]]::new()
try {
    Launch
    [AmberLProbeWindow]::SetText((Control 120), $fixture)
    Click 122
    Wait-Until { [AmberLProbeWindow]::Send((Control 140), 4100) -eq 120 } 'background scan'
    $checks.Add('120 synthetic profiles scanned in actual native controls')
    [AmberLProbeWindow]::SetText((Control 130), '00001')
    Wait-Until { [AmberLProbeWindow]::Send((Control 140), 4100) -eq 1 } 'filter'
    [AmberLProbeWindow]::Send((Control 140), 256, 40) | Out-Null
    Wait-Until { [AmberLProbeWindow]::Text((Control 150)).Contains('00001') } 'keyboard row selection'
    $checks.Add('Filtering and keyboard row selection update details')
    [AmberLProbeWindow]::SetText((Control 130), '')
    Wait-Until { [AmberLProbeWindow]::Send((Control 140), 4100) -eq 120 } 'clear filter'
    [AmberLProbeWindow]::SetText((Control 120), (Join-Path $testRoot 'unscanned'))
    if ([AmberLProbeWindow]::IsWindowEnabled((Control 125))) { throw 'Changed path retained attachment permission' }
    [AmberLProbeWindow]::SetText((Control 120), $fixture)
    Click 122
    Wait-Until { [AmberLProbeWindow]::IsWindowEnabled((Control 125)) } 'preview required after path edit'
    $checks.Add('Editing the path invalidates attachment until a fresh scan succeeds')
    Click 125
    Wait-Until { [AmberLProbeWindow]::Send((Control 124), 326) -eq 2 } 'attach'
    $registryPath = Join-Path $dataDirectory 'libraries.json'
    $saved = Get-Content -LiteralPath $registryPath -Raw -Encoding utf8 | ConvertFrom-Json
    if ($saved.entries.Count -ne 1) { throw 'Attachment not committed' }
    $registeredId = $saved.entries[0].id
    Close-Probe
    Launch
    Wait-Until { [AmberLProbeWindow]::Send((Control 124), 326) -eq 2 } 'persistent library list'
    [AmberLProbeWindow]::Send((Control 124), 334, 1) | Out-Null
    [AmberLProbeWindow]::Send($script:window, 273, (124 -bor (1 -shl 16)), (Control 124).ToInt64()) | Out-Null
    Wait-Until { [AmberLProbeWindow]::Send((Control 140), 4100) -eq 120 } 'registered scan'
    $checks.Add('Attach, normal exit, restart and registered scan preserve the same library ID')
    foreach ($size in @(@(720,680),@(1140,820))) {
        [AmberLProbeWindow]::MoveWindow($script:window,0,0,$size[0],$size[1],$true) | Out-Null
        Click 102
        if (-not [AmberLProbeWindow]::Text((Control 150)).Contains('FRP')) { throw 'Multiplayer placeholder lost' }
        Click 101
    }
    $checks.Add('Small/large window resize and page switching respond without crashing (not visual/DPI acceptance)')
    Click 126
    Wait-Until { [AmberLProbeWindow]::Send((Control 124), 326) -eq 1 } 'detach'
    $saved = Get-Content -LiteralPath $registryPath -Raw -Encoding utf8 | ConvertFrom-Json
    if ($saved.entries.Count -ne 0) { throw 'Detach not committed' }
    Close-Probe
    if ((Snapshot) -cne $before) { throw 'Game fixture changed' }
    $checks.Add('Detach preserves every fixture file path and SHA-256')
    $result = [ordered]@{passed=$true; executable_sha256=(Get-FileHash $executablePath -Algorithm SHA256).Hash; fixture_profiles=120; library_id=$registeredId; checks=$checks; limitations='Hidden-window message automation; not real IME, accessibility, rendered layout, DPI, game or user acceptance.'}
    $result | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $testRoot 'result.json') -Encoding utf8
    $result | ConvertTo-Json -Depth 5
    Write-Output "Evidence: $testRoot"
} finally {
    if ($script:probe -and -not $script:probe.HasExited) {
        # Only the specific process launched by this test, never a process-name search.
        try { [AmberLProbeWindow]::Send($script:window,16) | Out-Null } catch { }
        if (-not $script:probe.WaitForExit(3000)) { $script:probe.Kill(); $script:probe.WaitForExit() }
    }
}
