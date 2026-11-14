# Owned content and Java runtimes / 内容与 Java 管理

Game configuration opens **模组与资源**; stopped modded servers open
**管理服务端模组**. Search Modrinth, select a project and compatible version,
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
Additional providers, filter overrides and plugin support remain open.

**Java** can discover local executables or explicitly download an independent Mojang
runtime. The official manifest and each raw file are checksum-verified; installed
runtime files are rechecked on discovery. No PATH or system Java setting is changed.
Real installations verified Java 8u51, 21.0.7 and 25.0.1. Java 8u51 is the historical
Mojang runtime offered by that catalog, not a claim of a current Java 8 security build.

Sources: [Modrinth API](https://docs.modrinth.com/api/operations/getversion/),
[Mojang runtime catalog](https://piston-meta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json).
