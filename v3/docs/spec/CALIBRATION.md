# v3 CALIBRATION — the coordinate contract

**Status: the contract, written 2026-10-25. The live measurement that fills it is `seal-win`'s and is
still open (see §7).** This document is the v3 truth for what a calibrated value means and how it is
measured; [COORDINATES.md](../../../v2/docs/spec/COORDINATES.md) and
[CALIBRATION.md](../../../v2/docs/spec/CALIBRATION.md) in v2 remain the RECORD of the measurements
that produced these rules — cite them for evidence, not for current behaviour. The shape of the
values on the wire is [PROTOCOL.md](PROTOCOL.md) §3; which file they live in is
[CONFIG.md](CONFIG.md) §1; where a side is not built yet is [OPEN.md](../backlog/OPEN.md). One fact
lives in one place: this file does not repeat those.

---

## 1. One canonical space

Every calibrated value is in **physical pixels, relative to the game window's CLIENT area** — origin
at the client top-left, excluding title bar and borders. Not window-relative, not screen-absolute.

- **Why client-relative**: the player moves the window; moving it must not invalidate the
  calibration. The screen point is computed immediately before an action: `physical_client.origin +
  offset` (see [`DisplayInfo::cursor_target`](../../core/crates/seal-domain/src/geometry.rs)).
