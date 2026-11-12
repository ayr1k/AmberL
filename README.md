# AmberL / Ambrum

A lightweight, carefully designed Minecraft game and multiplayer workstation.
Ambrum is its UI-independent Rust core. **Early development: not a usable launcher yet.**

轻量、精致、易用的 Minecraft 游戏与联机工作站。减少必须做的决定，保留必要选择。
**当前是技术样机，不具备完整安装、登录、游戏启动或联机能力。**

## Current milestone / 当前进度

- Read-only AMBER-managed and `.minecraft` library inspection.
- Explicit library/client/server identities; conflicting or absent isolation stays unresolved.
- Official profile, per-version PCL/HMCL directory evidence; bounded JSON and inheritance.
- Cancellable background scan, virtual native Windows list, search and inspectable diagnostics.
- Persistent Windows library list: preview, attach, reopen by stable ID and detach without deleting game files.
- Handle-checked directory identity, exclusive registry lock and staged app-only saves.
- Streaming HTTPS artifact cache with size + SHA-1/SHA-256 verification, cancellation,
  explicit retry and corrupt-cache recovery (core/CLI; not game installation).
- CLI Java executable inspection with timeout, cancellation and bounded output.
- Read-only Vanilla version/asset plans with platform rules, Java requirements,
  separate native archives, exact argument boundaries and conflicting-path rejection.
- Public Microsoft application settings match AMBER; no account credentials are imported.

The Windows UI is a **system-control comparison baseline**, not the final design and
not a substitute for the Slint/WinUI selection experiment. No WebView or Node process.
All game mutations, login, game/content download workflows and process supervision
remain future work. A detected loader is **not** a claim of launch compatibility.

## Try the development probe / 试用开发样机

Unzip the development package and run `amberl-native-probe.exe`. Choose a **copy** of
a game library, scan it, then click **添加此库**. Select a saved library in the dropdown
to scan it again. **移除登记** removes only the registration, never the game files.
The independent registry lives at `%LOCALAPPDATA%/io.github.ayr1k.amberl/libraries.json`.
Path replacement or damaged configuration produces an error and preserves the old record.

解压开发包，运行 `amberl-native-probe.exe`：选择测试库副本 → 扫描 → 添加此库。
关闭后重新打开，可从已添加的库列表继续扫描。移除登记不会删除游戏文件。
当前包是免安装技术样机，不能登录、安装和启动游戏或联机；外观不是最终设计。
包内包含对应源码压缩包、修订号、校验值和依赖许可。

For isolated testing, create an empty app-data directory and pass
`--data-dir <directory>` to the native probe. Do not use a game directory as app data.
CLI registry operations: `amberl-inspect libraries <app-directory>` with optional
`add <library>`, `scan <registered-id>` or `remove <registered-id>`.

Developer download probe (Windows, app cache only):
`amberl-inspect fetch <app-directory> <https-url> <sha1:hex|sha256:hex> <bytes>`.
Progress is on stderr; success JSON is on stdout. Ctrl+C requests cancellation; a blocked
OS call returns on its network timeout. Retry explicitly with the same descriptor.
Expected hashes must come from trusted provider metadata; SHA-1 compatibility is not
a signature/authenticity guarantee. No bearer credentials are accepted by this downloader.
The package includes `test-download-network.ps1`, which fetches a pinned public license
and checks cache reuse. This host's live attempt was blocked by DNS (Windows 12007).

新增下载核心与命令行测试入口：暂存、取消、失败重试、大小和哈希校验、缓存复用。
尚未接入游戏安装或内容搜索界面；本机真实网络请求仍因 DNS 解析失败而未通过。

Metadata planning (Windows x64, no network or game writes):
`amberl-inspect plan-vanilla <metadata.json> <windows-os-version>` and
`amberl-inspect plan-assets <asset-index.json>` print JSON plans.
Use an explicit OS version, for example `10.0.26100`; this is not a readiness check.
Only standalone Vanilla metadata with explicit download descriptors is supported.
Inherited/loader metadata and custom entrypoints return errors instead of partial plans.

版本与资源索引可生成只读计划，保留 Java 要求、平台规则和资源映射；
未知 Java 要求保持未知。计划成功不代表完成下载、安装或游戏启动。

## Build / 构建

Rust stable (2024 edition) and Windows C++ build tools/SDK for the Windows UI:

```powershell
cargo test --workspace --locked
cargo build --workspace --release --locked
./target/release/amberl-native-probe.exe
./target/release/amberl-inspect.exe scan 'D:\Games\.minecraft'
./target/release/amberl-inspect.exe java 'C:\Java\bin\java.exe'
./scripts/test-native-probe.ps1
```

Use disposable fixtures or verified copies of game libraries. The current probe is
read-only, but this does not turn incomplete inspection into a security guarantee for
future write operations. Do not import personal tokens, raw logs or caches into this repo.

## Project documents

- [Product and stages / 产品规格与阶段](docs/PLAN.md)
- [Architecture and decisions / 架构与决策](docs/ARCHITECTURE.md)
- [Compatibility and evidence / 兼容范围与证据](docs/COMPATIBILITY.md)
- [Validation record / 验证记录](docs/VALIDATION.md)
- [Third-party notices](THIRD_PARTY_NOTICES.md)

Source: MPL-2.0. AMBER reference baseline:
[`3af52c7`](https://github.com/ayr1k/amber-launcher/commit/3af52c7a3fbd5e301ed12d204b3e8fa3d9ae69b2).
The core is newly written Rust informed by AMBER's layouts, constraints and regressions;
AMBER's Electron/Node service graph is not embedded. No stable release is authorized.

