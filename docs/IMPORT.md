# Independent game copies / 游戏库独立副本

Select an external library's Vanilla instance, open **Configure → Library and copy**,
review its paths/isolation evidence, and choose a separate existing parent folder and
new name. The name also becomes the copy's initial display name. Shared directories
require an explicit checkbox: their data belongs to the directory, not one profile.
Unknown or conflicting isolation, modded loaders and AMBER-managed layouts cannot
be imported through this flow. External sources are never launched or modified.

选中已有游戏库中的原版实例，在 **配置 → 游戏库详情与副本** 查看来源和隔离依据。
选择与原库分离的已有父目录，填写新名称。共享目录必须明确勾选，会复制其中全部
存档与设置，无法按启动配置区分归属。隔离未知、冲突或模组加载器暂不支持导入。
请先关闭使用来源目录的游戏。副本创建后可以独立启动、管理内容、改名和备份。

## Copied data

- Files under `saves`, `resourcepacks`, `shaderpacks`, `screenshots`, and `config`.
- `options.txt`, `optionsof.txt`, `optionsshaders.txt`, and `servers.dat`.
- Official Minecraft executable resources are downloaded/revalidated independently;
  source JARs, libraries, launch arguments and patched executables are not trusted inputs.

Accounts, tokens, launcher settings, mods and other unlisted files are excluded.
Empty directories are not preserved. Resource/shader files are copied as data;
this does not install the loaders or mods needed to render shaders. Shared-world
compatibility is not inferred: a Vanilla profile can share a directory with modded
worlds, which must not be opened without their required mods. The original remains intact.

仅复制上述游戏数据，不复制账户、启动器配置、模组或未列出的文件。空目录不保留。
共享目录可能含模组世界；创建原版副本不代表这些世界与原版兼容。游戏版本必须能
对应官方目录；自定义版本别名、第三方补丁和整合包不在这次支持范围内。

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
