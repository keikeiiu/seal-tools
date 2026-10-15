# The boarding window's close — retry, and an already-open probe

Design record, 2026-10-11, branch `feature/pet-flow`. Step 1 of
[PLAN-PET-FLOW.md](archive/PLAN-PET-FLOW.md) §8 ("close retry + already-open guard"), which asked ⚑6: *retry
the X, or probe whether the window is already open before the next open?* — and answered itself *"the
probe is strictly more useful: it also covers a window left open by a stopped run."*

**Built:** retry (commit `7daf94a`) and probe (this record's commit). Three proposals were designed
independently and judged by two lenses (failure-direction safety; consistency with this codebase);
the probe as built follows the judges' winner with one deliberate change of provenance, below.

---

## The problem, measured

`pet.log` holds **17 closes where the cursor could not reach the X** — `close: couldn't reach the X —
the cursor wouldn't move to (844,327) — it stopped at (861,327)`, most recently 2026-10-09. Two
corrections from the judgement that this record adopts:

- Those coordinates are **HidPointer's logical-space goals**, not a moving calibration. The configured
  X has been the client point `(866,177)` for all 450 logged closes; `cursor_trace.txt` shows the
  placement giving up 18 px short after 16 damped steps. That is what the retry targets.
- The 17 events split **7 same-run** (a close failed and the next open follows in the same run) and
  **9–10 across a run boundary** (the process stopped between open and close), which is why the belief
  below is persisted rather than kept in memory.

A failed close leaves the window **open**, and the next open clicks 目錄 and the feed icon *blind* —
both are coordinates in the closed world, so with the window up the first click lands on whatever the
window shows there. The tool's own comment names the hazard: *"leaving the boarding window on top of
the game for the next cycle to click 目錄 behind."*

## The options weighed

| # | Mechanism | Verdict |
|---|---|---|
| A | A crop of the X (learned), compared against a live grab of the same rectangle; **gated by a persisted "the window may still be open" belief** so the ordinary open pays nothing | **chosen** — the probe is a second production use of the stored-crop-vs-live-region shape the empty-slot check already runs daily; unknown degrades to today's blind click |
| B | Memory-only "last close failed" flag + an extra X click before the next open | rejected — cannot cover a stopped run (9–10 of 17), and the blind X click lands on the game world when the belief is stale |
| C | OCR the 結束代養 toggle label | rejected as the primary — its live readability is unproven (the game's Chinese is Traditional and the measured OCR confuses 餐/養), and it pays an OCR pass on every open; kept as the hardening path below |

The judges' killer objection, kept honestly: **no single signal is uniquely present in the true state
and absent in every wrong one.** Two blue round X buttons exist on one screen (the breeder's and the
bag's, ~440 px apart), so a mis-marked point could store bag pixels; and any belief can outlive a
window the player closed. Every design therefore *bounds* the false-open direction rather than
eliminating it. Two things do the bounding here: the belief gate (the probe is consulted only after a
close that could not be sent, or a run that died between open and close) and a score logged on every
probe, so one live occurrence is visible in `pet.log` rather than invisible.

## What was built

**The belief** — `pet.boarding_window_may_be_open` (local.yaml, session half; the run writes it, the
player never does). Set when the window opens, cleared when a close click is **sent** (a sent click is
taken to have worked — the assumption every click in this tool makes), and **left standing** when no
click could be sent. Persisted for exactly the killed-run case: the window outlives the process, so
only the next run can notice it.

**The reference** — `pet.close_button_png`, a 28×28 crop = the X's calibrated point ±14 px, base64 in
local.yaml. MEASURED for the design against the player's own capture: the glyph is ~26 px wide, a
28×28 crop at the matcher's 6-px inset leaves a 16×16 compared window wholly on the button (32×32
starts catching the animated world behind the corner), identical content scores 0.000, the glyph alone
occupies 0.36 of the window, and other screen patches score 0.98–1.00. Threshold **0.10** — three and
a half times below the glyph's own ink, and a probe that is too strict only falls back to today's
blind open.

**Deliberate deviation from the winning proposal: the crop is learned at RUNTIME, not at calibration
time.** The proposal cropped it when the player marks the X on Calibrate Pet; the judges made a
mark-time validation mandatory (the tab cannot verify which of the two captures a mark sits on).
Learning it instead from a window whose contents have just been READ removes the question. A crop
taken on faith could be the game world, and a world crop *matches the world*, which would turn a
closed window into a skipped open — the one direction this guard must never fail in.

**The proof is deliberately narrow, and it is taken at the moment it happens** (tightened after the
2026-10-11 adversarial review, which showed the first version could be satisfied by a reading of the
game world): only three structures of the boarding window's own interior count — a slot that
**matched its empty reference**; **digits** read off a feeder slot by the OCR (not the colour verdict
"this cell has no food", which is a fact about a crop); and the game's own **completion-form** time
line. The capture runs inside the read, the instant one of those lands, not after the remaining rows'
OCR passes — a player can close the window at any moment, and this tool runs for days beside them. An
existing reference that stops matching is replaced, from the same proven state, so a re-rendered X
cannot kill the guard silently; every learn and every replacement is logged.

**The probe** — `BoardingWindowLooksOpen`: park the cursor (the failed close leaves it beside the X,
and a reference never contains the arrow — the same reason the bag scans park first), grab the same
client rectangle, `IconMatch.DifferingFraction` against the reference, log the score always. Every
unreadable path — no reference yet, no point, window gone, **game not in front**, grab failed —
returns null, and null means the old blind open. A positive match skips **only** the two clicks; the
shared tail is unchanged, including the unconditional Enter that clears the out-of-food message.

## The probe's second job: confirming an open actually happened

> **SUPERSEDED 2026-10-11 (PROGRESS 88, review 2).** The paragraph below says a second unconfirmed
> open is "logged loudly and carried on from". The live 13:56/15:09 incident showed what "carried on"
> means when the window really is closed — the reads describe the game world and the reload presses
> its buttons at it — so the shipped behaviour is now a **deferred visit** (no reload attempted, no
> strike), with the X's reference thrown away after two consecutive unconfirmed opens. The text below
> is the original record and is left as it was.

Added the same day, from the player's report — *"in some cases, the first click may fail and cause
the breeding window not open."* After the two opening clicks, `OpenBoarding` asks the probe whether
the X is where its reference says. A **positive mismatch re-opens once**; unknown changes nothing; and
a second failure is **logged loudly and carried on from**, never a hard failure — a stale reference
would otherwise fail every open forever (no read would ever succeed, so nothing would ever re-learn
it), and a run that silently stops feeding is the one outcome the tool refuses. The backstop for the
window really not being up is the slot read: a difference from an empty slot's reference in the
"not-the-window" band (≥ 0.85 — measured: pets read 0.65-0.785, everything else 0.91 up) now answers
**unknown**, so the world below a window that never opened can no longer read as "a pet is in the
loader" and press the end toggle at it.

## Accepted residual, and the hardening path

A mis-marked X (the bag's X, or the game world) can, in the belief-gated states, match the live region
and skip an open whose window is actually closed. Bounded by the gate and made visible by the logged
score; not eliminated. If a live occurrence is ever seen, the structural cure is a **second
independent live signal** at probe time — the cheapest candidate is the OCR of the toggle label the
rows already calibrate (`row.ToggleLabel`), which needs an offline readability check against a saved
boarding-window capture before it can be trusted.

## Verification

Pet and Tests projects build clean; the 265-test suite passes, with the two new keys carried by the
existing reflection fences and a save→load round trip (`PetCalibrationRoundTripsAndKeepsItsOwnBagGrid`)
because the load merge has no fence over it. **No live run has exercised the probe yet** — the first
watched run should show either `learning the boarding X's reference from this open window` once, or a
`boarding-open probe:` line after any failed close.
