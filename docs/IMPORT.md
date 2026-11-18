# Independent game copies / 游戏库独立副本

Select an external instance and open **Configure → Library and copy**. Review the
resolved Minecraft version, exact loader version and isolation evidence; choose a
separate existing parent folder and a new copy name. Original sources stay read-only.
Shared directories require explicit acknowledgement. Modded copies additionally require
confirmation that the source mods/scripts are trusted, because they execute in Minecraft.
Forge/NeoForge rebuilding requires compatible Java selected in the form.

选中外部实例，在 **配置 → 游戏库详情与副本** 核对来源、游戏和加载器版本。
新副本必须使用与原库分离的位置。共享目录需确认复制整个目录的数据；模组实例
还需确认信任来源。Forge / NeoForge 重建需要填写兼容的 Java 路径。

## Supported capability matrix

| Source | Inspect | Independent copy | Launch/write original |
|---|---|---|---|
| Official/PCL/HMCL `.minecraft`, known Vanilla version and isolation | Yes | Yes | No |
| Known Fabric/Quilt or supported modern Forge/NeoForge, exact game/loader identity | Yes | Rebuild official resources, copy trusted mod data | No |
| Renamed profile with `clientVersion`, game patch or consistent inheritance/loader evidence | Yes | If it resolves to official game and loader catalogs | No |
| Shared data directory | Yes | Explicit shared-data acknowledgement required | No |
| Unknown/conflicting isolation or conflicting loader versions | Yes, with issue | Blocked | No |
| Custom patched executables, unsupported legacy loader, unresolved version alias | Limited | Blocked; no guessed replacement | No |
| AMBER `instances` layout | Read-only metadata inspection | Not supported by this copy path | No |
| Reparse/symlink/hard-link filesystem aliases | Rejected where unsafe | Blocked | No |

The supported copy reconstructs runtime resources from official providers. Source JARs,
launcher JVM arguments, Java agents, custom library URLs and executable patches are not
reused. A profile name is distinct from its actual game version. FML's own version is
not mistaken for a NeoForge version. Conflicting identity evidence stops copying.

## Copied data

- `saves`, `resourcepacks`, `shaderpacks`, `screenshots`, `config`.
- `options.txt`, `optionsof.txt`, `optionsshaders.txt`, `servers.dat`.
- For confirmed modded instances only: `mods`, `defaultconfigs`, `kubejs`, `scripts`.

Accounts, tokens, launcher settings and all unlisted files are excluded. Empty directories
are not preserved. Imported mods remain user files, not invented Modrinth installation
groups; content management will not overwrite them. Whole-instance snapshots preserve
them. Additional mod-specific data directories are not silently included. Resource/shader
files alone do not install a shader renderer. Shared directories can contain worlds from
other versions/modpacks: copying does not prove every world is compatible.

仅复制列出的游戏数据，不复制账户、启动器配置或未列出的文件。导入模组保持为
用户文件，不伪造平台安装记录，也不会被内容安装覆盖。整实例备份会保留这些文件。
共享目录中的存档可能需要其他版本或模组，不能据此认定全部兼容。

## Boundaries and recovery

The registered library/profile, game version, data directory and shared/isolated
choice are captured and checked again before work. Directory handles prevent path
replacement; ordinary source files are pinned read-only. Links, hard links, unsupported
paths, changed selection and busy source files stop the operation. Capture is bounded
to 200,000 entries/32 path components and 16 GiB of data; the complete installed copy
including game resources must also fit the 16 GiB budget. Download-cache space is separate.

All output uses the existing staged, checksum-verified new-directory transaction.
No existing target is replaced. Cancel/failure preserves the source and incomplete
stages; a complete sealed stage can use the existing installation-recovery command.
Retry may reuse verified downloads. This is not a filesystem-wide atomic snapshot
against third-party writers; close the source game first. Unrelated new source files
created after capture are not added retroactively.

## Evidence

Synthetic official/PCL/HMCL directory samples cover known isolated input, shared
confirmation, unknown isolation, changed game-directory selection, hard links,
target collision and cancellation. Copied worlds remain mutable while executable
resources retain installation checks. Account/launcher fixture files are excluded.

A disposable HMCL-style 26.3 sample containing a previously created QA world was
imported through the native UI. Initial download interruption was recoverable by
retry. All 42 save files and game settings matched before launch; all 46 source
files stayed unchanged. The copied world was actually entered, saved and exited
normally (Java 25, exit 0); the source still matched afterward. This is a controlled
layout fixture, not a claim that all versions of HMCL/PCL or arbitrary user libraries
are accepted. Package/PR records identify the tested source revision.

The final native form was also checked at 1002 × 702: shared confirmation gates
creation, the entered name appears on the completed shared copy, and unknown
isolation disables both destination fields and creation. All tests used disposable
libraries. The 126-test workspace suite and both targeted import regressions passed.

Current development evidence: exact Fabric/Quilt/Forge/NeoForge identities and renamed
profiles covered by scanner fixtures; conflict, changed selection, missing mod consent,
wrong destination loader and excluded launcher state covered by import regressions.
A disposable HMCL-style renamed Fabric 26.3 / 0.20.1 profile was rebuilt using official
resources, copying three known test mod JARs. Minecraft displayed 51 loaded mods,
entered a new QA world, saved and exited with code 0. Seven source fixture files retained
their hashes; an injected fixture Java-agent argument was not copied to launch metadata.
This is real Fabric evidence, not a claim that every arbitrary modpack is compatible.
