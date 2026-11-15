# AmberL Preview installer / 测试安装包

Windows x64, current user only. Independent application ID, uninstall registration,
Start Menu entry and default `LocalAppData/Programs/AmberL Preview` install location.
It does not replace AMBER, AmberE or AmberX. No automatic stable update feed is enabled. Manual official release discovery is described in UPDATES.md.
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

## Runtime delivery

The packages and Preview installer now include unmodified, Microsoft-signed x64
release CRT DLLs from the build machine's Visual Studio redistributable directory.
Each signature is checked before copying; runtime version, original notices and
package checksums are retained. No administrator/system runtime installation is run.
Actual QA confirmed VCRUNTIME140.dll loading from beside amberl.exe. This removes
the observed missing-VC-runtime prerequisite; it is not clean-OS certification.
Windows system DLLs and graphics drivers remain OS prerequisites.

测试包随应用携带官方签名的 Visual C++ 运行库，并纳入校验清单；不修改系统运行库。
已确认实际从应用目录加载。干净系统和其他显卡的验收仍需独立记录。

## Default game locations / 默认游戏位置

The installer creates `AMBER原生` and `.minecraft` beside `amberl.exe`. First launch
registers these locations, and portable launches create them if writable. Each game
still gets its own new named installation under the selected location; `.minecraft`
here is a container, not a promise to share one versions/assets directory. Uninstall
preserves these folders and their contents.

安装页通过列表选择这两个目录或已添加的位置。使用“添加游戏库”可以登记其他已有
文件夹；识别到的已有游戏库仍可扫描。新游戏使用独立目录，不覆盖旧游戏。已有目录
如果包含 versions/instances/saves/libraries 等游戏内容，安装位置显示为其中的
`AmberL` 子目录，只有开始安装时才创建该子目录。目录不可写时可以添加其他位置。

This candidate's folder creation, upgrade and uninstall behavior has **not** been
executed or tested, per the user's manual-acceptance instruction.
