# Product contract and staged delivery

## Outcome / 目标
Complete local game and multiplayer fundamentals with lower decision burden, clear
object context, fast response and refined native interaction. Compare against AMBER
on the same machine, cloned libraries, accounts, network and cache conditions before
making any superiority claims. Smaller Rust executable size alone is not success.

## Required stable scope / 正式版范围
- Multi-library installation, management and real game launch; official/PCL/HMCL
  layouts with isolated/shared/unknown states and explicit operation capability matrix.
- Mod/resource/shader search, version choice, dependency choices, installation,
  cancellation, retry and recovery. Recommended filters are overridable.
- Microsoft login, ownership verification, account lifecycle and OS-protected tokens.
  Reuse AMBER's application ID/authority/scope, not its token files or application state.
- Java discovery, explicit executable selection, on-demand install, Java 8 and modern Java.
- Server creation/start/graceful stop/console/configuration, mods/plugins, FRP,
  joining and sharing. Server and tunnel can run independently.
- Truthful process/readiness/tunnel/reachability states and explicit close behavior.
- Logs, recoverable deletion, verified backups of actual saves/config and recovery.
- Light/dark, Chinese/English, scaling, Chinese IME, keyboard and accessibility.

Exclude seed maps, gradient text, theme stores, arbitrary layout customization,
complex guided setup, workflow orchestration, MySQL and schedules from the first release.

## Information architecture / 信息架构
Games and Multiplayer are primary workspaces. Instances own content browsing and
configuration. Downloads/tasks, accounts and settings are auxiliary. First launch offers
Install game / Add existing library, without mandatory tutorial. Keep identity/title/main
action positions stable. Shared-directory effects are shown before changes. Unknown
isolation must be resolved before launch/mutation. No artificial splash delays.

## Stages and gates / 阶段与门槛
| Stage | Work | Exit evidence |
|---|---|---|
| A | Specification, task scripts, identity model, compatibility dimensions, benchmark plan | Every required capability has a user task and explicit unknown/error handling |
| B | Ambrum vertical prototypes, Slint/WinUI comparison, real old/new Java and game window, download, auth, tray/process experiment | Comparable measurements and resolved critical feasibility risks; framework decision recorded |
| C | Local end-to-end flow, multi-library, installation, content, isolation, recovery | Supported matrix cells complete install/play/change/recover; failures do not contaminate other instances |
| D | Servers, console, content, FRP, joining, exit protection | Two devices actually join; normal stop/reload preserves saves; state reporting is truthful |
| E | Fault recovery, copies of old libraries, large-library speed, accessibility/DPI, installer upgrades and user tasks | Required cells pass; no known critical data-boundary failures; measured budgets met |
| F | Independent identity/update channel, reproducible delivery, notices/docs | User acceptance and explicit stable-publication authorization |

The initial read-only native probe implements part of B, not completion of B/C.
No calendar estimate is made before UI/dependency feasibility and first measurements.

Resumed stage B work adds actual official HTTPS discovery, a new-directory Vanilla
installation transaction and bounded native extraction/recovery. Draft PR #1 and Windows
CI make the prototype reviewable and downloadable. Microsoft device login, DPAPI storage,
Java discovery/explicit choice and actual Vanilla 26.3 world/save/exit now extend this slice.
Slint/WinUI comparison, old Java/game windows, runtime installation and server lifecycle
remain open gates; this does not complete B or C.

## User task comparison / 体验对比
First use, add existing library, install a game, install a mod, create/join multiplayer.
Measure success, elapsed time, incorrect actions, help requests and feedback; alternate
product order to limit learning bias. Freeze revisions and input sets. Separate network
time from active interaction. Do not reuse private libraries directly.

Record package size, disk footprint, first dependency downloads, cold launch, total
idle process memory/CPU/disk, scan/search/download/list responsiveness. Report Java/FRP
separately and also show combined totals. Set budgets after baseline and sample results.

## Implementation discipline
Prefer a small number of concrete modules and mature dependencies. No homemade TLS,
crypto, compression, FRP or general GUI framework. Use targeted regressions rather than
repeating AMBER's unrelated tests. Newly ported behavior still needs its own verification.