- **Why not the frame rect**: the frame includes the title bar and borders, whose size follows the
  theme and the window style, so a value stored in frame space moves when none of the game has. The
  frame rect travels in [`DisplayInfo`](PROTOCOL.md#3-types) for diagnostics only — **never as a
  coordinate origin**.
- **Never scaled by the display scale**: the physical space is the space the screen is painted in.
  Only the cursor-move conversion (§2) divides by the scale, and it does so per call.

What a mark is, and what later derives from it:

| Mark | Shape | What it means / what derives from it |
|---|---|---|
| Bag grid (shop) | rect: whole 8×8 bag | The single uniform pitch `w/8, h/8`; all 64 slot centres are derived from it |
| Bag slot (shop) | rect: one slot | The INDEPENDENT measurement; it must agree with the grid's pitch or the save is refused — two numbers that cannot both be right are a bad drag, not something to average |
| Shop list region | rect: exactly the visible rows | Row tops/bottoms are derived from it; a buy preset's scroll is notches down from the list's top |
| Focus / scroll point | point | An inert spot both shop tools left-click at run start, and where the wheel is aimed. Starting a tool means clicking the launcher, which leaves the game unfocused — and an unfocused game ignores the wheel and a right-click. It must not be over a row or a slot, where a click selects or buys. **v2 stored it as `buy_sell.scroll_point`; v3's kind is `scroll_point` and writes that same path** (the old `focus_point` kind wrote a field no consumer reads and was withdrawn) |
| MAX button | point | The count dialog's MAX, clicked after a bag/row click |
| Gem points | points: N, G, DG, Register, Combine, 3 resource slots | Absolute click targets; the composer places the cursor on one and clicks |
| Result gem area | rect + empty verdict | The crop the empty check compares against. The save carries the player's answer to v2's question "is the box empty?": **empty: true** saves the crop AND samples `empty_signature`; **empty: false** samples the signature only (no crop) — a crop of a busy box would poison the strong test. Its drawn border is excluded (see §5) |
| Tuner OCR region | derived, NOT a mark | The capture window for a tuner scan is the bounding box of THREE dragged boxes (grade area, attribute band, remaining band) plus the derived `row_height`; v2 never dragged the region itself. The one-rect `tuner_region` kind wrote an array where the load path parses an object and dropped the derivation, so it was **withdrawn** until the three-box calibration session is built (OPEN) |
| Grade area / bands | box and `[top,bottom]`/`[left,right]` bands, **region-relative** | The grade letter box and the row bands the OCR reader sub-crops. `row_height` is the measured attribute pitch (tighter than `height/3`); `grade_area` must lie inside `region` — the load-time rule is `ConfigValidator`'s |
| Spring point | point | Where the 發條 button is; only read in `spring_mode: hid`, when the tuner places the cursor itself |
| Pet marks | points: 目錄, pet feed icon, window X, page tabs, MAX (shared with buy/sell — the same count dialog) and the return slot in the bag; rects: per-row start/end toggle, feeder strip, boarding pet slot, the boarding bag's grid + one slot, and food cells as `[page, cell]` | The feeder's whole click path. The **boarding bag grid is its own**: the bag sits elsewhere on screen, and sharing the shop's numbers aims every click at the wrong item. The feeder strip is DIVIDED by the row's stack count — `FeederLayout` owns that arithmetic |
| Pet reference crops | base64 PNG in `local.yaml` | Identity for "did the pet go in?" and "is a pet in there now"; the `close_button_png` is session state the RUN learns, not something the calibrator draws |
| Tooltip | offset + shape | The hover panel the feeder reads growth / EXP% from before boarding; without it the feeder cannot tell a finished pet from a feedable one |
| ~~Quest step marks~~ | — | **Not real, in v2 or v3**: `QuestStep` carries only action/value/delay, and the tool clicks wherever the PLAYER placed the cursor during the countdown (the cursor guard watches it). The old row invented marks the executor never used and is withdrawn |

**Not coordinates, never scaled**: `gem.movements.*` are raw HID counts sent as `D dx dy` — hand-tuned,
not pixel deltas (see §2 and `MOVE-SETS.md` in v2); `gem.empty_signature`, `colored_gap_min` and the
grade colour masks are colours/thresholds. Scaling either by the display scale would be nonsense.

**The one exception**: the `ui:` block is the SHELL's own window geometry (its position and size in
its own coordinate space, not the game's client space), written only through `ui.window`
([PROTOCOL.md](PROTOCOL.md) §4/§7). It is machine state, not game calibration, and no tool reads it.

## 2. The two-space model — and why the core stays DPI-unaware

The core process is **DPI-unaware on purpose**; the screen blit is **1:1 physical**. That asymmetry
is measured and load-bearing, and the port copies it rather than "fixing" it into a scale.

- **Why unaware**: commit `7f9e1c7` (2026-09-08) recorded that `SetCursorPos` /
  `SetPhysicalCursorPos` return `ok=False` on a mixed-DPI two-monitor setup when the process is
  `PerMonitorV2`-aware — a hard blocker on the reference machine, and no coordinate conversion saves
  a call that fails outright. Full evidence, including the refuted anti-cheat theory, is
  `v2/docs/investigations/CURSOR-INVESTIGATION.md`.
- **Consequence**: every Win32 geometry call in the core is virtualised by the system scale, so
  `GetClientRect`/`ClientToScreen` return **logical** pixels while the blit is physical. A capture
  asked for the logical size grabs the top-left CROP of the client, not a scaled image — the bug that
  hid the minimap and hotbar from every screenshot (measured: physical client 2865×1789 at (12,47),
  logical 1910×1193 at (8,31)).
- **`GetDpiForWindow` is useless here**: it reports the WINDOW's awareness (96 for this DPI-unaware
  game window, even on an aware thread), not the monitor's scale. The scale is MEASURED.

### The double read

`Dpi.Measure` reads the same window twice — once normally, once on a thread temporarily switched to
`DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2` (`SetThreadDpiAwarenessContext`, thread-scoped and
restored in a `finally`), then derives:

```
scale        = physical client WIDTH ÷ logical client width     (2865 ÷ 1910 = 1.5)
monitor_dpi  = round(96 × scale)                                (144, shown as a cross-check)
```

- **Widths, not heights**: 1193 × 1.5 = 1789.5 truncates to 1789, so a height ratio comes back
  ~1.4996 and every derived coordinate drifts. The width ratio is exact. Pinned by
  `scale_is_measured_from_the_client_widths_not_the_heights`.
- **The ratio, not `GetDpiForWindow`**: the ratio is the actual transform between the two spaces and
  stays correct on a monitor whose scale differs from the system scale.
- **Nothing is hardcoded**: 125 % yields 1.25, 100 % yields 1.0.
- **Never cache**: re-measure per call. A mixed-DPI multi-monitor move changes the answer, and
  `WindowFinder`'s comment ("Re-query per call — never cache") is the rule that survived from v2.
