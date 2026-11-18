# Loader installation verification / 加载器安装验证

2026-11-13, Windows 11 x64, Microsoft OpenJDK 25.0.4 x64, Minecraft 26.3,
4096 MiB explicitly selected. Exact packaged commit/checksums are recorded in PR #1.
All installs used new disposable AmberL directories; no existing AMBER library or
account store was used. Existing AmberL test authorization was reused through DPAPI.

| Component | Provider version | Real installation + launch preflight | Game main menu / normal exit |
| --- | --- | --- | --- |
| Fabric | 0.20.1 | Passed, 5233 resources verified | Passed / exit 0 |
| Quilt | 0.31.0-beta.1 | Passed, 5235 resources verified | Passed / exit 0 |
| Forge | 26.3-66.1.8 | Official installer + client processors passed, 5276 resources | Passed / exit 0 |
| NeoForge | 26.3.0.103 | Official installer + client processors passed, 5256 resources | Passed / exit 0 |

Fabric/Quilt logs explicitly report their loader identity. Forge main menu reports
66.1.8 and two loaded mods; NeoForge reports 26.3.0.103 and two mods. Each launch revalidated the complete immutable receipt,
replayed the loader merge against newly verified Mojang metadata, refreshed the saved
Microsoft session and verified game ownership before starting the selected Java.
Tests exercised the same core APIs used by the desktop coordinator. This is not a
claim that every UI installation combination or every historical loader was played.
No world creation/save acceptance was repeated in this iteration.

## Automated coverage

94 workspace checks (including prior auth, DPAPI, directory identity, download,
transaction and launch tests), Rust formatting and strict Clippy. New checks cover
catalog identity/duplicates/types, Maven traversal and origin rejection, checksum
failure, cancellation, stale loader selections, library replacement and profile
identity, immutable metadata tampering, icon/name bounds and modern NeoForge detection.
The native dialog screenshots and compact-window checks are in `design-qa.md`.

## Limits

Only modern loader metadata/entrypoints are supported. Legacy Forge installers that
lack `--installClient`, quoted legacy profiles, OptiFine/LiteLoader, modpack setup,
Fabric API/QSL download and modifying existing instances remain outside this change.
Forge/NeoForge execute official Java installer code in an isolated *directory*, not
an OS sandbox. Their temporary workspace/logs are retained; the byte budget applies
to validated resource plans and final installation, not a hard disk quota while a
third-party installer runs. Installation cancellation terminates only its owned
installer process. Running-game cancellation still requests graceful shutdown.

Official interfaces: [Fabric Meta](https://github.com/FabricMC/fabric-meta),
[Quilt Meta](https://meta.quiltmc.org/),
[Forge installer](https://github.com/MinecraftForge/Installer),
[NeoForge Maven](https://maven.neoforged.net/releases/net/neoforged/neoforge/).
