# First-version completion ledger / 首版完成清单


## Current candidate: user-requested manual acceptance only

Latest instruction supersedes the automated-verification direction below: **do not
run tests after this update; the user will test manually**. This candidate is built
and packaged only. No automated tests, application launch, installer execution,
login, game, UI or resize-performance acceptance is claimed for these changes.
Earlier evidence below belongs to earlier revisions, most recently `974d3d6a`.
The exact candidate revision is recorded in each package's TEST-REVISION.txt and PR.

本次按用户要求只编译、打包，不运行测试或启动程序。以下改动均待用户手动验收，
不能沿用旧版本的通过记录：默认游戏目录与安装列表、账户自动选择、独立可取消的
微软登录、设备码自动复制、任务页、鼠标焦点边框和窗口缩放纹理更新。

The CI commit trailer `[manual-test-only]` selects build/package-only delivery for
this candidate. Verification steps are visibly skipped and the job summary records
that no tests ran. Ordinary commits without that trailer retain existing checks.
No merge, stable Release or website publication is authorized.

User direction, 2026-11-14: continue until the first version is complete, including
multiplayer. This ledger tracks the full PLAN.md contract; an implementation check
is not user acceptance or authorization to merge/publish. Baseline: `452c161`.

Latest user direction: skip acceptance requiring the user's participation for now;
finish independent development and automatable verification. Deferred acceptance is
not a pass and does not block work on other areas. Two-device/NAT testing, the user's
own-library/task acceptance and subjective framework/appearance approval remain
explicitly deferred. Merge and stable publication still require separate authorization.

用户要求先跳过需要本人完成的验收，继续完成开发和可自动验证项目。双机外网、
用户实际游戏库与体验验收保持“待验”，不能记作通过，也不作为继续开发的阻碍。

| Area | Implemented development scope / evidence | Deferred acceptance / supported limits |
|---|---|---|
| Accounts and launch | Microsoft/DPAPI, explicit Java, Vanilla and four client loaders run; 1.12.2/Java 8 and 1.21.4/Java 21 actual world entry/save/re-entry/exit; contextual recovery guidance | All historical loader/game combinations are not certified |
| Multiplayer | Vanilla, Fabric, Quilt, Forge, NeoForge and Paper lifecycle; verified snapshots restored into new runnable worlds; bounded configuration with recovery; invitation; one-PC authenticated entry; FRP forwarding/node-restart reconnection and independent stop | Public-node and two physical devices/NAT acceptance |
| Content | Modrinth search, overridable discovery filters, compatible versions, required/optional dependencies, verified install, shared dependencies, recoverable groups; actual Mod Menu client and LuckPerms Paper loading | Provider declarations are not universal mod compatibility; copied unmanaged mods retain their own provenance |
| Java | Local discovery plus official manifest/hash-verified independent runtimes; real Java 8/21/25 inspection and game/server use | Additional OS/architecture/runtime variants are outside the tested Windows x64 matrix |
| Libraries and data | Read-only registration; Vanilla and exactly identified Fabric/Quilt/Forge/NeoForge copies; renamed profiles, explicit mod trust, shared confirmation/unknown blocking; names/icons, snapshots/new-copy restore and recoverable removal | User-library acceptance; unknown/custom/legacy profiles remain blocked per IMPORT.md |
| Desktop | Frameless glass, searchable versions, saved choices, themes/languages, page/button transitions, independent Multiplayer, native drag/resize, contextual recovery help; Slint candidate delivery | Final subjective framework comparison, full Narrator/high contrast, cross-monitor DPI; raw provider diagnostics keep their language |
| Delivery | Exact-source ZIP, checksums/notices, Draft PR/CI, per-user installer; app-local signed CRT; manual repository-specific stable-release check | Another clean Windows machine and user installer acceptance; no automatic installer execution or stable publication |
| Product acceptance | Disposable local fixtures and user feedback | Same-condition AMBER comparison, two-device and user task acceptance |

Only synthetic fixtures or disposable copies may be changed during validation. All
original libraries, credentials, installations and other Amber products stay outside
this development scope. Two peers on one host never count as two-device acceptance.

Merge, stable Release and website publication remain outside this task authorization.

2026-11-14 user clarification: no second PC is currently available; complete all
automatable work first. Two-device/NAT acceptance remains explicitly unverified.

Development evidence (each package/PR pins its tested revision):
- Fabric 0.19.5, Quilt 0.31.0-beta.1, Forge 26.3-66.1.8, NeoForge 26.3.0.103,
  Minecraft 26.3: create, actual readiness, protocol response, `list`, graceful save,
  same-world restart, exit 0. Forge can delay status responses beyond its Done line;
  the fixture retries the query independently rather than treating Done as reachability.
