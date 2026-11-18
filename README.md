# AmberL / Ambrum

A lightweight, carefully designed Minecraft game and multiplayer workstation.
Ambrum is its UI-independent Rust core. **First-version development candidate for Windows x64; user acceptance and stable publication are separate steps.**

轻量、精致、易用的 Minecraft 游戏与联机工作站。减少必须做的决定，保留必要选择。
**当前是开发预览：原生界面已接入微软账户、安全存储、Java 选择、组件安装、原版启动，以及服务端和邀请联机；跨设备联机和完整兼容性验收尚未完成。**

Start with the Windows test installer or `launcher/amberl.exe` in the UI ZIP from
[Draft PR #1](https://github.com/ayr1k/AmberL/pull/1). The PR identifies the exact tested
revision and downloadable builds. Runtime DLLs and notices are bundled; keep the whole
ZIP together. The core/probe ZIP is for diagnostics, not the main launcher.

首版开发候选包含原版与 Fabric/Quilt/现代 Forge/NeoForge 安装、微软账户、Java 下载与选择、
内容搜索和恢复、游戏库副本、独立联机工作区、服务端与邀请及 FRP。首次使用可先登录微软账户，
安装游戏并选择兼容 Java；联机页面分别管理服务端和连接，不把进程启动等同于已可加入。
外部库默认只读，支持范围见 [副本能力表](docs/IMPORT.md)。

See [completion and deferred acceptance](docs/FIRST-VERSION.md),
[multiplayer](docs/MULTIPLAYER.md), [content/Java](docs/CONTENT.md),
[installer](docs/INSTALLER.md), and [manual updates](docs/UPDATES.md).
Two-device/NAT, another clean Windows machine, full accessibility and personal-library
acceptance are deferred. No merge or stable release is implied.

## Current milestone / 当前进度

- Read-only AMBER-managed and `.minecraft` library inspection.
- Explicit library/client/server identities; conflicting or absent isolation stays unresolved.
- Official profile, per-version PCL/HMCL directory evidence; bounded JSON and inheritance.
- Cancellable background scan, virtual native Windows list, search and inspectable diagnostics.
- Persistent Windows library list: preview, attach, reopen by stable ID and detach without deleting game files.
- Handle-checked directory identity, exclusive registry lock and staged app-only saves.
- Streaming HTTPS artifact cache with size + SHA-1/SHA-256 verification, cancellation,
  explicit retry and corrupt-cache recovery (core/CLI; not game installation).
- Bounded Java discovery/inspection and explicit executable selection with exact official major/x64 checks.
- Read-only Vanilla version/asset plans with platform rules, Java requirements,
  separate native archives, exact argument boundaries and conflicting-path rejection.
- Batch Vanilla cache preparation: verified asset-index discovery, explicit byte budget,
  cross-resource deduplication, cancellation and retries that reuse verified cache files.
- Official version discovery with bounded HTTPS metadata and catalog SHA-1 verification.
- New isolated Vanilla directory installation from verified cache, bounded native ZIP
  extraction, explicit disk budget and recoverable no-replacement directory commit.
- Microsoft device login, Xbox/Minecraft ownership/profile checks and independent Windows DPAPI storage.
- Authenticated Vanilla sessions with pinned installed resources, separate arguments, own-process exit and redacted output.
- Public Microsoft application settings match AMBER; no account credentials are imported.

The original Windows UI remains a **system-control comparison baseline**. Separate
[Slint and WinUI 3 samples](docs/UI-COMPARISON.md) now exercise the same Rust model;
input/accessibility/DPI acceptance remains open. Slint is the development-candidate
delivery host; final comparison acceptance remains deferred. No WebView or Node process.
The [integrated launcher preview](docs/NATIVE-UI.md) uses the selected dark visual direction,
full-window scenery and cached Gaussian glass panels. Open `launcher/amberl.exe` in the
UI package for real account/Java/library/install/launch/content/multiplayer controls. A detected loader
is **not** a claim of launch compatibility.

## Try the development probe / 试用开发样机

Unzip the development package and run `amberl-native-probe.exe`. Choose a **copy** of
a game library, scan it, then click **添加此库**. Select a saved library in the dropdown
to scan it again. **移除登记** removes only the registration, never the game files.
The independent registry lives at `%LOCALAPPDATA%/io.github.ayr1k.amberl/libraries.json`.
Path replacement or damaged configuration produces an error and preserves the old record.

解压开发包，运行 `amberl-native-probe.exe`：选择测试库副本 → 扫描 → 添加此库。
关闭后重新打开，可从已添加的库列表继续扫描。移除登记不会删除游戏文件。
Core 包保留扫描基线和命令行；UI 包中的 `launcher/amberl.exe` 是新的完整背景与毛玻璃界面。
ZIP 可免安装运行，也提供独立的用户级测试安装器。联机功能位于 UI 包的新启动器；原始扫描窗口不是新启动器界面。
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
and checks cache reuse. The resumed Windows test passed actual HTTPS and cache reuse;
the earlier DNS failure is retained only as history in the validation record.

新增下载核心与命令行测试入口：暂存、取消、失败重试、大小和哈希校验、缓存复用。
同一下载核心已用于游戏安装与内容下载；以下命令行说明保留独立诊断入口。

Metadata planning (Windows x64, no network or game writes):
`amberl-inspect plan-vanilla <metadata.json> <windows-os-version>` and
`amberl-inspect plan-assets <asset-index.json>` print JSON plans.
Use an explicit OS version, for example `10.0.26100`; this is not a readiness check.
Only standalone Vanilla metadata with explicit download descriptors is supported.
Inherited/loader metadata and custom entrypoints return errors instead of partial plans.

版本与资源索引可生成只读计划，保留 Java 要求、平台规则和资源映射；
未知 Java 要求保持未知。计划成功不代表完成下载、安装或游戏启动。

To populate only the app cache, run
`amberl-inspect prepare-vanilla <app-directory> <metadata.json> <windows-os-version> <max-bytes>`.
Use metadata from a trusted provider and choose the maximum unique resource bytes explicitly.
The budget includes cache hits; discovering the complete plan may require downloading the
asset index first. Failures retain completed cache objects for a verified retry. The task is
sequential, and success means cached resources, not an installed or launchable game.
`test-resource-plan.ps1` tests the packaged CLI with tiny synthetic metadata and seeded
cache files without a network request. Live HTTPS has separate tests.

新增批量缓存准备：先校验索引，再检查总量上限，支持去重、取消与失败后重试。
只写 AmberL 缓存，不安装或修改游戏；测试使用合成数据，不等同于联网验收。

## New-directory installation / 新目录安装验证

Use separate existing empty directories for app data and an installation parent.
`resolve-vanilla <app-directory> <version>` fetches the official catalog and verifies
the selected metadata. Use its returned `metadata_path` with `prepare-vanilla`, then:

```text
amberl-inspect install-vanilla <app-directory> <metadata.json> <windows-os-version> <install-parent> <new-name> <max-bytes>
amberl-inspect recover-install <install-parent> <operation-id>
```

The installer creates a **new** isolated directory; an existing destination is always
rejected. It copies pinned, reverified resources rather than linking the cache. The
budget counts every materialized copy, metadata and expanded natives. An interrupted
operation retains its stage and reports its ID; recovery commits only complete,
reverified stages. Incomplete stages are preserved and a new attempt can reuse cache.
Installation success is not game launch or account acceptance.

新目录安装使用独立暂存、真实目录锁、文件复验和不覆盖的目录提交；默认添加外部库
仍为只读。损坏／不完整暂存不会自动删除或当作可运行实例。详见
[安装与恢复边界](docs/INSTALLATION.md)。`test-install.ps1` 使用合成输入；
`test-install-network.ps1` 才会通过 Ambrum 实际下载官方资源并验证安装副本。

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
- [Installation and recovery / 安装与恢复](docs/INSTALLATION.md)
- [Multiplayer preview / 服务端与邀请联机](docs/MULTIPLAYER.md)
- [Third-party notices](THIRD_PARTY_NOTICES.md)

Source: MPL-2.0. AMBER reference baseline:
[`3af52c7`](https://github.com/ayr1k/amber-launcher/commit/3af52c7a3fbd5e301ed12d204b3e8fa3d9ae69b2).
The core is newly written Rust informed by AMBER's layouts, constraints and regressions;
AMBER's Electron/Node service graph is not embedded. No stable release is authorized.


## Accounts and actual launch / 账户与真实启动

See [login, Java and launch instructions](docs/AUTH-LAUNCH.md). Windows device login,
protected account reopen/refresh, explicit Java 25 and Vanilla 26.3 menu/world/save/exit
have been tested on a new disposable instance. The original CLI vertical slice remains
available; these flows are also integrated in the native launcher. No original game/account
data was reused.

### Native installation wizard (development)

The glass launcher now has integrated window controls, a searchable official game
version list, Fabric / Quilt / modern Forge / NeoForge installation, and creation-time
custom names with four AMBER icons. See [native UI](docs/NATIVE-UI.md) for exact limits
and [loader verification](docs/LOADER-VERIFICATION.md) for tested combinations.
无边框窗口、分步版本/组件选择和内置实例图标仍属于开发预览；不代表正式发布。

### Multiplayer and saved launch choices / 联机与启动配置保存

The first usable release includes multiplayer. This development slice adds independent
Vanilla server lifecycle/console, verified EasyTier invitation rooms, status queries and
Quick Play joining where supported. A real authenticated Vanilla 26.3 client entered a
server through two room peers on one PC and exited normally; this does not establish
two-device or public-relay acceptance. Per-game Java/account ID/memory choices persist.
See [the multiplayer guide](docs/MULTIPLAYER.md) for host/join steps and open gates.

The current development work also adds actual modded servers, verified server
backups/new-copy recovery, stopped-server configuration, official Java downloads and
recoverable Modrinth content groups with optional dependency choices. See
[content/Java](docs/CONTENT.md), [test installer](docs/INSTALLER.md) and the
[full first-version completion ledger](docs/FIRST-VERSION.md). This is still a
development candidate; two-device and other user/device-dependent acceptance remain deferred.
