# State Control Fix Plan

**Status:** PENDING EXECUTION (recorded 2026-08-24, awaiting user go-ahead after gem composer recheck)

## Problem

Tool "running / loaded / grade" status in the launcher UI is wrong after Load, and the Gem Composer never shows a Start button.

## Root Cause

State lives in two places that drift apart:

1. **Launcher** knows only if the *process* is alive (`_current_proc.poll()`).
2. **State file** (`state.json` / `gem_state.json` / `spammer_state.json`) is written by the *tool*, but **only on transitions** (start/stop/loop/quit) — **never on load**.

So right after clicking **Load**, a fresh process starts but the previous run's state file (still `running: true` + old grade/attempt) is on disk. The launcher reads it and shows "ON" → hides the Start button.

## The Rule

> The tool is the single source of truth for its own state, and it writes that state the instant it loads.

## Plan (3 parts)

### Part 1 — Tools write initial `running: false` on load
- `gem_composer.py` → `write_state(False)` after `cycle = 0` — **DONE**
- `seal_tuner.py` → write `{"running": false, "attempt": 0}` after `attempt = 0`
- `skill_spammer.py` → call `write_state()` after `_shared` is configured (before main loop)

### Part 2 — Launcher deletes the state file at launch (closes the 2–3s race window)
In `start_tool()`, before `Popen`, delete the tool's `state_file` (if present). Eliminates the brief "ON" flash between spawn and the tool's first write.

### Part 3 — Launcher only surfaces transient fields when the process is alive
Refactor `api_tools()`:
- **loaded** = process alive (authoritative)
- **running** = state file's `running`, trusted **only when loaded**
- **not loaded** → clean `OFF`: no stale `grade`/`attempt`/`count`/`current`; show only config previews (gem `start_grade`).

## Refinements (from self-validation)

1. **Tuner Part 1** writes a *clean* state `{"running": false, "attempt": 0}` (matching end-of-run), NOT "preserve attrs" — attrs aren't persisted across runs anyway.
2. **Part 2** *deletes* the state file rather than "set running=false preserving fields" — cleaner, drops misleading `attempt` history (resets to 0 each run anyway).

## Out of scope (noted, not fixing)

- State-file writes are non-atomic (transient `{}` on one poll if read mid-write); already guarded by `except: pass`, self-corrects.
- Frontend `fmt()` labels the spammer press `count` as "Keys" (cosmetic).

## Net effect

Load → `IDLE` + Start button. Start → `ON` + live grade/cycle/count. Quit → clean `OFF`.
