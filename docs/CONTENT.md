# Owned content and Java runtimes / 内容与 Java 管理

Game configuration opens **模组与资源**; stopped modded servers open
**管理模组与插件**. Search Modrinth, select a project and compatible version,
preview the exact required dependency/file list, then install. Optional dependencies
are individually selectable and trigger a new complete plan check. Resource packs
and shaders are client-only. Shader installation does not install a renderer.

内容仅写入 AmberL 创建、目录身份和会话锁均通过检查的实例。外部游戏库保持只读。
所有下载先验证来源、大小和校验值，再提交；已有未托管文件、手动修改、不同版本
和声明的互斥依赖会阻止覆盖。移除按安装组处理，仍被其他组使用的依赖会保留。
已移除的文件可恢复；中断任务可继续。恢复遇到同名文件时不会覆盖。

Compatibility uses provider-declared game versions/loaders and required/optional/
incompatible dependencies. It is not a complete solver for arbitrary loader-specific
version-range expressions. A real client was tested with Mod Menu 21.0.1,
Placeholder API 3.2.0 and Fabric API 0.166.2 on Fabric 0.19.5 / Minecraft 26.3.
Faithful 32x and Complementary Reimagined passed package install/remove/restore;
shader rendering is not claimed. Jade/JEI optional selection was verified against
real 1.21.1 metadata (one file → four files → one file), without changing game data.
Paper servers default to the plugin category. Paper/Spigot/Bukkit versions are
checked against provider metadata and installed into `plugins`, separately from
loader mods. LuckPerms 5.5.71 passed actual search, install/remove/restore and
loading on Paper 26.3 build 249, including world restart and backup restoration.
Plugin-generated data/configuration are preserved when its managed JAR is removed.
The game-version and loader recommendation filters can be disabled independently
for discovery and version browsing. Nonmatching versions are explicitly labeled;
installation planning still rejects incompatible target versions/loaders. The
provider's project category and client/server boundary remain enforced. A real
LuckPerms read-only check expanded 1 matching version to 103 versions and rejected
a mismatched installation plan. This does not provide forced incompatible installs.
Additional providers remain open.

**Java** can discover local executables or explicitly download an independent Mojang
runtime. The official manifest and each raw file are checksum-verified; installed
runtime files are rechecked on discovery. No PATH or system Java setting is changed.
Real installations verified Java 8u51, 21.0.7 and 25.0.1. Java 8u51 is the historical
Mojang runtime offered by that catalog, not a claim of a current Java 8 security build.

Sources: [Modrinth API](https://docs.modrinth.com/api/operations/getversion/),
[Mojang runtime catalog](https://piston-meta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json).

## Content browser refinement / 内容浏览改进（待手动验收）

Opening the content page starts its first search automatically. Vanilla targets
start with resource packs, modded targets with mods, and Paper with plugins. A browse
snapshot pins owned receipts/identity and verifies only the selected version metadata
and identity receipt. It does not hash all assets or grant launch/write authority.
Search/version browsing reuses this context; dependency planning and all mutations
continue to revalidate the actual target independently. Refresh rebuilds the snapshot.

Project results show title, summary, author and download count. Optional PNG icons
load independently from the Modrinth CDN with four background workers, bounded bytes
and image dimensions, and a letter fallback. Image failures do not block the list.
No arbitrary image origins, credentials, redirects or executable content are used.

Search supports relevance/downloads/updated/newest ordering, provider total counts
and 20 results per page. Up to 16 pages are retained in memory for the current target;
explicit refresh/search discards the page cache. Selecting a project opens a dedicated
version section; the first matching version is selected automatically, and preview
still checks actual compatibility. A list being visible is not installation authority.

Installed content uses searchable 20-row pages with all/installed/removed/incomplete
filters, state/file counts, file details and the existing resume/remove/restore actions.
These remain managed installation groups, not an inventory of manually copied files.
Shared dependencies and modified-file protections are preserved.

All dropdowns now use the same glass-style control. Mouse selection leaves no focus
outline; keyboard navigation keeps focus indication and supports arrows, Enter/Space,
Escape and Tab. Memory is a preset list (including 1024/2048/4096/8192/16384 MiB);
any valid existing nonpreset allocation is retained as an additional choice.

本次修复首次不加载、浏览反复校验游戏导致等待、列表空间浪费和无翻页的问题。
项目与已安装内容均提供列表、搜索/筛选与翻页，项目另有简介、作者、下载量和图标。
内存改为常用值列表，下拉框鼠标点击后不保留亮色焦点框，键盘焦点仍可见。
本候选仅编译打包，未运行功能测试、界面测试、网络请求验收或性能测量；由用户手测。

## 2026-11-18 candidate: version and dependency cards

The permanent top navigation includes **资源 / Content**, and the current client content destination is shown on the page/confirmation panel. Project rows and version
rows display **已下载 / Downloaded** for the current target's managed content. The
snapshot verifies the recorded files' actual size/hash before assigning badges;
missing/changed files and same-name unmanaged files do not get this badge. Project
badges mean at least one verified version; version badges require the exact version.
All managed dependency files are included, not only root projects.

Version selection is a compact recommended card plus expandable Minecraft-version
groups. Recommendation prefers compatible featured stable releases, then compatible
stable releases, then the newest compatible version. The initial selection follows
this recommendation. Unpinned dependencies prefer an already verified compatible
version in the current target before choosing another stable release. Broader discovery filters do not permit incompatible installs.

The confirmation panel shows the selected version and separate required/optional
dependency cards. Required dependencies start checked; optional ones start unchecked.
Users may change either. Each change regenerates the plan (including transitives),
with shared anonymous project/version metadata cached for at most 120 seconds,
128 entries/16 MiB. Explicit list refresh clears this cache. File sizes/hashes and
actual target ownership are still checked by the existing install path.

Unchecking required dependencies is an explicit opt-out, not a compatibility pass.
The UI warns about possible startup failure, and the installed group retains the
omitted dependency titles. They are not silently re-added. Dependencies required by
other selected branches can still be included; the displayed final file count/bytes
reflects the resulting graph. Already verified shared files are reused, not downloaded
again. Modified/unmanaged files continue to block overwrites.

Thumbnails now support PNG/JPEG/WebP/GIF with bounded decode and retry attempts.
They remain optional; network failures, unsupported or oversized images use the
fallback. No actual provider/network or UI test was run for this candidate.

本轮按 HMCL 参考图重做紧凑的版本分组与依赖确认页，默认选中推荐版本。必要、可选
依赖均可勾选；取消必要依赖会显示启动风险，并记录到安装组详情。当前实例中已校验
的托管依赖会标为“已下载”，同名未托管文件不会冒充已识别内容。全部行为待手动验收。
