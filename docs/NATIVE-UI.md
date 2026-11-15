# Native launcher preview / 原生启动器预览

## Window and workspace polish — 2026-11-14

Games and Multiplayer now share the primary navigation. Multiplayer fills the
workspace, with a left sidebar and a scrollable form rather than a centered dialog.
Forms have a maximum readable width on large screens; the minimum window is
960 × 640 logical pixels. Drag the logo or unused top-bar area to move the window;
resize at its edges or the visible bottom-right grip. Window operations use the
native winit/Windows move and resize paths.

The launcher enables FemtoVG GPU rendering, retaining the software fallback.
Glass corner masks are sampled at up to twice logical resolution (bounded to
3840 × 2160); the blur applies to the scenery, not text or controls. The original
comparison executable explicitly retains its software renderer, so its earlier
measurements are not presented as measurements of the new launcher renderer.

Page entry fades take 180 ms and button color transitions take 130 ms. Windows'
client-area animation preference is read at startup and respected. Successful
background refreshes no longer write generic completion messages; active progress,
errors and explicit server/room/FRP state remain visible. Existing task logs remain.

联机使用独立主界面和左侧导航；小窗口内容可滚动，大屏表单不再无限拉宽。
顶部标志与空白区域可拖动，边缘和右下角角标可调整尺寸。短过渡遵循 Windows
动画设置。移除空闲时的通用“操作完成”，保留真实任务进度、错误及联机状态。

GPU availability and fallback quality still depend on the machine/driver. The
existing background artwork remains 1487 × 1058; it is not a newly generated 4K
asset. Cross-monitor DPI/Narrator acceptance remains separate from this UI fix.

Verification: the native launcher was inspected in Chinese and English at about
962 × 641, normal 1280 × 820 and maximized 2560 × 1440 on the current Windows
desktop. Dragging the maximized top bar restored and moved the window; the visible
corner grip resized it to the minimum. Settings, server creation and FRP controls
remained readable, and idle generic completion text was absent. The desktop-model
tests (7), strict workspace/Slint Clippy and release UI build passed. These checks
use an isolated app directory, not the installed preview's data.

Open **`launcher/amberl.exe`** in the UI development ZIP. The `slint/` and `winui/`
executables are the older framework comparison samples, not this interface.
Windows x64, tested on Windows 11; unpack-and-run preview, not a stable installer.

The user selected the dark launch-first direction, retained the AMBER graphic mark,
and then requested a full-window background with Aero-like frosted menus. The
implemented view has a 64 px navigation panel, compact game list and aligned 56 px
launch controls. Teal identifies the logo center, terminal L and primary action.
The native Windows caption is removed. Integrated minimize/maximize/close controls,
OS-backed dragging and a 10 px resize border share the existing safe-close handler.

Glass uses one Gaussian-blurred copy of the embedded static scene, calculated once
at startup and reused with viewport-aligned offsets and clipped rounded panels.
It does not blur the desktop, other apps or live game windows. No WebView, Node
process, service or per-frame CPU convolution is used. System fonts are not bundled.

## Use / 使用

1. Add a disposable library copy, or install an official game version with optional Fabric, Quilt, Forge or NeoForge into a new
   directory under an existing parent. Existing destinations are never replaced.
2. Open the Microsoft account tile. Select an explicitly saved account or start
   device login and complete it yourself on the Microsoft official page. Passwords
   are never entered into AmberL. Accounts use its independent current-user DPAPI store.
3. Open Java/configuration, find local candidates and explicitly choose one, or enter
   the executable path. Set the maximum heap. Per-game choices persist when saved,
   launched or switched; account IDs are references to the independent protected store.
4. Select a game. Only an AmberL-created complete installation can proceed to launch;
   the marker shown in the list is a hint, not authorization or readiness evidence.
   Core launch rechecks identity, receipts, resource hashes, Java and account ownership.
5. Tasks shows bounded progress/errors. Cancelling a running game requests normal
   exit and waits while retaining core locks. Closing the launcher while a task is
   active requests cancellation and keeps the window open until the task finishes.

默认配置独立保存在 `%LOCALAPPDATA%/io.github.ayr1k.amberl`。隔离测试可传入
`amberl.exe --app-directory <已有独立目录>`；不要把游戏目录作为应用数据目录。
账户与 Java 不会自动替换为第一项。后台任务期间不允许切换启动目标。
外部游戏库只登记和扫描，不迁移、不修改、不直接启动。
已确认隔离位置的原版或可准确识别组件的实例可通过 **配置 → 游戏库详情与副本** 创建独立副本；
共享目录需要勾选确认，原库仍保持只读。详见 [副本导入与限制](IMPORT.md)。

## Limits / 限制

This is a first-version development candidate. Slint carries the chosen visual implementation;
full Narrator/high contrast, multi-monitor DPI transitions and another clean-machine
deployment remain deferred acceptance. Read-only library parsing is broader than launch
compatibility. Two-device multiplayer acceptance remains deferred. Manual stable-release
discovery is available in About; see UPDATES.md. There is no automatic installer execution
or force-exit policy. Managed runtimes and a per-user test installer are available;
see CONTENT.md and INSTALLER.md. Process creation
does not establish world readiness. A windowless/unresponsive game can keep cancellation
waiting; there is no bulk Java termination. No stable release or merge is authorized.

