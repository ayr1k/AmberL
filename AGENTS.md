# AmberL

Independent product and repository, Rust core named Ambrum. The user authorized
implementation, creating/uploading ayr1k/AmberL using the desktop's ayr1k account.
Do not merge subsequent task PRs or publish stable releases without user approval.
Keep AMBER/AmberE/AmberX source, installed applications, settings and libraries intact.

## Product contract
- Complete local play and multiplayer fundamentals; smaller decision burden,
  not fewer necessary choices. No WebView, Node daemon, or local microservices.
- Rust core is UI-independent. UI framework selection requires comparable samples.
- Reuse AMBER's public Microsoft application settings, not account files/tokens.
- Retain versions, loaders, Java, library location and isolation choices.
- Unknown isolation, readiness, connectivity or reachability stays unknown.
- Client/server/library/session identities must be explicit; UI selection is not authority.
- Default library attachment is non-mutating. Never test on original game data.
- Writes need path ownership, reparse/alias checks, conflict detection, staging,
  integrity verification, recoverable commits and shared-directory locks.
- Own process handles only; graceful server stop; no bulk Java termination.
- Secrets use OS secure storage; no plaintext fallback or secrets in logs.
- No automatic firewall/DNS/security changes.
- Preserve accessibility, Chinese IME, keyboard input, DPI, language and readability.

## Work efficiently
Keep modules small and direct. Test new behavior and affected boundaries only.
Do not re-run AMBER's unrelated suite. Re-test after relevant changes, failures or
unresolved risks. Distinguish unit/synthetic tests from actual game/user acceptance.
Never describe a read-only probe as a working launcher or a UI baseline as final design.
Document exact source revision, checks and limitations in task deliveries.
Generated packages stay out of commits. Keep concise English/Chinese documentation.


## Accepted native UI direction — 2026-11-13

Retain full-window scenery, static-scene Gaussian glass and the AMBER graphic mark
with AmberL teal center/L. Use app-integrated window controls (no Windows caption),
compact aligned dialogs, a searchable version list and a staged installation flow.
Creation supports custom display names and AMBER grass/dirt/anvil/pickaxe icons.
Fabric, Quilt and modern Forge/NeoForge are actual installation paths, not mock choices;
keep older-loader and additional-mod limits explicit. This does not authorize merging,
a stable release, or declaring Slint's final framework/accessibility acceptance complete.