- **Never call `SetCursorPos`/`SetPhysicalCursorPos` while a thread is aware** — that is exactly what
  broke in `7f9e1c7`. The core never calls either API; cursor positioning is the Arduino's (below),
  and the awareness switch exists only inside measurement and capture.
- [`DisplayInfo::from_rects`](../../core/crates/seal-domain/src/geometry.rs) is the ONE derivation of
  the struct [PROTOCOL.md](PROTOCOL.md) §3 prints; a zero-width logical client yields scale 1.0
  rather than infinity, so a minimised window stays measurable.

### Cursor moves are the Arduino's

The core does not position the OS cursor. A relative `D dx dy` goes to the HID device, which is why
a refused `SetCursorPos` cannot stop an action: `GemPointer.To` reads `GetCursorPos` (read-back always
worked), computes the error in the process's logical space, sends `D`, and repeats until within 2 px.
Measured gain is exactly 1:1 and a normal placement converges in one move.

**Not wired yet:** this closed loop is `seal-board::placement` — pure, over a `Cursor` and
`Transport` seam with a fake in every test. The OS adapters (serial open, `GetCursorPos`) are the
remaining piece (gap G2); until they land, no tool can move the cursor.

**A long move is walked out by the firmware in 10-px `Mouse.move` chunks with `delay(1)` between
them**, so a 600-px write keeps arriving after `Write` returns. The placement loop therefore polls
until two consecutive `GetCursorPos` reads agree (15 ms apart, bounded) instead of sleeping: the
fixed-20-ms version read a stale position and stacked a second move on the first, which is how a far
target overshot. **Callers must check the placement result** and stop the tool with the reason rather
than clicking somewhere arbitrary. Raw HID counts for the composer's tuned moves are the same kind of
value as `D` — never derived from a pixel delta.

`DisplayInfo::cursor_target` maps one client-relative physical offset into both spaces, and is the
only place the division happens:

```
logical  = logical_client.origin  + round(offset / scale)   // round-half-to-EVEN: C# Math.Round
physical = physical_client.origin + offset
```

The rounding is ties-to-even because `Math.Round` is; Rust's `f64::round` (away from zero) would move
a cursor to a pixel v2 never clicked. Pinned by
`the_logical_rounding_matches_c_sharps_round_half_to_even`.

## 3. Capture

- **`CopyFromScreen` only.** `PrintWindow` was measured to return a solid black frame for this game;
  do not reintroduce it.
- The blit is 1:1 physical, so the capture must be given **physical** rects and must run on the
  aware thread — the result is the full 2865 × 1789 client, not a crop.
- **The game must be in front.** `CopyFromScreen` reads what is on screen; a covered game would be
  photographed over. The core asks `GetForegroundWindow` and refuses with `game_not_foreground`
  ([PROTOCOL.md](PROTOCOL.md) §4/§6); the UI hides itself, waits a frame, then asks, so the
  calibrator's capture is the game only.
