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
- CLI Java executable inspection with timeout, cancellation and bounded output.
- Public Microsoft application settings match AMBER; no account credentials are imported.

The Windows UI is a **system-control comparison baseline**, not the final design and
not a substitute for the Slint/WinUI selection experiment. No WebView or Node process.
Library attachment persistence, all mutations, login, downloads and process supervision
remain future work. A detected loader is **not** a claim of launch compatibility.

## Build / 构建

Rust stable (2024 edition) and Windows C++ build tools/SDK for the Windows UI:

```powershell
cargo test --workspace --locked
cargo build --workspace --release --locked
./target/release/amberl-native-probe.exe
./target/release/amberl-inspect.exe scan 'D:\Games\.minecraft'
./target/release/amberl-inspect.exe java 'C:\Java\bin\java.exe'
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