- Mojang runtimes 8u51, 21.0.7, 25.0.1: official manifest/file checksum validation,
  actual Java inspection and rediscovery. No PATH/system Java changes.
- Vanilla 1.12.2 / downloaded Java 8 and Forge 26.3 / downloaded Java 25: snapshot,
  original preserved, new-copy restore, recovered world starts and saves normally.
- Mod Menu 21.0.1 + Placeholder API 3.2.0 + Fabric API 0.166.2: compatible plan,
  install/remove/restore, actual authenticated Fabric client, visible Mod Menu and
  normal exit 0. Faithful 32x 26.3 and Complementary Reimagined r5.9.3:
  verified package install/remove/restore only; shader rendering is not claimed.
- Native Java discovery/selection and recoverable server removal/return verified.
  Configuration dialog spacing corrected and rebuilt UI/package verified.
- Content search, version/dependency preview and installed-group removal/restoration
  verified in the actual desktop UI. Empty-list action enablement corrected.
- Jade/JEI optional choices use real provider data: 1 → 4 → 1 planned files.
- Server configuration: stale edits/live sessions rejected, interrupted write blocks
  startup and restores original with interrupted bytes preserved; Chinese round-trip
  passes. UI save verified; same-content reload refresh corrected after UI testing.
- FRP 0.72.0: pinned official frpc, TLS CA verification, endpoint-bound DPAPI token,
  actual Minecraft query through localhost frps, node restart/reconnection, independent
  tunnel stop and graceful server save. No external-node or two-device claim.
- Paper 26.3 build 249 + LuckPerms 5.5.71: actual search/install/remove/restore,
  plugin loading, query/console/save/restart, snapshot/new-world restoration. Fixed
  Paper's generated `versions` directory conflicting with app-state checks; the
  exception is confined to already-owned, locked server state. UI plugin category,
  search, installed records and 1002 × 702 layout inspected.
- 120 workspace tests pass after the FRP/Paper changes. Installer delivery at
  c75e5ad4 had 738 package file hashes/156 source blobs and both CI jobs verified.
- Discovery filters can be relaxed independently; incompatible results are labelled
  and install planning still rejects incompatible versions. Actual LuckPerms catalog
  expanded from 1 to 103 versions; UI mismatch handling verified.
- Owned-instance full snapshots/new-copy restoration preserve mutable worlds and
  immutable installation verification. Actual restored Fabric client displayed 51 mods
  and exited normally. Native naming/icons, recoverable removal and restore inspected;
  see INSTANCE-RECOVERY.md for limits. 123 workspace tests pass for this development slice.
- Chinese/English UI now switches immediately with restart persistence. English
  account/configuration/installation/multiplayer screens and Chinese user-created
  names inspected at 1002 × 702; actual English Paper catalog retrieval passed.
  Fixed truncated action labels and a busy-state cancel button escaping the dialog.
  Theme-only configuration upgrade, independent preference saves and corrupt-file
  preservation passed. Raw logs/provider errors and some diagnostics retain their
  original language; broader localization/accessibility acceptance remains open.
- External Vanilla data can now be copied into a new verified official installation.
  Actual native import of a disposable HMCL-style world, world entry/save/exit 0,
  unchanged source hashes, shared confirmation and unknown-isolation blocking passed.
  The final form was inspected at 1002 × 702; 126 workspace tests pass. See IMPORT.md
  for copied data and recovery. The following slice extends that original Vanilla scope.
- Exactly identified Fabric/Quilt/Forge/NeoForge and renamed metadata are now covered
  by the read-only capability matrix. Mod/script copies require explicit trust;
  official resources are rebuilt without source launch arguments. A real Fabric 26.3 /
  0.20.1 copy loaded 51 mods, entered/saved a new QA world and exited 0; seven source
  fixture files still matched after launch. 129 workspace tests pass.
- Manual stable-release discovery is independent of AMBER/AmberE and was checked
  against the live empty AmberL release list, including the native About UI.
  Release metadata validation covers version/channel/repository/digest boundaries.
- Packages include signature-checked, app-local Microsoft release runtimes and notices;
  actual VCRUNTIME140.dll loading from the application directory was confirmed.
  This removes the observed runtime dependency, not the need for another-machine acceptance.
- High application scale revealed oversized initial windows; initial sizing now waits
  for the native window and fits the monitor work area. 200% startup, minimum 800 × 500
  logical layout, scrollable configuration/import and Multiplayer were inspected.
  Contextual recovery instructions supplement raw diagnostics. See NATIVE-UI.md.

The first development candidate covers the supported scope above. Package integrity,
installer lifecycle, exact revision and CI results are recorded in Draft PR #1 for
each delivery. The remaining user/device-dependent acceptance is deferred, never
counted as passed; this candidate is not a merged or published stable release.
