# Calibration screens: multiple captures with per-target assignment

**Status:** design proposal, 2026-11-05. No code yet. It replaces the one-shared-capture model in
the Calibrate workspace. The motivation is a game constraint: the Buy/Sell window, the Pet panels
and the Gem Composer cannot be open at the same time, so one screenshot cannot serve every target.

## Step 0 — existing system (detection)

- **Framework:** Flutter desktop, existing `MaterialApp` theme.
- **Tokens:** `ShellSpace` (2/4/8/12/16/24), `ShellRadius` (6/10/14/999), `ShellWidth`
  (prose 620, aside 300, picker 240), `ShellSize.iconSm/Md/Lg`, `ShellText.mono`,
  `ShellColors` success/warning/info. No raw values in the new UI; use these.
- **Components:** `SurfacePanel`, `PageHeader`, `StatusChip`, `StatusBanner`, existing capture shelf
  dialog and mark editor. No new component library.
- **Dark/light:** both supported; colour is never the only carrier (chips include a word/icon).

## Problem

The Calibrate workspace has one page-level Capture action and one selected frame. Each tool remembers
one frame, but a tool with no frame adopts the newest shelf frame, and the canvas falls back to the
newest shelf frame. That produces the wrong calibration picture — e.g. a Pet target shown on a
Buy/Sell frame — and there is no way to pre-capture several game screens and assign them to the
features they belong to.

## Design goal

Let the player capture every needed game screen **first**, then walk the target tree and assign the
right screen to each target, then save. A screen may serve several targets; a target has at most one
screen at a time.

## Hierarchy (what wins the eye)

| Rank | Content | Action |
|---|---|---|
| Primary | The picture and the mark being drawn | **Save** is the only filled action while editing |
| Secondary | The selected target and its assigned screen | **Capture screen** (filled only when no screen exists) |
| Tertiary | The screen shelf, Refresh stored, Details, Stored marks | quiet outlined/text actions |

The target tree and editor keep their current positions. The capture shelf becomes a compact strip,
not a modal, because assignment is a first-class part of the workflow.

## Layout

```text
Calibrate                                              [Refresh stored]

Capture every screen the tools read, then assign one screen to each target.

┌ Capture bar ─────────────────────────────────────────────────────────────┐
│ [📷 Capture screen]   Screens 3/12   [Details] [Stored marks]            │
└──────────────────────────────────────────────────────────────────────────┘
┌ Screens ─────────────────────────────────────────────────────────────────┐
│ [ + Capture ]  [ Shop · 1  ✓2 ]  [ Shop · 2  ✓1 ]  [ Pet · 1 ]  → scroll │
└──────────────────────────────────────────────────────────────────────────┘
┌ Targets (250) ─────┬ Canvas ──────────────────────────┬ Editor (340) ─────┐
│ Shop 5/5           │                                   │ Target: bag_grid  │
│  ● bag_grid        │        selected screen            │ Screen: [Shop·1 ▾]│
│  ○ bag_slot        │        with marks and fit/1:1     │ [✓ assigned]      │
│ Gem 3/9            │                                   │ x/y/w/h fields    │
│ Pet 9/20           │                                   │ [Revert] [Save]   │
└────────────────────┴───────────────────────────────────┴───────────────────┘
```

- The **Screens** strip sits directly under the capture bar and is always visible while a mark tool
  is active. It scrolls horizontally when there are more screens than width allows.
- The first tile is `+ Capture` (same action as the capture bar); after the first capture it becomes
  a normal thumbnail.
- A thumbnail shows the game screenshot, a label (`Shop · 1`), and an assignment badge (`✓2`).
  Selected = primary outline + check icon (not colour alone). Assigned-to-current-target gets a
  small filled footer.
- The strip is hidden in Tuner/Tooltip session tabs; those sessions already own their capture.

## Interactions

### Capture

1. Press **Capture screen** (capture bar or `+ Capture` tile).
2. The shell hides, captures, and shows again exactly as today.
3. The new screen is added to the strip.
4. If the selected target has **no** assigned screen, the new screen is assigned to it. If the
   target already has one, the assignment is left alone; the screen is merely added to the shelf.
   This supports both “capture as I go” and “capture everything first”.

### Assign

