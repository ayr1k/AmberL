# Instances and backups / 实例与备份

Select an AmberL-created game, then **配置 → 实例与备份**. Change its display name
and one of the four bundled icons without rewriting Minecraft version metadata.
**移除游戏（可恢复）** hides the instance and retains every file. Restore its display
from **备份与恢复**; this is not disk-space cleanup. Stop the game before editing,
removing or backing it up. External/imported libraries remain read-only.

选择 AmberL 创建的游戏，进入 **配置 → 实例与备份**，可修改显示名称和内置图标。
移除只隐藏条目，保留全部文件；在 **备份与恢复** 中可恢复显示。请先关闭游戏。

## Full snapshot and new-copy restoration

**创建完整备份** captures saves, configuration, installed resources and other ordinary
files in the owned game directory. Snapshots are stored under the independent app
directory's `client-backups`, with a 16 GiB / 210,000-file limit per snapshot and
256 completed snapshots. It needs additional disk space; no automatic pruning occurs.
Links, overlapping app/game paths and active AmberL sessions are rejected. Cancellation
or failure preserves an incomplete `stage-*` directory; incomplete stages are not
offered as recoverable snapshots. A snapshot is not protection against disk failure:
this preview stores it on the configured local app drive.

Choose a completed snapshot, an existing parent directory separate from app data,
and a **new** folder name. Restoration verifies each file's size and SHA-256, stages
the entire copy, then commits and registers a new independent instance. Existing
targets are refused. A registration failure reports the completed copy's path so
it can be attached again. The source instance and snapshot remain unchanged.

完整备份包含实际存档、配置和安装资源，单份上限 16 GiB。恢复必须使用新的文件夹，
逐个校验文件后创建独立副本，不覆盖原游戏。失败现场保留，未完成的备份不会进入列表。
备份位于应用数据目录；本机备份不等于异地备份。

Immutable installation resources retain startup verification after restoration;
worlds and other gameplay files remain writable. Installation receipts are regenerated
for the new directory identity. App-level accounts, launch preferences and content
management group records are not cloned. Restored mod/resource files are retained as
unmanaged files, so a new content plan cannot overwrite conflicting files silently.
Use this revision or a newer compatible preview to open a restored instance: older
previews do not understand mutable-file records in its installation receipt.

## Evidence

Synthetic tests cover writable restored saves, immutable-resource tampering, active
session refusal, recoverable removal, corrupt snapshots, cancellation and an existing
destination. A disposable Fabric 26.3 installation was fully copied, restored and
authenticated into the real client: the main menu displayed 51 mods, then normal exit
returned code 0. Chinese naming, icon selection, removal/return and new-copy restore
were exercised in the native UI, including its 1002 × 702 layout. Chinese text injection
does not establish IME pre-edit acceptance. No original AMBER library was modified.
