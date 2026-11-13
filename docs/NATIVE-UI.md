# Native launcher preview / 原生启动器预览

Open **`launcher/amberl.exe`** in the UI development ZIP. The `slint/` and `winui/`
executables are the older framework comparison samples, not this interface.
Windows x64, tested on Windows 11; unpack-and-run preview, not a stable installer.

The user selected the dark launch-first direction, retained the AMBER graphic mark,
and then requested a full-window background with Aero-like frosted menus. The
implemented view has a 64 px navigation panel, compact game list and aligned 56 px
launch controls. Teal identifies the logo center, terminal L and primary action.
The ordinary Windows caption retains native move/resize/system-menu behavior.

Glass uses one Gaussian-blurred copy of the embedded static scene, calculated once
at startup and reused with viewport-aligned offsets and clipped rounded panels.
It does not blur the desktop, other apps or live game windows. No WebView, Node
process, service or per-frame CPU convolution is used. System fonts are not bundled.

## Use / 使用

1. Add a disposable library copy, or install an official Vanilla version into a new
   directory under an existing parent. Existing destinations are never replaced.
2. Open the Microsoft account tile. Select an explicitly saved account or start
   device login and complete it yourself on the Microsoft official page. Passwords
   are never entered into AmberL. Accounts use its independent current-user DPAPI store.
3. Open Java/configuration, find local candidates and explicitly choose one, or enter
   the executable path. Set the maximum heap. Choices currently last for this session.
4. Select a game. Only an AmberL-created Vanilla installation can proceed to launch;
   the marker shown in the list is a hint, not authorization or readiness evidence.
   Core launch rechecks identity, receipts, resource hashes, Java and account ownership.
5. Tasks shows bounded progress/errors. Cancelling a running game requests normal
   exit and waits while retaining core locks. Closing the launcher while a task is
   active requests cancellation and keeps the window open until the task finishes.

默认配置独立保存在 `%LOCALAPPDATA%/io.github.ayr1k.amberl`。隔离测试可传入
`amberl.exe --app-directory <已有独立目录>`；不要把游戏目录作为应用数据目录。
账户与 Java 不会自动替换为第一项。后台任务期间不允许切换启动目标。
外部游戏库只登记和扫描，不迁移、不修改、不直接启动。

## Limits / 限制

Stage B remains a preview. Slint carries the chosen visual implementation while
IME composition, Narrator/high contrast, multi-monitor DPI transitions and clean-machine
deployment remain acceptance gates. Read-only library parsing is broader than launch
compatibility. Multiplayer, loaders, runtime downloading, persisted per-game settings,
installer/updater delivery and a force-exit policy remain future work. Process creation
does not establish world readiness. A windowless/unresponsive game can keep cancellation
waiting; there is no bulk Java termination. No stable release or merge is authorized.

See `design-qa.md` for this visual iteration and PR #1 for the exact tested revision,
package checksums and current real-game verification; earlier CLI acceptance is kept
distinct from this UI's tests.
