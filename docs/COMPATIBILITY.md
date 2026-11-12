# Compatibility: explicit capabilities

These are the **initial probe's** capabilities, not the stable product contract.
All real Minecraft launching/installing/mutating/restoring is currently unimplemented.

| Input | Inspect | Isolation evidence | Launch / install / modify / restore |
|---|---|---|---|
| AMBER `instances/<id>/instance.json` | Basic identity/name/game/loader | Recognized managed directory layout; ID must match | Not implemented |
| Official `.minecraft/versions` | Profile and bounded inheritance | Explicit matching profile gameDir; missing evidence unknown | Not implemented |
| PCL per-version `PCL/Setup.ini` | Same profile inspection | Recognized IndieV2 and legacy explicit values; duplicates rejected | Not implemented |
| HMCL per-version `hmclversion.cfg` | Same profile inspection, game patch label | Only explicit `usesGlobal:false` and known modes | Not implemented |
| Missing global PCL/HMCL config | Metadata where readable | Unknown; content folder names do not prove isolation | Not implemented |
| Conflicting configured directories | Metadata where readable | Conflict; no target selected | Not implemented |
| Mixed `instances` + `versions` root | Rejected as ambiguous | User decision needed | Not implemented |
| Forge/NeoForge/Fabric/Quilt coordinates | Loader label only | Independent of loader label | Not implemented |
| Custom entrypoint/unknown loader | Unknown label | Do not infer vanilla | Not implemented |
| Legacy game arguments with gameDir | Row reports unsupported | No guessing quoted legacy paths | Not implemented |
| Junction/symlink/reparse/parent traversal | Conservatively rejected | No external path adoption | Not implemented |
| Server/FRP library | Not implemented | Not implemented | Not implemented |

Limits: 8 MiB metadata files, 16 inheritance levels, 20,000 profile directories, bounded
JSON parsing. Scanning can be cancelled between filesystem operations, but a blocked
OS filesystem read is not promised to interrupt immediately. Windows app-only registration
supports up to 256 libraries; missing/replaced roots fail when opened, retaining registration.
The read-only path strategy does not guarantee isolation against hostile concurrent path
replacement. Registered scans additionally guard the root and its ancestors by handles;
individual game metadata remains read-only. Game writes remain disabled until the resource
transaction and shared-content locking layer exists. Registry detach is not game deletion.

## Stable matrix to populate
Each evidence row must pin layout sample, Minecraft/loader version, isolation mode,
Java/OS/architecture and source revision. Separate columns: identify / launch / install /
content edit / component edit / recycle / backup restore. Use passed / limited /
unsupported / unverified, with exact reason and evidence. Test copies, never originals.

AMBER 1.7.1 has bounded external launch support; its loader recognition and prior test
results do not establish AmberL compatibility. Modern Forge/NeoForge module launchers
and old Java native-window behavior are explicit future test cases.