- **Frames carry their origin**: the frame is a rectangle of the physical screen, and
  [`Frame`](PROTOCOL.md#3-types) carries `origin` so the shell never has to guess where it came from.
  Frames are PNG+base64 for now; the core will retain the last four by `frame_id` (not built yet) so
  `calibration.save` references the frame the calibrator just took instead of shipping megabytes
  twice.

## 4. The shell's side — mapping a frame back to coordinates

The shell is DPI-AWARE, and its own window coordinates are irrelevant to tooling: it never asks
Win32 for a mapping, never calls `GetDpiForWindow`, and never assumes a scale. It draws the captured
frame and converts a drag rectangle with the `DisplayInfo` the core sent — the conversion is in ONE
place:

```
client_offset_of_frame_pixel = frame.origin + pixel − physical_client.origin
```

Everything in that formula is a field of `DisplayInfo`/`Frame` ([PROTOCOL.md](PROTOCOL.md) §3). If
the shell renders the frame scaled to fit, it divides the displayed pixel by the shown zoom BEFORE
the formula; `DisplayInfo.scale` is the cursor-space conversion (§2), not the canvas zoom, and using
it as one is how a calibrated point lands scaled-wrong. On `WM_DPICHANGED` the shell re-lays out and
re-derives from the next `DisplayInfo`; a canvas that cached a mapping across a monitor move is the
same bug v2's "never cache" rule exists for.

The exactness of this conversion is a step-1 spike ([PROTOCOL.md](PROTOCOL.md) §11.2): it is proven
on the reference machine at 150 %, and if it is not exact, `DisplayInfo` grows the field the
conversion needs rather than the shell deriving one from Win32.

## 5. Calibration artifacts

The MEASURED half of `local.yaml` — the eleven owned blocks (`settings` plus the ten machine blocks,
`calibration` among them, which also records the display environment the calibration was measured in)
— is [CONFIG.md](CONFIG.md) §1's subject. What matters here is that calibration produces ARTIFACTS,
not only numbers:

| Artifact | Written by (v2) | Role |
|---|---|---|
| `local.yaml` marks | each Save | The coordinates and derived geometry |
| `config/calib_tuner.png` | Save Tuner | Reference screenshot with the OCR bands drawn on it |
| `config/calib_gem.png` | Save Gem Composer | Reference screenshot with the click points and result box |
| `config/calib_gem_result.png` | Save Gem Composer | Crop of the result-gem area — what empty-result detection compares live pixels against |
| base64 crops IN `local.yaml` | calibrators / the run | Pet slot-empty references, the pet icon, the food icon, the close-X crop learned by the run |

**The empty-result crop is deliberately taken from the capture with the launcher HIDDEN, not from a
live grab.** A live grab once read the launcher's own dark UI and produced a meaningless verdict; the
reference must be a clean frame of the game. The composer asks whether the box is currently empty:
answer **Yes** and it saves the crop and samples `empty_signature`; answer **No** and empty detection
falls back to the weaker colour-signature test — the crop is the strong test, the signature only the
fallback.

**A save with `empty: false` samples the six-feature signature but does not delete an existing
`config/calib_gem_result.png`.** A crop captured while the box was genuinely empty stays a valid strong
reference for the same geometry, so discarding it on a busy-box recalibration would only weaken the
check; if the geometry changed, the crop's size no longer matches the new area and the gem run falls
back to the signature exactly as a missing crop does. The signature is re-sampled from the pixels the
player just marked, so the weaker fallback always describes the current box.

The crop comparison ignores the box's drawn **6-px border**: the frame moves by a pixel when the
window moves, and the border rows alone scored an empty box 7.7 % — above the gate, so the composer
never advanced again. Measured from the interior: empty 0.000, gem 0.55, gate `empty_distance` 0.01
— an enormous margin on both sides, and colour/shape-blind on purpose ("is this still the same
picture?"). The check only judges while the game is the FOREGROUND window; otherwise it answers "not
empty" (the safe direction — keep combining rather than advance a grade). The decision half of that
rule is `seal-domain/src/tools/gem.rs`.

**No auto-anchor.** A different game window size means RECALIBRATE. The size ratio is deliberately
not used to transform stored coordinates: the game UI does not scale linearly (the HP bar keeps its
pixel size while the world view expands), so a proportional transform would place points wrongly.
`reference_window` records the size but is otherwise unused for exactly this reason.

## 6. CalibrationCheck and the Detect flow

The Setup surface answers one question: does the display environment still match the one this
calibration was measured in? The rule is a stored field **carries a value AND it moved**:

- A field the stored calibration does not carry is NOT a difference — an install that has never been
  calibrated has nothing to be wrong about, and the surface says "nothing stored yet" separately.
- Three fields, because each silently invalidates every stored coordinate at once: **scale** (the
  physical space stops meaning what it meant), **client size** (every client-relative point lands
  elsewhere), **screen** (the calibration was measured on a display that is no longer there).
- The scale is compared with a tolerance: it is rounded to 4 places on save, so `1.49999996` is the
  same scale. `> 0.001` is a mismatch; exactly 0.001 is not. The message formats each scale as C#’s
  `{0.###}` — at most three decimals, trailing zeros trimmed — because the player reads it.
- Order is scale, client, screen; all moved fields are reported together, not first-error-wins.

The comparison is pure and lives in
[`seal-domain/src/calibration.rs`](../../core/crates/seal-domain/src/calibration.rs), with v2's seven
cases translated in `tests/calibration.rs`. The LIVE side — measuring a fresh `DisplayInfo` to compare
against — is `seal-win`'s (§7); the surface that shows the lines is the shell's, via the protocol.

Different scale with the same game resolution is NOT a recalibration: the stored physical coordinates
remain valid, and the only change is the physical → logical conversion for cursor moves, which uses
the CURRENTLY measured scale (§2).

## 7. What is open

Nothing in this document is invented: where v3 has no implementation, the v2 behaviour is cited above
and the open item is named here rather than restated. The living list is
[OPEN.md](../backlog/OPEN.md) §3/§4, with the protocol's own open items in
[PROTOCOL.md](PROTOCOL.md) §11.

The **named-instance kinds** carry a `target` (PROTOCOL §12.8): `pet_page_tab` (`ITEM1`..`ITEM3` ->
pet.page_tabs), `pet_toggle`/`pet_boarding_slot`/`pet_feeder_strip` (row `1`..`4` ->
pet.slots[i].{toggle_label, boarding_pet_slot, feeder_strip}), `gem_grade` (N/G/DG/Register/Combine ->
gem.grade_positions, the absolute click targets the composer places the cursor on) and `gem_resource`
(`1`..`3` -> gem.resource_gems). The pet's single points (menu 目錄, feed icon, window X) are their own
kinds with no target. A mounted mark that names a row or page the config does not carry is refused,
and a per-row write leaves that row's other fields — stacks, reference crops, the other two marks —
exactly as they were. The pet's reference crops, the food cells/queue and the sell-slot selection stay
explicitly OUT of the kind table: they are live-screen session features (OPEN).

`calibration.save` and `calibration.fetch` are implemented: the kind table lives in
`seal-core/src/calibration.rs` and accepts `bag_grid`, `bag_slot`, `shop_list`, `scroll_point`,
`max_button`, `result_gem_area` (with its `empty` verdict) and `spring_point`; marks go through the
one `local.yaml` write gate, and the result-gem crop is the first binary artifact. The kind table was
REPAIRED after the gap audit (`PROTOCOL §12.7`): `focus_point` wrote a field no consumer reads and
became `scroll_point`; `max_button` had a mark with no kind and now has one; `result_gem_area` lost
v2's empty verdict and got it back; `tuner_region` was withdrawn rather than faked (the three-box
session is a plan), and the invented quest-step row was removed. Still open:

| Not built yet | What it needs | Pointer |
|---|---|---|
| The remaining mark kinds | the pet points/rects and the gem points need the optional `target` on `calibration.save`/`fetch` (named instances: page tab, row); the tooltip offset + shape needs v2's two-capture hover session (it cannot be a single-frame mark); the pet crops/cells and the sell-slot editor are Pet-tab features, not calibration kinds. Today they are hand-editable and preserved, not calibrated from the UI | PROTOCOL §4; this file §1; OPEN |
| The tuner's three-box session | v2 dragged grade area + attribute band + remaining band and DERIVED `region`, the bands and `row_height`; a session-shaped calibration (several marks, or several commands with a documented order) is the only way to keep that derivation. Until then the tuner OCR geometry is hand-edited and `tuner_region` is refused | OPEN; `startup.rs`'s load rules |
| The full reference screenshots (`calib_tuner.png`, `calib_gem.png`) | only the result-gem CROP exists as an artifact; the drawn-over screenshots are v2 conveniences no tool reads | this file §5 |
| `config.changed` for calibration writes | LANDED: a save now queues `config.changed {rev, block, values}` for the block it marks, after its response (PROTOCOL §5), so a shell keeping a mirror updates without refetching | PROTOCOL §5 |
| Frame→offset exactness at other scales | step-1 spike; if it is not exact, `DisplayInfo` grows a field | PROTOCOL §11.2 |
| Detect surface / Detect flow | the shell's Setup canvas + a protocol path for the comparison result | OPEN §4 |
| The boundary a measurement must match | `CalibrationCheck` is ported and tested in the domain; `calibration.save`'s optional `display` argument now records the environment it compares, but no Detect command consumes the comparison yet | this file §6 |
