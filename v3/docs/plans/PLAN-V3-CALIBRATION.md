# Plan — v2's full calibration feature set in v3

**Proposal.** Nothing here is built. This document describes work not yet done; its state lives on the
[plans board](README.md) and nowhere else in this file.

Why it exists: the player asked for **all of v2's calibration features** in v3 — *"the calibration
feature also need to be created — u also need to create all the calibration feature exist in v2"* —
and fixed the acceptance in the same breath: *their v2 config geometry is correct for their TW-live PC,
and the same config must produce the same behaviour with **no re-calibration***.

The generic Calibrate tab is not that feature. It saves 18 individual marks well
([CALIBRATION.md](../spec/CALIBRATION.md) §1, [PROTOCOL.md](../spec/PROTOCOL.md) §4) and it is the
right foundation, but v2's four Calibrate surfaces are **sessions**: several captures, derived
geometry, reference crops, pointer tests, live reads, and reference screenshots with the marks drawn
on them. This plan turns the 18-mark surface into those four sessions plus **Setup → Detect**, and
records the protocol, the increments, and the decisions that cannot be guessed.

Sources: v2 [USER_GUIDE.md](../../../v2/docs/spec/USER_GUIDE.md) (Calibrate Tuner §607, Calibrate Gem
§622, Buy/Sell §706, Calibrate Pet §764, Calibrate Tooltip §890, Setup §945),
v2 [CALIBRATION.md](../../../v2/docs/spec/CALIBRATION.md),
v2 [CONFIG.md](../../../v2/docs/spec/CONFIG.md), v2 [MOVE-SETS.md](../../../v2/docs/spec/MOVE-SETS.md),
and the v2 implementation in `v2/SealTools.Launcher/MainWindow.xaml.cs` and
`v2/SealTools.Core/*.cs` (cited below where the guides understate a rule). v2 has **no view models** —
the tabs and their rules are code-behind; that absence is why those rules must move to the core here
("no widget keeps a rule", [DESIGN.md](../spec/DESIGN.md) §2).

---

## 1. What "the same behaviour" means

Two independent claims, so acceptance cannot hide in one of them:

1. **A v2 `local.yaml` loads unchanged and every derived value matches.** The typed machine model
   already parses every v2 field — `LocalOverrides` carries `calibration`, `tuner.ocr`/`spring_point`,
   `gem.*`, `buy_sell.*` (including `sell_slots`/`sell_cap`), `pet.*` (including `slots`, `queue`,
   `page_tabs`, `food_slots` and the base64 crops) and `tooltip` — and
   `seal-domain/tests/v2_config_replay.rs` already pins a sanitised v2-era file. This plan extends that
   proof from *decodes* to *derives*: with no capture and no recalibration, each surface's read-only
   verification must produce the same geometry, the same derived regions, the same counts and the
   same verdicts v2 did.
2. **A from-scratch recalibration reproduces v2's numbers.** Dragging the same three tuner boxes,
   marking the same gem points, the same pet strip etc. must save the same values v2's code derived
   from them (the formulas are in §3.6), so the player's next machine is calibratable with the new UI.

Not part of the ask: tool behaviour, new config keys, new firmware. No config key is added (§3.7) and
the firmware is untouched.

---

## 2. What v3 already has — do not rebuild it

