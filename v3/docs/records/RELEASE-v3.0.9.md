# Release v3.0.9 - calibration captures become screen-first

**Date:** 2026-11-06 繚 **Branch:** `main` 繚 **Built from:** `e7ec227`

v3.0.9 makes calibration work with the way the game actually behaves. A calibration pass can now keep
**twelve live screens**, assign a different screen to each mark target, and reuse the same shared
Capture bar for the Tuner and Tooltip sessions. A screen can be deleted on purpose with a two-step
confirmation; deleting it never touches saved calibration coordinates. This release also carries the
Claude-review Phase 1/2 fixes and the firmware watchdog/abort work, so the firmware zip is NOT
byte-identical to v3.0.8; reflash is required for the new firmware level.

## Artifacts (built 2026-11-06, published as GitHub release assets)

| artifact | bytes | SHA-256 |
|---|---|---|
| `dist/SealTools-v3.0.9.zip` | 46,132,285 | `B9709F8B5B542B5B301F0910FAC26CBAD8D35954E4734FDA509BCD7D3AD1C234` |
| `dist/SealTools-v3.0.9-firmware.zip` | 6,574 | `F7BDD22E0395C5EC83FD7842B522C3BFC0F5BF347844032612D7C7EB8C425DD4` |

`dist/SHA256SUMS.txt` carries the lowercase hashes. The stage, app-zip and firmware audits all PASS.
Signing is NOT performed: the payload is unsigned (SHELL-F9); verify the SHA-256 before running.

## What changed from v3.0.8

### Calibration captures are screen-first

- **Twelve live frames** (was four). A whole Shop/Gem/Pet pass can be captured before saving.
- **Per-target screen assignment:** each mark target has its own screen, keyed by `kind`/`kind@target`.
  A target with no screen starts empty and Save is disabled; it never borrows another tool's newest
  frame. The **Screens** strip previews every live capture, labels it by tool (`Shop · 1`) and shows how
  many targets use it. Assigning is explicit (the editor's Screen row or **Use this screen for this
  target**); a capture auto-assigns only when the selected target has no screen yet.
- **Sessions reuse the same bar:** Tuner draws its three boxes and Spring point on the selected shared
  screen. A screen change clears the Check and disables Save until **Check boxes** runs again. Tooltip
  uses the selected screen as its visual background; **Capture hover** remains the live cursor anchor
  for **Test read**. The Tuner's duplicate private Capture screen button is gone; the shared bar owns
  capture. Right-side session editors keep their existing layouts.
- **Delete a screen:** every tile has a small Delete action. Two confirmations are required; only the
  second calls `capture.discard`. The live frame and session-only assignments are removed, while
  **saved marks and coordinates are never modified** - the dialogs say so and the tests pin it.
- Capture quality fixes: the shell hides, waits for the composited hide (`DwmFlush` + 30 ms), captures
  with the calibration exemption, then shows. Assignments are session-only; saved marks store no frame
  id. Eviction still clears assignments and warns; Save against an evicted frame is refused.

### Review and robustness fixes

- Claude-review Phase 1/2 fixes across the shell and core (the eight `SEAL-*` test-pass issues and the
  follow-up review batch), including input/foreground handling, config writes, tooltip/tuner checks and
  the shipped specs; the full `feature/seal-issues` series is in `records/PROGRESS.md` (entries 1-49).
- The legacy `v1/checkin/makeup.py` helper is now tracked; it contains no credentials or session data
  and reads its config/session from the existing `checkin.py` files.

### Firmware level 3 - REFLASH REQUIRED

`arduino/seal_mouse/seal_mouse.ino` is now `FW_VERSION 3`:

- hung-host failsafe: 3 s without a host byte releases a held spacebar or left button;
- `!` aborts an in-progress move/wheel frame instead of holding the queue;
- move counts are parsed wide and bounded (no 16-bit wrap moving the pointer the wrong way);
- overlong frames are discarded rather than split and re-parsed.

The firmware zip was rebuilt and audited. A v3.0.8 board reports firmware level 2 and must be reflashed
for the watchdog/abort/bounds behaviour.

## Verification

- Rust: fmt, clippy (pedantic, `-D warnings`) and rustdoc clean; **95 suites, 1,357 passed, 0 failed,
  6 ignored**.
- Shell: `dart format`, `flutter analyze --fatal-infos`, **380/380 tests**, Windows debug build.
- Release pipeline: Rust and shell gates, release core with `--locked`, Flutter release bundle, stage
  audit, app-zip audit, firmware source+zip audit; all PASS.
- `build-info.json` in the stage records `product_version 3.0.9`, `git_commit e7ec227`, branch `main`,
  `git_dirty false`.

## Upgrade notes

- Config format is unchanged; no migration.
- The twelve-frame cache and screen assignments are session state; after restart, select/capture screens
  again. Saved marks and coordinates are unaffected.
- Reflash the Arduino with the v3.0.9 firmware zip if you want the new watchdog/abort behaviour.
- Windows SmartScreen/antivirus may flag an unsigned input-driving tool; compare the SHA-256 above.
