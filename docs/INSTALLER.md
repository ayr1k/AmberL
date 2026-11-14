# AmberL Preview installer / 测试安装包

Windows x64, current user only. Independent application ID, uninstall registration,
Start Menu entry and default `LocalAppData/Programs/AmberL Preview` install location.
It does not replace AMBER, AmberE or AmberX. No stable release/update feed is enabled.
The exact Git revision and corresponding MPL source archive are installed alongside
the executable. See FIRST-VERSION.md for implemented behavior and pending acceptance.

安装时无需管理员权限。默认安装到当前用户的 AmberL Preview 独立目录。
覆盖安装用于更新此测试渠道；更新前请正常退出 AmberL 及其游戏、服务端和联机任务。
安装器不会强制结束这些进程。卸载只移除安装器记录的程序文件，保留账户、游戏、
服务端、Java 和应用数据。卸载后这些数据仍可供重新安装的 AmberL 使用。

This development installer is unsigned. Do not describe it as a signed stable release.
The build tool is pinned Inno Setup 7.1.1 from the official jrsoftware/issrc release,
verified by SHA-256 and its Pyrsys B.V. Authenticode signature. Compiler tools are
prepared in the project's ignored local directory; they are not committed.

Build a clean committed UI package first, then run:
`./scripts/package-installer.ps1 -PackageRoot artifacts/AmberL-ui-comparison-<revision>`.
The resulting installer and SHA-256 sidecar belong in the Draft PR's test artifacts.
Clean-machine, upgrade and uninstall/data-retention checks must be reported separately
from merely compiling the installer. No stable publication is implied.
