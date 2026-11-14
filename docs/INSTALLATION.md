# New-directory Vanilla installation / 原版新目录安装

This is a stage B CLI slice. It materializes a standalone Vanilla resource plan into
a new isolated directory. It does not authenticate, execute Java/Minecraft, update an
existing instance, adopt a third-party library for writing, or finish the product UI.

## Inputs and identity

- App data/cache and installation parent must be separate, non-overlapping directories.
- The parent must not itself look like an existing game library, saves or app-cache root.
- The caller captures parent, new directory name, metadata, OS version and byte budget.
  A later UI selection never changes them. The API returns a client identity and game path.
- `resolve-vanilla` obtains a bounded official catalog through Ambrum's WinHTTP adapter,
  accepts only known official metadata origins, and verifies catalog SHA-1 and version ID.
  SHA-1 supplies Mojang compatibility, not an independent signature or ownership proof.
  Discovery refreshes catalog/metadata on each call; the verified metadata cache can hit.
- Arbitrary input metadata remains the caller's trust decision. Installation is not
  authority to execute custom code. The existing standalone Vanilla planner limits apply.

## Write and commit boundaries

1. Parse version and verified asset index; validate all file/directory paths and aliases.
2. Count all materialized bytes, including duplicated virtual/resource layouts, normalized
   version JSON and expanded native entries. Cache hits do not reduce the budget. Journals
   add bounded overhead (up to 64 MiB plus a small pending record) outside the payload budget.
3. Check every cache resource before creating an installation stage. Each copy reopens
   and hashes a handle denying write/delete sharing, rejecting reparses and hard links.
4. Lock the actual installation parent, conservatively serializing new installations in
   that parent. All dependencies stay inside the new instance; there are no shared game
   resources outside it. Pin parent ancestors and newly created stage directories.
5. Stage under `.amberl-install-<operation-id>`. Files are create-new, flushed, hashed,
   and never hardlinked to cache. A pending journal binds target, parent and stage identity.
6. After all payloads succeed, write a complete SHA-256 receipt. Recheck every file, reject
   unexpected entries, recheck identities and destination conflict, and check cancellation.
7. Rename the still-open stage directory with `SetFileInformationByHandle`, replacement
   disabled. Only successful rename emits Complete. No directory is deleted or overwritten.

An external same-user program can modify files after handles close or after installation.
This is not a permanent immutable snapshot or a defense against a hostile account owner.
Power-loss durability across storage controllers, removable/network filesystems and ReFS
is unverified. The tested boundary is a local Windows filesystem and normal process faults.

## Native archives and older assets

ZIP/Deflate uses zip 9.5.1 + flate2/zlib-rs. Input archives are limited to 16 MiB each,
4,096 entries each and 256 MiB combined expanded native bytes, also subject to the caller's
installation budget. No custom archive decompressor is used. Validate names before writes:
no traversal, absolute/drive/UNC paths, backslashes, alternate streams, reserved names,
symlinks, encryption, special Unix file types or case/file-directory collisions. Native
entries go under `natives/`; `META-INF/` is excluded. CRC errors prevent completion.
Archives with unsupported compression or provider-specific extraction rules are rejected.
Virtual assets and `map_to_resources` use independent verified copies with budget accounting.
These capabilities do not establish that old Java, native windows or old game versions run.

## Failure and recovery / 中断恢复

The CLI progress line contains the operation ID. Cancellation and ordinary failures leave
the uniquely named stage intact; an existing target and other instances stay unchanged.
No automatic recursive cleanup runs. Completed cache resources remain reusable.

`recover-install <install-parent> <operation-id>` acquires the same parent lock, checks
original parent/stage identities and schema, and requires a complete receipt. It rehashes
every recorded file and rejects additions, missing files, corruption, links, active writers,
changed identities or an existing target. Only then can it finish the no-replacement rename.
An incomplete stage cannot be recovered as installed; rerun installation to a new stage.
Inspection/discard UI and resuming partially written game files are future work. Keep an
unrecognized stage for inspection rather than guessing its ownership or deleting it.

安装失败会保留 `.amberl-install-<操作 ID>`，不会覆盖同名实例，也不会自动删除暂存。
只有完整且重新验证通过的暂存能够用 `recover-install` 完成提交；未完成的暂存不能
冒充安装成功。重新安装可复用已经校验的应用缓存。成功安装仍不代表登录、游戏启动
或兼容性验收成功；请勿把本样机用于原始存档或私人账户数据。
