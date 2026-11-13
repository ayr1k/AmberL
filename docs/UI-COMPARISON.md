# Stage B: native UI comparison / 原生界面对照样机

These are two working comparison hosts, not the final launcher UI. The existing
authenticated CLI launch slice remains available separately. No framework is selected
yet: Chinese IME composition, reader acceptance, DPI transitions and clean-machine
deployment still gate the decision.

两套样机调用相同的 Ambrum 只读扫描和交互模型，可扫描测试库、筛选与选择条目、
查看隔离证据、编辑中文文本、运行和取消模拟任务。不会登录、安装或启动游戏。
此前已验收的微软账户、DPAPI、Java 与原版启动功能继续保留在 CLI 测试包中。

| Host | Pinned implementation | Process model |
| --- | --- | --- |
| Slint | 1.18.1, winit, software renderer, accessibility enabled | Rust executable, shared model linked directly |
| WinUI 3 | Windows App SDK components 2.6.5, C++/WinRT 3.0.260818.1 | Native executable + Rust DLL in the same process |

Both use `amberl-ui-model`; only WinUI crosses `amberl-ui-bridge`'s bounded UTF-8
JSON ABI. No UI types enter Ambrum. A response-size/read retry cannot execute a
command twice. Every row has a core identity; filtering cannot select a hidden or
stale row. Scan cancellation discards even an already queued successful result.
Both poll at 50 ms. The explicitly synthetic task generates 100 lines/second for
10 seconds and retains 120 lines. It is not download or game-readiness evidence.

## Run the test package

Windows x64; Windows 10 1809 or newer is the SDK minimum, not a tested compatibility
claim. Actual checks used Windows 11. WinUI includes its app-local Windows App SDK
runtime; the Microsoft Visual C++ 2015–2022 x64 runtime must already be installed.
It does not install or change system runtimes. No account/game files are packaged.

Extract the complete ZIP. Open `slint/amberl-slint-probe.exe` or
`winui/amberl-winui-probe.exe`, enter the same synthetic library path, then Scan.
Keep WinUI's DLLs and `resources.pri` beside its executable. A new test library can
be generated from the extracted package root in PowerShell 7:

```powershell
./scripts/make-fixture.ps1 -Destination local/comparison-fixture -Count 3000
```

Use only disposable synthetic/copied libraries for acceptance. `TEST-REVISION.txt`
identifies the exact source, `AmberL-source.zip` contains that committed tree and
`SHA256SUMS.txt` covers the delivered files. This is an unpack-and-run development
package, not a signed production installer.

## Observations, 2026-11-13

- Both actual windows opened and scanned the same 3,000-profile Chinese fixture.
  First observed scan: Slint 14,882 ms, WinUI 3,874 ms. These sequential observations
  have different filesystem cache warmth and are **not a framework speed ranking**.
- WinUI Chinese text filtering selected the expected single profile; choosing it
  showed its actual isolated directory and HMCL evidence. Synthetic task cancel
  stopped at 84% and retained a bounded log.
- Accessibility trees exposed named path/filter/list/detail/input/task/log controls
  in both hosts. This is not Narrator or full keyboard/IME acceptance.
- One post-scan process sample: Slint working set 34.8 MiB / private 14.1 MiB;
  WinUI 127.1 MiB / private 104.6 MiB. These are diagnostic snapshots on this machine,
  not controlled startup/idle benchmarks or a release memory promise.
- Five new model/ABI tests cover cancelled queued results, failed scans preserving
  rows, stable selection across filters, stale identity/query bounds, log pressure,
  command response replay, invalid input and separate model handles.
- Fixed WinUI startup resource failure by implementing `IXamlMetadataProvider`
  through the SDK provider; explicitly generate `resources.pri` for unpackaged use.

Not yet accepted: actual IME pre-edit/candidate placement, clipboard/undo and full
keyboard traversal, Narrator/high contrast, 100/125/150/200% DPI and monitor switching,
controlled rendering/idle benchmarks, and clean-machine runtime deployment. Literal
Chinese text injection does not verify IME composition. Stage B remains incomplete.

## Reproduce builds

Install Rust MSVC, VS 2022 v143 C++ tools and Windows SDK 10.0.26100.0. No .NET SDK
or UWP workload is needed. `scripts/build-ui-comparison.ps1` builds the Rust DLL,
isolated locked Slint workspace, and locked NuGet native WinUI project. Use
`-Offline` only after dependencies have been restored. From a clean committed tree,
`scripts/package-ui-comparison.ps1` builds and collects runtime assets, source,
original third-party notices and file hashes. The CI UI job publishes this ZIP
separately from the existing authenticated CLI/native-baseline test package.

Slint's desktop sample uses the pinned Royalty-free 2.0 option. The accessible
About button exposes the standard AboutSlint widget and attribution. Original license
texts accompany the package. Ambrum and the host source remain MPL-2.0; no third-party
API exposes Slint internals. WinUI's WebView2 NuGet metadata is needed to generate
complete C++ projections; this sample never instantiates a WebView or browser runtime.

References: [Slint license options](https://github.com/slint-ui/slint/blob/v1.18.1/LICENSE.md),
[Windows App SDK self-contained deployment](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/self-contained-deploy/deploy-self-contained-apps),
[programmatic WinUI metadata provider](https://github.com/microsoft/microsoft-ui-xaml/discussions/8151).
