# First-version completion ledger / 首版完成清单

User direction, 2026-11-14: continue until the first version is complete, including
multiplayer. This ledger tracks the full PLAN.md contract; an implementation check
is not user acceptance or authorization to merge/publish. Baseline: `452c161`.

| Area | Existing evidence | Remaining work / acceptance |
|---|---|---|
| Accounts and launch | Microsoft/DPAPI, explicit Java, Vanilla and four client loaders run | Broader supported matrix, old Java/game, error/recovery acceptance |
| Multiplayer | Vanilla, Fabric, Quilt, Forge, NeoForge server lifecycle; verified snapshots restored into new runnable worlds; bounded configuration with write recovery; invitation; one-PC two-peer authenticated entry | Plugins, FRP, reconnect, two physical devices/NAT |
| Content | Modrinth search, compatible version choice, required/optional dependency preview, verified install, shared dependencies, recoverable groups; actual Mod Menu client launch | Additional provider/error matrices, filter overrides, plugins and final GUI acceptance |
| Java | Local discovery plus official manifest/hash-verified independent runtimes; real Java 8/21/25 inspection; Java 8 old server and Java 25 Forge worlds | Additional runtime variants and game/UI acceptance |
| Libraries and data | Multi-library read-only registration, new isolated installs | Owned-instance maintenance, shared/unknown capability rules, backups/restore/recoverable removal |
| Desktop | Frameless glass, version list, saved per-game choices | Final framework comparison decision, Chinese/English, light/dark, keyboard/IME/Narrator/DPI |
| Delivery | Exact-source portable ZIP, checksums/notices, Draft PR and CI; independent per-user preview installer implemented | Installer/upgrade/data-retention validation, update channel, clean-machine startup, release documentation |
| Product acceptance | Disposable local fixtures and user feedback | Same-condition AMBER comparison, two-device and user task acceptance |

Only synthetic fixtures or disposable copies may be changed during validation. All
original libraries, credentials, installations and other Amber products stay outside
this development scope. Two peers on one host never count as two-device acceptance.

Merge, stable Release and website publication remain outside this task authorization.

2026-11-14 user clarification: no second PC is currently available; complete all
automatable work first. Two-device/NAT acceptance remains explicitly unverified.

Current development verification (not yet a packaged revision):
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
  Configuration dialog spacing corrected after visual inspection; final rebuilt UI
  and package validation remain pending.
- Content search, version/dependency preview and installed-group removal/restoration
  verified in the actual desktop UI. Empty-list action enablement corrected.
- Jade/JEI optional choices use real provider data: 1 → 4 → 1 planned files.
- Server configuration: stale edits/live sessions rejected, interrupted write blocks
  startup and restores original with interrupted bytes preserved; Chinese round-trip
  passes. UI save verified; same-content reload refresh corrected after UI testing.