- Selecting a target restores its assigned screen in the canvas and editor.
- The editor's **Screen** row is a dropdown of live screens, each with a thumbnail and `Shop · 1`
  label. Choosing one assigns it to the current target. Targets may share one screen.
- Clicking a screen thumbnail in the strip **selects it for preview**; the editor then shows
  **Use “Shop · 1” for this target**, which performs the assignment. This avoids a stray click
  silently reassigning a target while still letting the picture be inspected first.
- The assignment is session state, like the frame cache. Marks do not persist a frame id; after an
  app restart the player re-captures. The editor says “screens are session-only” in its help line.

### Save

- Save is enabled only when the target has an assigned live frame and the mark is valid.
- The save payload uses the assigned `frame_id`.
- If an assigned screen was evicted (core cache), the target shows a warning chip
  **“screen evicted — recapture or choose another”**, and Save is disabled.
- After saving, the assignment stays, so the next target on the same screen is one click away.

### Empty and error states

| State | Surface |
|---|---|
| No screens | Strip shows a single dashed `Capture the game window` tile; target tree still selectable; editor explains “capture a screen first” |
| Screen exists, target unassigned | Editor: `Screen: [Choose a screen]`, Save disabled |
| Assigned, live | Editor: `Screen: Shop · 1 [✓]`; Save enabled once the mark is valid |
| Assigned, evicted | Warning chip; Save disabled; recapture or choose another screen |
| Capture refused | The core line stays in the capture bar verbatim (existing behaviour) |

## Data model

Shell-only session state (no protocol change beyond cache capacity):

```text
CalibrationCapture { frame, display, capturedAt, sequence, tool, label, bytes }
_targetFrameIds: Map<String, String>   // key = kind@target, or kind for untargeted
_selectedScreenId: String?             // current preview/assignment
```

- `_frameIdForTool` and `_activeCapture` lose their newest-frame fallback. A target with no
  assignment shows no picture.
- Target key uses the same identity as the save (`kind` + optional 1-based target) so assignments
  cannot collide across named instances.
- Capture labels are generated (`Shop · 1`, `Shop · 2`, `Pet · 1`); renaming is out of scope for the
  first pass. Thumbnails carry the visual difference when two screens share a tool.

## Core requirement

`FrameCache::CAPACITY` must hold the whole session's screens. Proposed **12** live frames:
Buy/Sell (2) + Gem (1) + Pet (3) + retries/margin. This updates `PROTOCOL.md` §8, the cache test, the
shell count/eviction copy, and the memory note. The cache keeps PNG bytes and lazily decodes RGBA;
12 full-client frames is a bounded desktop cost.

## Tokens and components used

- Panel: `SurfacePanel` with `ShellRadius.m`, hairline `outlineVariant`.
- Selected thumbnail: 1.5 px `primary` border, `StatusChip`/check icon; assigned badge uses
  `ShellColors.success` with a word, not colour alone.
- Strip spacing: `ShellSpace.sm` inside, `ShellSpace.md` between tiles; no raw pixels.
- Labels: `labelSmall` (11/600); status/help: `bodySmall`; numbers/coordinates: `ShellText.mono`.
- Thumbnail aspect: game client aspect, height 56 px, width derived (approx 96 px); list scrolls.

## Responsive / dense cases

- At the 1120 px minimum the strip still scrolls; target tree, canvas and editor keep their current
  breakpoint behaviour.
- More than 6 screens: horizontal scroll with a visible scrollbar/fade; count tells the player how
  many are live.
- The strip is one row (~88 px) so the canvas does not collapse.

## Increments

1. **Core:** raise frame capacity to 12, update protocol/spec/tests and the shell copy.
2. **Shell model:** per-target assignment map; remove cross-tool/newest fallbacks; Save gating.
3. **Shell UI:** screens strip, selected/assigned/evicted states, editor Screen row, capture
   auto-assign rule.
4. **Tests:** target-to-frame isolation, shared screens, eviction, save payload, empty state,
   capture button per tool; full shell gate.

## Open questions

1. Capacity **12** — enough for your real screen list, or should it be 16?
2. Is a **persistent strip** preferred over keeping the existing **Captures dialog** and adding an
   assignment dropdown only in the editor?
3. Auto-assign a new capture to the selected target when it has none — yes?
4. Do you want auto labels only, or rename/inline label editing in the first pass?
5. Assignment stays session-only (no frame ids in saved marks) — accept?