| Capability | Where it is today | State |
|---|---|---|
| Every v2 machine-config field, typed, v2 file loads | `core/crates/seal-domain/src/config/local.rs`; `seal-domain/tests/v2_config_replay.rs` | built |
| DPI-unaware measurement + `DisplayInfo` + `cursor_target` | `seal-domain/src/geometry.rs`; [CALIBRATION.md](../spec/CALIBRATION.md) §2 | built |
| Capture, frame origin, 4-frame `frame_id` cache | `seal-core/src/win.rs:151-190`, `dispatch.rs:990-1012` | built (an old PROTOCOL §8 line calling the cache "not built" is stale) |
| Hide → one frame → capture → show choreography | `shell/lib/ui/tabs/calibrate_tab.dart:277-340`; `shell/windows/runner/flutter_window.cpp:32-47` | built |
| 18 marks + named-instance targets + result-gem crop artifact + `config.changed` | `seal-core/src/calibration.rs` (KINDS :70-215, save :568-851, fetch :854-922); `shell/lib/ui/tabs/calibrate_tab.dart` | built |
| Generic canvas: click/drag → mark, zoom-aware, fields authoritative | `shell/lib/ui/tabs/calibration_widgets.dart:36-253`, `calibrate_tab.dart:415-449` | built |
| Bag grid: pitch, 64 centres, disagreement rule | `seal-domain/src/tools/shop.rs:68-140` (ported from v2 `BagGrid.cs`) | built |
| Shop row derivation (region → row centres) | `seal-domain/src/tools/shop.rs` (ported from v2 `ShopGeometry.cs:44-67`) | built |
| Feeder layout: strip → slot boxes, count region, ETA region | `seal-domain/src/tools/pet.rs:288-370` (ported from v2 `FeederLayout.cs`) | built |
| Display-environment comparison (`CalibrationCheck`) | `seal-domain/src/calibration.rs:34-90` (v2 `CalibrationCheck.cs:23-45`) | rule built, no command/surface |
| Attribute matcher + filter verdict | `seal-domain/src/ocr/matcher.rs:67-230` | built |
| Result-gem empty verdict (crop-first, colour fallback) | `seal-domain/src/tools/gem.rs`; [CALIBRATION.md](../spec/CALIBRATION.md) §5 | built |
| Composer cycle steps (the 21-click cycle's rule) | `seal-domain/src/tools/gem.rs:424+`; v2 `MOVE-SETS.md:92-102` | decision-half built |
| Closed-loop cursor placement, click, tuned `D`, wheel, hover primitives | `seal-core/src/effects.rs` (`place_at`/`click_at` :68-90; `Effects` :92-175); `seal-board/src/placement.rs` | built |
| OCR pipeline + `ocr.scan` modes | `seal-core/src/dispatch.rs:230-324`; `seal-ocr/src/modes.rs` | built (no upscale ladder — OPEN §2) |
| Reference-screenshot loader at startup (v2 `LoadCalibrationImages`) | v3 has no equivalent | **gap** |

The four v2 surfaces are therefore **not a port of geometry** — the geometry is already in the core.
They are a port of **the sessions around it**: captures, crops, drafts, live tests, artifacts, Detect.

---

## 3. Feature inventory, control by control

Each table is the v2 surface to reproduce. "Rules" citations are for behaviour the guide does not
state; every one of them must end up in the core, not the widget.

### 3.1 Calibrate Tuner (v2 [USER_GUIDE.md](../../../v2/docs/spec/USER_GUIDE.md):607-618; [CALIBRATION.md](../../../v2/docs/spec/CALIBRATION.md):43-57)

| Control | v2 behaviour | Rules understated by the guide |
|---|---|---|
| Capture 發條 window | full physical grab, launcher hidden, blink expected | — |
| Canvas | drag **three** boxes in order: grade letter, three attribute lines, spring count; colour-coded (green/blue/orange); a drag <4 px is ignored | `MainWindow.xaml.cs:3764-3771`; `TunerCapture` resets the three boxes + spring point (:3699-3720) |
| Spring point | single click in the capture; optional; only read in `spring_mode: hid`; **Test Click** places+clicks it | `TunerSave` writes it only when clicked; re-saving without clicking **clears** it because the block is rebuilt (:3872-3884) |
| Check OCR | full read → match → filter on the **current screen**; if all three boxes are dragged it uses those, else the saved `tuner.ocr` and says so; prints grade, remaining, the three attribute rows, `Matched:`, `Filter:`; **measures real attribute pitch** into `row_height` | `CheckTunerOcr:3799-3853`; geometry build :3946-3977; pitch algorithm `OcrEngine.cs:591-605` (collapse tops <3 px, median gap); fallback `attr.Height/3` only when not measured |
| Save Tuner | writes `tuner.ocr` + `tuner.spring_point` + the `calibration:` environment block; saves `config/calib_tuner.png` with the three bands drawn | all three boxes required, region ≥20×20; artifact drawn at natural size (`SaveTunerCalibrationImage:3584-3613`) |
| Result | one mono status/output pane | — |

### 3.2 Calibrate Gem (v2 guide:622-704; CALIBRATION:59-92, MOVE-SETS whole)

| Control | v2 behaviour | Rules understated by the guide |
|---|---|---|
| Capture gem window | one full grab | — |
| Points | click in order **N, G, DG, Register, Combine**, then **Resource1-3**, then drag the composed **result gem** box | `GemMouseDown:4011-4033`; step hints :4004-4009 |
| Coordinates grid | typed N/G/DG/Register/Combine + Resource1-3 + Result area x/y/w/h; **Save Coordinates** persists without a capture | `SaveCoordinates:4256-4291` — independent of the point/save flow |
| Tests (from/to dropdowns) | **Place cursor + click** (to point, no focus on cursor API); **Test tuned move** (click from, send raw `D dx dy`, click to); **Check Result Colour** (sample now, report colour + distance to empty reference + verdict); **Sample result gem** (same with channel spread / dominant tone detail); result-box crop preview | `GemTestClick:3131-…`; `GemTestMovement:3084-3116` (sends the RAW counts); `GemCheckResultColor:3464-3472`; `GemTestResult:3477-3492` (spread/dominant); preview from the capture / saved crop (:2737-2751, 4073-4108) |
| Move set | `Composer move mode` = `tuned`/`arduino`; inactive grid dimmed 45 % and disabled; selector on its own card | :2908-2923, :2965-2976 |
| Moves — tuned | one row per route (radio_N/G/DG, register_combine, combine_register, register_slot1, slot1_slot2, slot2_slot3, slot3_n/g/dg) with dx/dy and per-row **Send**; **Save tuned counts** writes `gem.movements` only | :2840-2929; parsers :3042-3080; `SaveComposerMoves:4293+` |
| Moves — arduino | same routes, destination point shown, per-row **Run** = click source, place closed-loop, click target | :2931-2963; `MOVE-SETS.md:72-87` |
| Run one full cycle | 21 real HID clicks: N (select/register/combine, deregister+register, combine, right-click 3 resources), G same, DG combine once; pre-resolves every point before clicking; logs one line to `logs/arduino_debug.txt` | step table `MainWindow.xaml.cs:3349-3434`; `MOVE-SETS.md:87-102` |
| Advanced | **Diagnose capture** (frame/client/non-client rects + sample PNG), **Debug Cursor (logical)**, **Debug Physical** | :2677-2735, 4295-4315 — diagnostics only |
| Save Gem Composer | all points + result area required; **empty-result question** answered from the calibration screenshot (not a live grab); `empty:true` saves `calib_gem_result.png` + samples `empty_signature`; `empty:false` samples signature only; writes positions/resources/area/signature/distance + move mode; saves `calib_gem.png` with yellow points + magenta box | `GemSave:4130-4218`, `SaveResultGemCrop:4109-4128`, `SaveGemCalibrationImage:3558-3582`; **moves are not touched** (:4181-4184) |

### 3.3 Buy / Sell calibration (v2 guide:706-762; CALIBRATION:94-142)

| Control | v2 behaviour | Rules understated by the guide |
|---|---|---|
| Capture bag window | one grab with **shop open, bag open and count dialog showing** — one frame covers every mark | hint :5364-5367 |
| Bag grid | **Draw grid area** (whole 8×8), then **Draw one slot**; both required; slots | `BsMouseUp:9750-9802` (20 px minimum); pitch rule `BagGrid.cs:66-110` (tolerance `max(5 px, 0.25 × pitch)`, refused not averaged) |
| Show 64 centres | draws grid-derived centres + derived shop-row dots + list-region box + focus + MAX, colour-coded, redrawn on resize | `BsRedrawOverlay:9841-9870` |
| Shop | **Draw list region** (exactly the visible rows), **Mark focus point** (single click, must be inert; both tools click it to focus the game), **Mark MAX button** | `ArmPoint/ArmDrag:9630-9686`; `ShopGeometry.cs:9-19,44-67` (rows=10, pitch=height/rows, centre X) |
| Save Calibration | grid + slot + list + focus + MAX into `buy_sell`; grid and slot must agree; saves **no screenshot** | `BsSave:9945-9961`; v2 CALIBRATION:219-229 |
| Setup so far | `ok`/`--` checklist per mark | `RefreshCalibChecklist:9902-9917` |
| Test scroll | notches + Scroll up/down (Q/Z): clicks the focus point first, then sends n notches; cap 400 is a guard | `BuySellTestScroll:9963-10030` |
| Sell grid | **Sell tab**, not the calibration tab: 8×8 bag picker toggling slots (plus per-row shortcuts), selection shown against the cap, **dry run** walks the cursor without clicking; selection is in-memory, never persisted | `BuildSellTab:4743-4802`, `ToggleSellSlot:5331-5354`, dry run :5239-5274; v2 CONFIG.md "the sell selection is deliberately never saved" |

### 3.4 Calibrate Pet (v2 guide:764-888; CALIBRATION:144-161; PET-TAB-DESIGN.md)

| Control | v2 behaviour | Rules understated by the guide |
|---|---|---|
| Capture game | **two captures expected**: icon panel open first (mark 目錄 + pet feed icon), then the boarding window (everything else); marks already placed are kept and re-capture only swaps the background | comment `MainWindow.xaml.cs:5874-5885`; `PetCapture:7428-7446` |
| Point marks | 目錄, pet feed icon, boarding X; single clicks | `PetSetPoint:7369-7392` (page tabs in the same switch) |
| Bag page tabs | ITEM1/2/3 points; absolute tabs, not next/prev | same |
| Breeding rows | row strip selects the row being edited; **+ Add row** up to 4; **Food slots in this row** (capacity: 2 free / 5 paid); rows have their own geometry and are enabled/disabled on the **Pet tab** | `EditRow`, add/remove :7369-7550; capacity is a property of the game, not a preference |
| Per-row geometry | **Draw start button**, **Draw pet slot**, **Draw food strip** (ONE box across the row's slots); four editable x/y/w/h numbers for the strip that re-derive slot boxes and crops as you type | drag targets :7394-7426; min drag **10 px** (:7505-7512); `FeederLayout.SlotBoxes:35-52` (integer slot edges), `ReadRegion:123-132`, `EtaRegion:105-120`; count crop = 40 % into each slot, FULL height (:55-72) |
| Read the counts | **Test read**: every ticked row, every slot, on that row's own strip; per-slot raw `text@score` list and the agreed count, with debug crops per rung; walks the same upscale ladder a run walks (3×, 9×, 24×) and hides the launcher ONCE for the whole read | `PetTestRead:8350-8470`; `PetTool.CountReadUpscales:1485` |
| Count knobs | **Count crop starts at** (fraction, 0.40 measured) and **Min score** (0.90-1.00 real vs ≤0.63 food-icon junk) | `FeederLayout.ReadLeftFraction:72`; `FeederCount` |
| Bag grid (its own) | whole 8×8, one slot, **Show 64 centres** — the boarding bag is NOT the shop bag and does not reuse those numbers | v2 guide:856-866 |
| Empty-slot reference | **Capture empty pet slot**: crops the selected row's pet-slot box from the capture, stores base64 in `local.yaml.pet.slots[i].pet_slot_empty_png`, previews it; must be captured with the slot empty; without it the occupancy check is skipped | `PetCaptureEmptySlot:9559-9590`; preview :7970-8005 |
| Save Calibration | writes the **calibration half only** (`LocalPet.ApplyCalibration`), leaving queue/food cells/booleans exactly as the file had them; blocks on bag grid/slot disagreement; saves `config/calib_pet.png` (plain screenshot — v2 CALIBRATION's file table omits it but the code writes it) | `PetSave:9598-9628`; `SavePetCapture:8991-9012`; loader/redraw :9016-9050 |

### 3.5 Calibrate Tooltip (v2 guide:890-921)

| Control | v2 behaviour | Rules understated by the guide |
|---|---|---|
| Capture game | for marking the hover point (the offset is **not** measured from this frame) | `TooltipCapture:5573-…` |
| Mark hover point | single click on the item whose panel appears on hover | `TooltipMouseDown:5688-5717` |
| Hover and capture | tool **places the cursor** on the point, clicks to focus, waits, captures; then drag a box around the panel | `TooltipHoverAndCapture:5648-5686`; focus-then-hover helper |
| Offset rule | **offset = box top-left − the point the tool placed the cursor on**; size = the box; nothing typed or eyeballed; box ≥20 px | `TooltipMouseUp:5728-5770` (offset formula :5754-5766) |
| Hover delay (ms) | a field, not a constant (700 ms measured then found wanting); the delay before the panel is read | :5543-5553 |
| Save | writes `tooltip` offset/size/delay to `local.yaml` | `TooltipSave:5860-5872` |
| Test read | hovers again, reads at **point + offset**, reports lines, saves the crop it was given to `logs/reads/tooltip_*.png` | `TooltipTestRead:5807-5857` |

### 3.6 Setup → Detect (v2 guide:945-981; CALIBRATION:26-41)

| Control | v2 behaviour | Rules understated by the guide |
|---|---|---|
| Detect | measures the live game window: monitor physical size, DPI, scale, frame and client rects; fills the fields | `BuildSetupTab:1489-1508` |
| Scale / Reference client size | **editable** if detection is wrong; prefilled from stored config on tab build | :1510-1513, :1554-1563 |
| Save Setup | writes `calibration:` = `dpi_scale` (rounded 4), `monitor_dpi` = round(96×scale), `client_size`, `screen` (from the last Detect), `measured_at` | :1530-1549 |
| Stored calibration | saved scale/client/timestamp, or "none yet" | `ShowStored:1479-1487` |
| ⚠ warning | compares stored vs live on tab build, on tab re-entry and on window activation (not only on Detect); wording for "nothing stored", "game closed — not compared", "matches", and "differs — scale/client/screen changed; recalibrate" | `CheckAgainstStored:1435-1477`; scale tolerance 0.001 (`CalibrationCheck.cs:27-45`) |

### 3.7 Files and artifacts produced (v2 CALIBRATION:219-242 + code)

| Artifact | v2 writer | Content |
|---|---|---|
| `config/local.yaml` blocks | every Save | the values, split per tab's half |
| `config/calib_tuner.png` | Save Tuner | natural-size capture + 3 colour-coded band rectangles |
| `config/calib_gem.png` | Save Gem Composer | natural-size capture + yellow point dots + magenta result box |
| `config/calib_gem_result.png` | Save Gem Composer (empty only) | the result-box crop the empty check compares against |
| `config/calib_pet.png` | Save Calibration (pet) | plain capture, marks redrawn from config on reopen |
| base64 crops in `local.yaml` | pet calibrator / run | `pet_slot_empty_png`, `close_button_png` (learned by the run), `food_icon_png`, queue-entry `png` |
| none | Buy/Sell | deliberately no screenshot; verification is the overlays |
| none | Tooltip | only the numbers |
| `logs/reads/*`, `logs/captures/result_box.png` | diagnostics/test reads | debug crops |

**No config key is added by this plan.** The v2-era `local.yaml` already carries every block and
artifact reference (`local.rs:33-344`), and v3's opaque merge keeps keys it does not model
([CONFIG.md](../spec/CONFIG.md) §3). The 18-mark `calibration.save/fetch` stays as the generic mark API;
the session commands below are additions, not a second config path.

### 3.8 Rules the guides understate, collected

- Tuner geometry is **derived, never dragged**: region = bounding box of three boxes; `grade_area` and
  the bands are offsets from that region; `row_height` = measured pitch (median of collapsed line
  tops, `OcrEngine.cs:591-605`) with `attr.Height/3` only as a fallback (`MainWindow:3946-3977`).
- Bag pitch: a slot is naturally smaller than the pitch (the gap); tolerance `max(5 px, 0.25 × pitch)`,
  and a disagreement is refused (`BagGrid.cs:66-110`).
- Shop rows: `rows = 10` (a game property, deliberately not exposed), pitch = region height / rows,
  row Y = top + (k + 0.5)×pitch (`ShopGeometry.cs:20-67`).
- Pet count crops are 40 % across the slot to its right edge, at the slot's FULL height; the ETA region
  is ratios of slot height, not fixed pixels (`FeederLayout.cs:55-120`).
- Min drag thresholds differ per surface: 4 px tuner, 10 px pet, 20 px buy/sell/tooltip.
- Gem `empty_distance` documentation disagrees (v2 CALIBRATION §6 says default 0.01, v2 CONFIG.md says
  0.18; v3's `local.yaml.example:47-52` says 0.18). The value must ride the player's file and must not
  be default-migrated.
- Empty-result reference comes from the **calibration screenshot** (launcher hidden), never a live
  grab; the crop compare ignores the box's 6-px border (v2 CALIBRATION:179-217; v3 CALIBRATION §5).
- Per-tool save scoping: each Save writes only its half of the v2-machine block (pet calibration leaves
  queue/food cells/flags; gem Save Gem Composer leaves `gem.movements`; moves/mode are separate saves).

---

## 4. Gap map

| (a) Generic mark saves | (b) Derived-geometry sessions | (c) Crop/reference captures | (d) Pointer actions | (e) Live test reads | (f) Artifact overlays | (g) Setup Detect |
|---|---|---|---|---|---|---|
| **exists** — 18 marks, targets, result-gem crop, `calibration.save/fetch` | **missing as commands**: v2 derive rules are already ported in `seal-domain` (tuner geometry, tooltip offset, bag/shop/pet derivations), but nothing invokes them from a draft + frame | **missing**: no way to save a reference crop (pet slot, queue icon, food icon) or a screenshot artifact | **primitives exist** (`effects.rs`), no protocol command; no arbiter/lease story for a calibration action | **partial**: `ocr.scan` has modes but no caller geometry, no matched/filter verdict, no row-height measurement, no upscale ladder, no per-slot batch | **partial**: core can write a crop (`write_artifact`) but has no overlay renderer; shell can draw previews from config + a screenshot | **missing**: comparison rule exists (`CalibrationCheck::differences`), no command and no surface; shell never writes `ui:`/`calibration:` through a calibration path |

Per surface: **Tuner** needs (b) + (e) + (f); **Gem** needs (b) + (c, one crop) + (d) + (e) + (f);
**Buy/Sell** needs (b, preview/save) + (d, test scroll) + shell overlay, no artifact; **Pet** needs
(b) + (c) + (e) + (f) + a reader upscale ladder; **Tooltip** needs (d: place→click→wait→capture) + (e).
**Setup** needs (g) only. All of them need the shell's canvas to become a **session canvas** (draw →
draft → preview → save) rather than one-mark-at-a-time fields.

---

## 5. Protocol additions (proposed)

### 5.1 Principles (consistent with v3's rulings)

1. **Stateless commands over drafts, not a core session object.** The shell already holds config drafts
   ([PLAN-V3-SHELL.md](PLAN-V3-SHELL.md) §5, `shell_model.dart` draft machinery); a calibration
   session is a draft plus one or more `frame_id`s. The core keeps only frames (existing FIFO) and
   state it already owns. This avoids a session lifecycle in the protocol, keeps replay fixtures
   stateless, and means a crashed shell leaves no calibration session in the core.
2. **The core owns every derivation and every user-facing check sentence**; the shell draws the
   draft's marks, sends rectangles/points already mapped with `DisplayInfo`, and renders results.
   No widget computes a pitch, an offset, a row height or a verdict.
3. **The core is the only writer**, including binary artifacts (the existing `write_artifact`
   temp-and-rename path, [CONFIG.md](../spec/CONFIG.md) §3/§5). The shell writes no file.
4. **Writes are atomic per surface and revisioned like `config.apply`**: validate → derive → write
   artifacts → write block(s) in one read-modify-write → bump `rev` → echo the block → emit
   `config.changed` after the response. A refusal writes nothing and consumes no rev.
5. **Frames are references, not bytes**: every session save takes the `frame_id`s its crops/overlays
   come from; an evicted/unknown id is `bad_request` and the shell re-captures (existing rule,
   PROTOCOL §8). Multi-capture surfaces (pet, and hover→capture) capture inside the command because
   the frame only exists after a pointer action; where the capture must suppress the shell window,
   the shell hides/shows around the command (existing host channel) exactly as for `capture`.
6. **Live test actions are gated like tool input**: they acquire the same input gate/port lease a
   tool does (v2 `AcquireInputAsync`), are refused with `tool_busy` while a foreground tool holds the
   game, wait for the resident feeder's bounded reload rather than fighting it, and release held keys
   through the RAII guard. Nothing here bypasses the arbiter.
7. **Additive to protocol `v:1`**: both halves ship together; existing commands/fixtures keep their
   shape; a new command is a new fixture, not a version bump. (`bad_version` remains the guard for a
   mixed pair.)
8. **No new error codes**: reuse `bad_request` (malformed args, draft fails a rule — the domain's line
   in `message`/`detail.errors`), `io` (artifact write), `board_unavailable`, `game_not_foreground`,
   `window_not_found`, `not_ready`, `tool_busy` (calibration action vs running tool), `internal`.

### 5.2 Commands (proposed table; args and results are the contract)

| cmd | args | result |
|---|---|---|
| `calibration.check` | `{}` | `{"live": {DisplayInfo + frame/client rects} \| null, "stored": {scale?, monitor_dpi?, screen?, client_size?, measured_at?}, "differences": ["scale 1.5 → 1.25", …], "matches": true\|false\|null, "note": "game window not open"\|null}` — v2's four warning states derive from `live`/`stored`/`differences`; **not an error when the game is closed** |
| `calibration.setup.save` | `{"scale":1.5,"client_size":{"w":2865,"h":1789},"screen":{"w":3840,"h":2160}?}` | `{"rev":12,"block":{…calibration…},"differences":[…]}` — core derives `monitor_dpi`, `measured_at`, keeps `screen` from the payload or a fresh measurement; rejects scale ≤0 / non-positive size with `bad_request` |
| `calibration.tuner.check` | `{"frame_id":"f1","boxes":{"grade":{…},"attr":{…},"remaining":{…}},"spring_point":{…}?}` | `{"geometry":{region,grade_area,grade_y,attr_y,remaining_y,attr_x,remaining_x,row_height}, "measured_row_height":19, "grade":"DG", "remaining":25000, "attributes":[[…],[…],[…]], "matched":[{"name":…,"value":…}], "filter":{"enabled":true,"passed":false,"reason":…}, "warnings":[…]}` — pure (no write); the same code path `save` uses |
| `calibration.tuner.save` | `{…check args…,"display":{…}}` | `{"rev":13,"block":{…tuner…},"derived":{…geometry…},"artifact":{"file":"calib_tuner.png","path":…}}` — writes `tuner.ocr`, `tuner.spring_point` when given, and the `calibration:` env block; the artifact is the frame with the three bands drawn |
| `calibration.gem.save` | `{"frame_id":"f1"?,"display":{…}?,"points":{"N":{…}, …, "Combine":{…}},"resources":[{…},{…},{…}],"result_area":{…},"empty":true\|false\|null}` | `{"rev":14,"block":{…gem…},"derived":{"result_area":…,"empty_signature":{…}},"artifacts":[{"file":"calib_gem.png",…},{"file":"calib_gem_result.png",…}],"targets":{…}}` — with a frame: full save + empty question + artifacts; without a frame: coordinates-only save (v2 **Save Coordinates**), `empty`/artifacts omitted |
| `calibration.gem.moves.save` | `{"movements":{"radio_to_register":{…},"register_combine":[dx,dy],…}}` | `{"rev":15,"block":{…gem…}}` — writes `gem.movements` only; refuses empty/garbage counts instead of v2's silent 0 (`MainWindow:3119`) |
| `calibration.gem.check_result` | `{"frame_id":"f1"?,"area":{…}?}` | `{"composition":{mean_v,mean_sat,colored_fraction,mean_r,mean_g,mean_b},"distance":0.012,"verdict":"empty"\|"has_gem"\|"no_reference","spread":0.04,"dominant":"neutral","crop_png_base64":"…"?}` — v2's Check Result Colour and Sample result gem in one answer; live capture when no frame, respecting the foreground guard |
| `calibration.gem.test_route` | `{"move":"tuned"\|"arduino","from":"N","to":"Register","tuned":{"dx":-40,"dy":12}?}` | `{"steps":[{"action":"click","point":"N","placement":{…ok,x,y}},…],"line":"…"}` — clicks source, sends raw `D` (tuned) or closed-loop places (arduino), clicks target; logs one `arduino_debug.txt` line like v2 |
| `calibration.gem.cycle` | `{"confirm":true}` | `{"steps":[{"index":1,"action":"click","point":"N"},…],"done":true,"log":"…"}` — the 21-step list, every point pre-resolved before the first click, real clicks; refuses without `confirm` |
| `calibration.buysell.preview` | `{"draft":{bag_grid?,bag_slot?,shop_rows?,shop_region?,scroll_point?,max_button?}}` | `{"bag_centres":[…] (64), "row_centres":[…], "pitch":{x,y}, "row_pitch":…, "problem": "…" \| null}` — pure; powers "Show 64 centres" before Save |
| `calibration.buysell.save` | `{"draft":{bag_grid,bag_slot,shop_region,scroll_point,max_button},"display":{…}?}` | `{"rev":16,"block":{…buy_sell…},"derived":{"pitch":…,"row_pitch":…},"warnings":[…]}` — requires grid+slot and refuses a pitch disagreement with the domain's sentence; writes no screenshot |
| `calibration.shop.test_scroll` | `{"notches":3,"up":true}` | `{"line":"…"}` — click the focus point, send `Q n`/`Z n` through the firmware; refuses >400 with the guard line |
| `calibration.pet.preview` | `{"draft":{slots:[…],feeder_count_left_fraction,feeder_count_min_score,bag_grid?,bag_slot?}}` | `{"rows":[{"slot_boxes":[…],"count_regions":[…],"eta_region":…,"problems":[…]}, …],"bag_centres":[…],"problem":…}` — pure derivation for the overlay and the checklist |
| `calibration.pet.save` | `{"frame_id":"f1","rows":[{"toggle_label":…,"boarding_pet_slot":…,"feeder_strip":…,"stacks":2,"pet_slot_empty_png":true?}, …],"points":{menu,feed,close,page_tabs},"bag":{grid,slot},"feeder_count_left_fraction":0.4,"feeder_count_min_score":0.75,"display":{…}}` | `{"rev":17,"block":{…pet calibration half…},"artifacts":[{"file":"calib_pet.png",…},…],"derived":{…slot boxes/count regions…},"warnings":[…]}` — crops the marked pet slots from the frame into base64, writes `calib_pet.png`; **only the calibration half** (queue/food cells/flags preserved); up to 4 rows, `stacks` 1..5 |
| `calibration.pet.test_read` | `{"rows":[{…draft rows…}],"feeder_count_left_fraction":0.4,"feeder_count_min_score":0.75}` | `{"rows":[{"row":1,"slots":[{"index":1,"reads":[{"text":"138","score":0.99,"upscale":3}],"count":138}, …],…}],"hidden_ms":…}` — all ticked rows/slots, the v2 upscale ladder (3×/9×/24×), one hide for the whole read, debug crops under `logs/reads/` |
| `calibration.tooltip.hover_capture` | `{"point":{"x":..,"y":..},"hover_delay_ms":700}` | `{"frame":{…},"display":{…},"placed":{"x":..,"y":..},"line":"…"}` — place (closed loop), click to focus, wait, capture; shell hides/shows around it; used for the offset measurement and Test read |
| `calibration.tooltip.save` | `{"offset_x":-8,"offset_y":-6,"width":320,"height":210,"hover_delay_ms":700}` | `{"rev":18,"block":{…tooltip…}}` — validates size ≥20, offset fields present |
| `calibration.tooltip.test` | `{"point":{"x":..,"y":..},"hover_delay_ms":700,"debug":true?}` | `{"lines":[{"text":"…","score":0.93}],"region":{…},"debug_path":"logs/reads/tooltip_…png"?}` — hover then read at point+offset |
| `pointer.place` | `{"point":{"x":..,"y":..}}` | `{"placed":{"x":..,"y":..},"ok":true}` — closed-loop placement only |
| `pointer.click` | `{"point":{"x":..,"y":..}}` | `{"placed":{…},"clicked":true}` — v2 "Place cursor + click" / "Test Click (發條)" |

Notes:
- `calibration.save`/`fetch` (18 marks) remain unchanged for pixel marks and third-party/test use;
  `tuner_region` remains withdrawn because the three-box command now has a real home.
- **Out of the protocol on purpose:** v2's Debug Cursor (logical/physical) — v3 forbids
  `SetCursorPos` in-process and the core cannot call it ([CALIBRATION.md](../spec/CALIBRATION.md) §2
  rule "never call it while aware"; the process refuses it outright); Diagnose capture — answered by
  `window.find`/`window.measure`/`capture` and their refusals; Sample result gem — folded into
  `calibration.gem.check_result`'s extra fields. Each is a "dropped, with reason" line in the guides.
- New commands are added to PROTOCOL §4's table and a new **§14 "Calibration sessions and
  artifacts"** documents the session-by-draft rule, artifact writer, foreground/hide contract, the
  arbiter rule for pointer actions, and the per-surface save scoping. §8's stale "frame cache is
  specified, not built" line and [CALIBRATION.md](../spec/CALIBRATION.md) §2's stale "not wired yet"
  placement line are corrected in the same increment (both contradict shipped code).
- Shell side: typed protocol client methods + result models, and a **session draft** per surface that
  reuses the existing config-draft/keep-alive and `config.changed`/conflict machinery. The shared
  `CalibrationFramePicture` becomes the session canvas (multiple overlays: bands, points, strips,
  derived centres, count-start line); no rule moves into it.

---

## 6. Increments

Order (argued against the suggested one): **protocol foundation first; Setup Detect second (it is
cheap and it is the acceptance harness); Tuner third (proves the session shape); Gem fourth (the
largest win after tuner and the reference for pointer tests/cycle); Buy/Sell fifth (canvas + overlay +
sell grid, no live-read risk); Tooltip sixth (small, proves hover-capture); Pet last (largest, and
depends on the reader's upscale ladder and on the Pet-tab setup work already queued as task-70).**

| # | Surface | Protocol | Acceptance (unit / protocol / shell / live) |
|---|---|---|---|
| 0 | Protocol + artifact/pointer foundation, no player-visible surface | §14 text; `pointer.place`/`pointer.click`; artifact overlay writer (rect/dot drawing over a frame PNG); fixture set; shell typed client + session-draft skeleton | core: fake board/cursor/frames replay (`TextBlock`-free protocol tests), artifact bytes decode + correct geometry; `cargo test --workspace`; fixtures from the real types; shell: fake-transport round-trip of the new drafted commands; doc corrections (PROTOCOL §8, CALIBRATION §2) |
| 1 | Setup → Detect | `calibration.check`, `calibration.setup.save` | domain: `CalibrationCheck` already tested (`seal-domain/tests/calibration.rs`; v2 `CalibrationCheckTests.cs`); core: four warning states from stored/live/missing/closing, save derives `monitor_dpi`/`measured_at`, revision + `config.changed`; shell: Detect fills fields, warning on build/re-entry/activation, Save round-trip; **live: player presses Detect on their TW-live PC and sees "matches the stored calibration" with their v2 config, then changes scale by 1.5→1.25 in the field and sees the differences line** |
| 2 | Calibrate Tuner | `calibration.tuner.check`, `calibration.tuner.save` | domain: geometry formulas pinned against v2 `BuildOcrGeometry` examples + `row_height` algorithm (collapse<3px, median); core: three-box derive, OCR-on-frame, matched/filter, save ordering, artifact pixels; **v2-config replay: `tuner.ocr`/`spring_point` from the v2-era fixture render the same bands and Check OCR reads the same grade/attributes on the archived/captured frame**; live: player's Check OCR first (no capture) reads their saved geometry, then drags the same boxes and saves without changing the numbers |
| 3 | Calibrate Gem | `calibration.gem.save`, `calibration.gem.moves.save`, `calibration.gem.check_result`, `calibration.gem.test_route`, `calibration.gem.cycle` | domain: empty verdict + routes + cycle step list (21) against v2 tests (`GemColorAnalyzerTests`, `GradeCyclesTests`); core: coords-only vs full save, empty question from the calibration frame, crop artifact bytes, signature; route tests against fake board; shell: points/drag order, coords grid, from/to tests, move grids + dimming, result preview, full-cycle confirm; **v2-config replay: same points/area/movements; Check Result Colour gives the same verdict on the same frame; Test route sends the same `D`/placements**; live: one tuned route and one arduino route land, full cycle completes and the player confirms |
| 4 | Buy/Sell calibration + sell grid | `calibration.buysell.preview`, `calibration.buysell.save`, `calibration.shop.test_scroll` | domain: pitch/tolerance + row derivation against `BagGridTests`/`ShopGeometryTests`; core: preview without a frame, refusal on disagreement, save + env; test scroll board traffic; shell: two-drag canvas with immediate overlay, 64 centres + rows, focus/MAX clicks, checklist, sell grid (memory-only) + cap display + dry run if kept; **v2-config replay: 64 centres and 10 row centres coincide with v2's for the fixture**; live: player shows 64 centres on the live bag/shop and scrolls 3 notches |
| 5 | Calibrate Tooltip | `calibration.tooltip.hover_capture`, `calibration.tooltip.save`, `calibration.tooltip.test` | domain: offset formula; core: place→click→wait→capture under the input gate + foreground guard + hide contract, test read at point+offset, debug crop; shell: two-capture flow (capture → mark → hover-capture → drag), delay field, result pane; **v2-config replay: stored offset/size load and Test read reads at the same region**; live: hover a pet and a food item, both panels read |
| 6 | Calibrate Pet | `calibration.pet.preview`, `calibration.pet.save`, `calibration.pet.test_read`; reader upscale ladder (OPEN §2) | domain: `FeederLayout` slot/read/ETA regions against `FeederLayoutTests`/`FeederEtaTests`, count parse against `FeederCountTests`; seal-ocr: per-scan upscale factor (3×/9×/24× ladder) with the run and the test sharing one code path; core: two-frame crops, calibration-half write scope, base64 crops round-trip, `calib_pet.png`, test read batch with one hide; shell: two-capture session, row add/remove + strip drawing + numeric geometry, count fraction/min score, empty-slot capture + preview, checklist; **v2-config replay: same slot boxes/count regions/ETA region from the fixture, and Test read produces the same numbers on the player's recorded screen (see ⚑1)**; live: full Test read on all four rows with food in the slots, then one empty-slot recapture |
| 7 | Parity pass + documentation (no new feature) | — | the player runs one recorded v2-vs-v3 worksheet (⚑1) across all six surfaces with the v2 config; every derivation and verdict matches or is explained; then one **from-scratch** recalibration pass on the same machine must save values within the tolerances v2 used; docs updated in the same commits: `spec/PROTOCOL.md` (§4/§8/§14), `spec/CALIBRATION.md` (§1 marks table, §5 artifacts, §6 Detect, §7 open, §8), `spec/CONFIG.md` (crop fields/artifacts), `USER_GUIDE.md` + `.zh-TW` + `INSTALL.md` (calibrate tabs, Detect, dropped diagnostics), `OPEN.md` prune, `DESIGN.md` §8, and the guide's stale v2 tab names replaced with v3 wording |

Every increment is TDD and one commit per success; the plan file itself is not updated as work lands
(the board row moves, then the plan is archived per the repo rule).

---

## 7. ⚑ Questions (player/Lead)

**⚑1 — Record the v2 oracle before v2 is retired (blocking; player action).** The strongest reading of
"same behaviour" needs v2's *outputs*, not v3's own re-derivation (v3's ported formulas could agree
with themselves and still differ from v2). Before this work starts, the player should run a short
recorded session in v2 on the TW-live PC with their config: Setup Detect lines; Tuner Check OCR (saved
geometry, no drag); Gem Check Result Colour + Sample result gem with a known box; each grid's Show-64-
centres row count; Pet Test read on one known screen; Tooltip Test read lines; three tuner boxes + the
resulting saved `row_height`; Pet one row strip → the derived slot numbers from the Test read pane.
Store it as a local worksheet (not committed if it contains player state), plus the **process** in the
plan's successor record. Without it, increment 7 degrades to "v3 against its own derivation".
*Recommendation: do it in the v2 maintenance window, before increment 0.*

**⚑2 — Stateless draft protocol vs a core session object.** Proposed: stateless (§5.1.1) — the shell
holds the draft, commands are pure/atomic, the core keeps only frames. Alternative: `calibration.session.*`
(open/mark/commit) in the core. Decided in favour of stateless in this plan; confirm with the Lead
before increment 0 freezes the wire.

**⚑3 — Who draws the reference screenshots.** v2's `calib_tuner.png`/`calib_gem.png` are annotated at
save time by the WPF shell. "The core never draws" is a UI rule, not a file rule; the plan therefore
has the **core draw rectangles/dots over the frame** with the existing `image`/PNG seam (single writer,
byte-stable artifacts). Alternative: core writes a plain screenshot + a JSON overlay and the shell
draws previews only (no annotated artifact — a parity loss). Confirm the reading; if the Lead insists
on "no drawing in core", fall back to plain screenshots + a marks sidecar and record the deviation.

**⚑4 — Pointer/test actions vs the arbiter.** Must a calibration action wait for a running tool,
refuse with `tool_busy`, or be able to preempt the resident feeder? Proposal: acquire the same input
lease as a tool (v2 `AcquireInputAsync` — waits when the resident feeder is mid-reload; refuses while
another foreground tool holds the game) and hold it for the whole action; a tool start during a
calibration action gets `tool_busy`. Confirm the exact busy/queue rule with the Lead; it is observable
behaviour and cannot be invented at the keyboard.

**⚑5 — Pet Test read parity needs the reader's upscale ladder.** v2 reads counts at 3×/9×/24× and the
Test read walks the same rungs (`PetTool.CountReadUpscales`); v3's `scan_rgba` has no upscale
parameter yet ([OPEN.md](../backlog/OPEN.md) §2). Bundling the ladder in increment 6 is recommended
because the live run needs it too; the alternative is a Test read that cannot explain a slot the run
would read — a parity hole in the one surface named "knife-edge".

**⚑6 — Which v2 quirks are behaviour and which are bugs.** Proposed parity list: tuner Save without a
spring point *clears* the stored spring point (v2 `TunerSave` rebuilds the block) — keep or fix?; a
malformed tuned count falls back to 0 in v2 (`ParseMove:3119`) — refuse instead (recommended); pet
marks are only persisted on Save even though the tab comment says "written as they are placed" — save
on Save (v2 really writes on Save); gem mode is saved with Save Gem Composer but v3 already edits it on
the Gem tab — keep the v3 home. Player/Lead call on the tuner-spring quirk in particular: it is
observable ("I saved and my spring point vanished").

**⚑7 — The stale tab names in user-facing messages.** Ported v2 refusals say "Calibrate Gem / Calibrate
Tuner / Calibrate Pet / Calibrate Buy/Sell" and "tick it on the Pet tab"; v3 has **one** Calibrate tab
and a read-only Pet tab. These messages are inside this plan's increments anyway; replace them with
the v3 wording ("Calibrate → Gem group") as each surface lands, and update both guides in the same
commit. (This also clears the audit's worst shipped-guide contradiction.)

**⚑8 — Setup warning placement and trigger.** v2 re-checks stored-vs-live on tab build, tab re-entry
and window activation; no global banner. Proposed: v2 parity for the Setup tab, **plus** an optional
one-line warning in the Calibrate tab header (the place a player actually starts calibrating) — the
player decides whether the extra line is wanted.

**⚑9 — Sell-slot selection persistence.** v2 keeps the selection in memory only (never saved) and
resets on restart. Proposed v3 parity (increment 4). If the player would rather have it persist, that
is a deliberate deviation and a `buy_sell.sell_slots` save — say so now.

**⚑10 — The Pet tab's queue/food-cell capture is NOT in this plan.** v2 captures queue pet icons and
food-icon crops from the Pet tab, not the calibrate surfaces; task-70 returns the row/queue/food-cell
**editors** without the capture session. This plan owns the calibrate-tab crops (empty-slot reference,
`calib_pet.png`) and leaves queue **add** (capture) as a named follow-on, reusing
`calibration.tooltip.hover_capture`-style primitives if wanted. Confirm the boundary.

**⚑11 — Protocol version.** Additive commands under `v:1` are proposed (both halves ship together,
fixtures grow, nothing existing changes). If the Lead wants a bump, the `hello` mismatch path already
exists; either is implementable, but the decision must land before increment 0 writes fixtures.

**⚑12 — v2 config with a stale environment should still load.** The acceptance says "v2 config in, no
recalibration"; Detect may immediately warn that scale/client/screen changed (e.g. the PC was moved to
150 % after calibrating). Proposal: **load and derive anyway** (v3 never refuses on a stale
`calibration:` block; the warning is advice, exactly v2's behaviour), and acceptance uses the player's
current machine where v2 already runs correctly. Confirm this is the intended reading for a machine
whose v2 config was measured elsewhere.

---

## 8. Out of scope, and v2 diagnostics that are dropped

- **Tool behaviour** (all tools already live) and the **Pet-tab queue/food-cell editors** (task-70;
  ⚑10). **Sell/Buy dry runs** are v2 Buy/Sell *tab* features, not calibration; they are not in this
  plan unless the Lead folds them into increment 4 (they are cheap: place/scroll without clicking and
  reuse `pointer.*`).
- **Dropped, with a reason:** Debug Cursor (logical/physical) — the API is refused in this process and
  v3 forbids `SetCursorPos` ([CALIBRATION.md](../spec/CALIBRATION.md) §2); Diagnose capture — answered
  by `window.find`/`window.measure`/`capture` and their refusal lines (the guide says so instead);
  Sample result gem — folded into `calibration.gem.check_result` detail; the "New Gem Composer Moves"
  duplication — one route grid with a mode selector, since v3 already has the mode on the Gem tab.
- **Not changed:** the Arduino protocol (level 2), firmware, the coordinate contract, the two-process
  split, the machine blocks' names.

---

## 9. Documents that ride each increment

`spec/PROTOCOL.md` (new §14 + §4 rows + stale §8), `spec/CALIBRATION.md` (mark table additions,
artifacts, Detect §6 no longer "surface not built", §7 open items closed, §2 stale placement line),
`spec/CONFIG.md` (crop/artifact fields and where each save writes), `spec/DESIGN.md` §8,
`USER_GUIDE.md` + `USER_GUIDE.zh-TW.md` (the four calibrate surfaces, Setup Detect, dropped
diagnostics, the corrected tab names), `INSTALL.md` (first-run and migration mention recalibration is
now UI-complete), `backlog/OPEN.md` (the session-shaped features and the caliper gaps leave), and the
`plans/README.md` board row — which is where this plan's state lives.

---

## 10. Lead decisions (2026-10-28)

The Lead's answers to §7, so increment 0 can freeze the wire. The player still owes ^1 and may overrule
^6/^9; everything else is decided.

- **^1 (v2 oracle) — accepted, scheduled, not blocking increments 0-2.** It needs the player, so it is
  taken during their next v2/v3 session (the second live pass), before increment 7; increments 0-6
  proceed against the ported formulas and the committed v2-era replay fixture. The worksheet stays
  local (player state), the process lands in the successor record.
- **^2 stateless drafts — DECIDED: stateless.** The shell holds the draft; commands are pure/atomic;
  the core keeps only frames (the existing `frame_id` cache). No `calibration.session.*` object.
- **^3 annotated artifacts — DECIDED: the core draws them**, single writer, byte-stable. Check the PNG
  seam first: if the core cannot draw rects/dots without a new dependency, the fallback is a plain
  screenshot plus a JSON marks sidecar, and that parity loss is recorded in the increment's commit.
- **^4 input lease — DECIDED: same lease as a tool.** Calibration actions wait for a resident feeder
  mid-reload, refuse `tool_busy` while a foreground tool holds the game, and hold the lease for the
  whole action; a tool start during a calibration action gets `tool_busy`. The arbiter gets one entry
  point, not a second policy.
- **^5 pet upscale ladder — DECIDED: in increment 6**, shared by the run and Test read.
- **^6 v2 quirks — DEFERRED to the player for the two observable ones.** Proposed parity for the rest:
  refuse a malformed tuned count rather than fall back to 0; pet marks save on Save; gem mode stays on
  the Gem tab (v3's home). The tuner-Save-clears-spring-point quirk is the player's call; until then
  parity (v2 behaviour) is the default.
- **^7 stale messages — DECIDED: replaced as each surface lands**, and the guides updated in the same
  commit; this plan owns closing the audit's worst contradiction.
- **^8 warning placement — DECIDED: v2 parity on the Setup tab plus the one-line warning in the
  Calibrate tab header.**
- **^9 sell-slot persistence — DEVIATION, already in flight.** The Sell grid (task-69) saves
  `buy_sell.sell_slots` through the normal config path, which v2 deliberately did not persist. It is a
  player-visible improvement requested live ("the grid is not completed"); the v2-parity alternative
  (memory-only) is noted here so the deviation is not silent. If the player prefers v2 parity, task-69
  is re-cut before release.
- **^10 boundary — CONFIRMED.** This plan owns the calibrate-tab crops (empty-slot reference,
  `calib_pet.png`); the Pet-tab queue/food-cell **capture** session is a named follow-on, not this
  plan's increments 0-7.
- **^11 protocol version — DECIDED: additive commands under `v:1`**, no bump; both halves ship
  together and the fixture set grows. PROTOCOL §14 is the section that gains them.
- **^12 stale environment — DECIDED: load and derive anyway.** A `calibration:` block that does not
  match the live machine is advice, never a refusal; Detect warns, tools run with the stored geometry
  (exactly v2).