The **联机** panel now includes Vanilla server creation/start/graceful stop, console,
EasyTier invitation rooms, status queries and joining a compatible selected game.
See [multiplayer instructions and limits](MULTIPLAYER.md). A local two-peer smoke test
is not two-computer or public-relay acceptance. Server Java/memory choices remain
session-scoped; game launch preferences persist independently per game.

See `design-qa.md` for this visual iteration and PR #1 for the exact tested revision,
package checksums and current real-game verification; earlier CLI acceptance is kept
distinct from this UI's tests.

## Installation wizard / 安装向导

Choose a version from the searchable official list (release/snapshot/historical filters),
then one loader and its provider version, then a display name, built-in icon, existing
parent directory and unique folder name. Grass/dirt/anvil/pickaxe reuse AMBER assets;
name and icon are saved in the immutable installation metadata and survive restart.
Changing the displayed name does not change the version identity used for launch.

Fabric/Quilt use official profile APIs and checksum-verified Maven libraries.
Modern Forge/NeoForge run their official installer in a unique AmberL work directory,
with explicit matching Java, bounded wait and cancellation of that owned installer
process only. Generated libraries are checked and imported into the verified cache;
all final files enter the existing journaled, no-overwrite install transaction. Work
folders are retained for diagnosis, not silently deleted. Third-party installers are
trusted executable code; this is not an OS sandbox. The selected resource/install
budget is checked before final commit, not a hard disk quota on a running installer.

旧版 Forge（旧式启动参数或不支持 `--installClient` 的安装器）暂不支持。
Fabric API、QSL、OptiFine、整合包、已有实例的组件修改不包含在这次安装向导中。
版本列表表示官方目录中可选的组合，不代表每一个历史版本都经过实际游戏验收。
External game libraries remain read-only. Existing installations are not upgraded
in place. Owned-instance display names/icons can now be edited independently from
immutable version metadata; see INSTANCE-RECOVERY.md for removal and backup restoration.

## Appearance / 外观

The top-level **设置** opens app settings; the bottom **配置** opens the selected game's
launch settings. Choose **深色玻璃 / 浅色玻璃** to switch immediately. AmberL remembers
the choice in its own `appearance-v1.json`; system and other Amber product settings
are unchanged. Both retain the scenery, teal identity and aligned glass panels.
Light mode uses dark text, themed native inputs and visible keyboard focus.

顶部 **设置** 管理应用外观，底部 **配置** 管理当前游戏。浅色与深色玻璃即时切换，
下次启动恢复上次选择。版本列表已实际验证 Tab 导航和回车选择；这不等于完整的
读屏、中文输入法预编辑或跨屏 DPI 验收。

## Language / 语言

Settings offers **简体中文 / English** with immediate switching and independent
theme/language persistence. English UI translations are bundled at build time;
no translation service or runtime download is used. Existing theme-only preferences
remain readable. Game/server names, paths, version identities and provider titles
are preserved. Loader selection uses canonical identities rather than translated labels.

设置中的语言按钮可即时切换中英文，下次启动恢复选择。主界面、安装向导、账户、
配置、内容与联机表单及常见状态已翻译；游戏日志、第三方错误和部分底层诊断
保留原文，不能将这次交付描述为所有诊断信息均已完成本地化。

The English install wizard and multiplayer forms were inspected at 1002 × 702;
real Paper catalog retrieval confirms translated loader labels still select the
intended provider. This is not full Narrator, IME or cross-monitor acceptance.

## Scaling and recovery / 缩放与恢复提示

Initial sizing waits for the native window, then fits physical bounds to the current
monitor work area using the UI scale. Minimum effective layout size is 800 × 500;
the display work area must accommodate that at the selected scale. Short forms scroll
instead of hiding their actions; changing pages resets the form scroll position.
At 200% application scale the initial window, native corner resize to 1600 × 1000
physical pixels, About/update check, configuration, mod-copy consent and Multiplayer
were inspected. This does not substitute for cross-monitor or all OS scaling tests.

Common network, Java, account, checksum, space and destination-conflict diagnostics
include Chinese/English next-step guidance. Original diagnostics remain visible;
retry is explicit and existing identity/integrity checks still run. No generic idle
“operation complete” message is restored.

## Manual-acceptance candidate: sign-in and resize refinement

Saved valid game account choice, then global account choice, then the first stored
Microsoft profile is selected automatically. This does not sign in to a new account
or copy credentials. Explicit account selection saves the global preference.
Microsoft device authorization has its own worker, deadline and cancel action;
waiting for the official page no longer holds the download/game worker busy.
The account page copies the requested device code once, reports clipboard failure,
and allows retry. Device codes are not written to logs or settings.

Tasks show the operation title, state, relevant cancel action and collapsible details.
Mouse clicks no longer leave a keyboard-focus outline; keyboard navigation keeps it.
Glass geometry settles for 140 ms before regenerating cached textures; during edge
resizing the last texture stretches with the panel. Final texture quality is retained.
These changes are implemented and built only: no UI, login or performance testing
was run for this revision at the user's request.

账户自动选择、安装位置列表、登录取消与设备码复制、任务页和缩放优化待手动验收。
此前 DPI/界面测试记录仅适用于其标注的旧版本，不证明本次体验已通过。
