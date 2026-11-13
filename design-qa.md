# AmberL native UI — visual verification, 2026-11-13

final result: passed

This passes the requested visual correction, not final product/framework acceptance.

## Visual truth and evidence

- Original selected direction: `docs/design/selected-dark-launcher.png`, 1487×1058.
- User correction overrides its fixed dark columns: full-window scenery, Aero-like
  Gaussian menus, smaller navigation, compact spacing, aligned buttons, no stray rules.
- Actual native capture: `docs/design/implemented-glass-home.png`, 1282×852 including
  the normal Windows caption; client area 1280×820 logical px at observed 100% scale.
- Compact capture: `docs/design/implemented-glass-compact.png`, 1002×732 including
  caption; 1000×700 client area. Captured before the final corner-mask-only fix.
- Post-fix settings capture: `docs/design/implemented-glass-settings.png`, 1282×852.
- Source and live capture were opened together in the same tool response. Compare
  content regions, excluding native caption; no claim of a pixel-identical clone of
  the old mock. The user explicitly requested different proportions and glass.
- Home captures show one actual installation with no account/Java selected. The mock
  has three fictional names and a selected fictional account. These content/state
  differences are intentional; no fake installed games or readiness were inserted.
- Full-resolution views made logo, typography, buttons and panel edges readable.
  The settings view supplies a focused panel comparison; separate enlarged crops
  were unnecessary. Private account/session captures are not included in source.

## Findings and correction history

1. **P1, fixed — scene offset and broken major-region proportions.** The rejected
   1440×990 implementation centered a child with an implicit origin, exposing a wide
   empty left strip. A single explicit-origin cover image now fills the client area.
   Home and compact captures show continuous scenery with no exposed strip.
2. **P1, fixed — oversized navigation and unanchored actions.** Replaced the 100 px
   header/112 px footer with a 64 px floating navigation and 100 px launch panel.
   Launch controls share a 56 px height and 22 px vertical inset. Removed isolated
   horizontal/vertical separators. The compact capture retains every persistent control.
3. **P2, fixed — excessive panel space and empty device-code box.** Account/library/
   settings/about panels now have content-appropriate heights; the device-code area
   exists only during device login. The 540 px settings panel was checked at normal
   and 1000×700 client sizes; all fields and the return action remain visible.
4. **P2, fixed — blurred image exposed at rounded corners.** The software renderer
   does not round child-image clipping. Final glass tiles now include an antialiased
   alpha mask; full-scene Gaussian and resize/panel caches are bounded and reused.
   Reopened home and settings captures show clean corners without pale square wedges.

## Required surfaces

- **Typography:** Windows Microsoft YaHei UI; 48/38 px hero, 22 px section heading,
  14 px actions, 11 px secondary labels. Titles remain legible and do not wrap into
  controls at the minimum size. No fonts are redistributed.
- **Spacing:** consistent 24 px outer horizontal inset; 20 px top inset; 24 px gap
  below navigation; 344 px library panel. Main buttons align in one dock rather than
  filling unrelated layout tracks. Native caption is retained for OS window behavior.
- **Colors:** dark blue-green glass, white/soft-gray text, teal selection and primary
  action. Disabled launch is visibly subdued. Actual blur is applied to the embedded
  background; foreground text and controls are not blurred.
- **Images:** existing generated voxel scene reused at cover scale; no generic image
  replacements. AMBER-derived silver mark with teal center/L. Gaussian is computed
  once, not on each frame. Panel texture caches keep the scene aligned on resize.
- **Copy:** real version/isolation/install-record hints replace fictional mock rows.
  Process-start status explicitly leaves world readiness unknown. Multiplayer says
  it is still under development. Main scene is decorative, not a live game preview.

## Interaction verification

- Clicked actual library row; selected identity and hero text updated together.
- Opened account/configuration panels; selected the pre-existing protected test
  account and discovered/selected Microsoft Java 25.0.4 x64. No new consent automated.
- Launch stayed disabled without all required choices. With choices complete,
  the new UI revalidated resources/account and opened the actual Vanilla 26.3 menu.
- Closing the launcher during the game requested normal exit, kept the launcher
  alive, observed normal game exit and then allowed launcher closure. No world entry
  was attempted this round; earlier user world/save acceptance remains separate.
- Verified 1280×820, 1000×820 and 1000×700 client layouts through native window sizing.
- Accessibility tree exposes named controls/list rows and task state. This is not
  complete Narrator/IME/DPI acceptance. Native app: browser-console checks do not apply.

## Follow-up polish and remaining acceptance

- P3: replace the raster lockup with an owner-provided vector master in a later asset
  pass; at its intended 154 px width it is readable, but large-scale print sharpness
  is not claimed. Custom per-instance artwork remains future work.
- IME composition, full keyboard traversal, high contrast/Narrator, monitor/DPI changes,
  large-library performance, clean-machine deployment and broader game compatibility
  remain open. GUI fresh login and GUI network installation were wired but not newly
  accepted here. Current tested launch reuses the existing authorized test account.

Implementation checklist: full scene, compact navigation, aligned dock, Gaussian glass,
corner mask, real selection/configuration/launch, normal-exit guard and resized layout
verified. No actionable P0/P1/P2 visual finding remains in the inspected states.
