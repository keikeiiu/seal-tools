# Seal Tools v2 — the open work we are executing

**What we intend to do, and roughly in what order.** Only work somebody has decided to do belongs here.
A defect nobody has scheduled, an idea nobody has designed, and a decision *not* to change something all
live elsewhere — the pointers are at the bottom.

What was done and when is [PROGRESS.md](../records/PROGRESS.md); how it works is
[DESIGN.md](../spec/DESIGN.md); the guardrails are [REVIEW.md](../spec/REVIEW.md); a live plan's build
state is the board in [plans/README.md](../plans/README.md); finished plans are in
[archive/](../archive/README.md).

---

## 1. .NET 10 LTS — .NET 8 reaches end of support on 2026-11-10

Every project targets `net8.0` / `net8.0-windows` and there is no `global.json`. After that date the
runtime gets no more servicing fixes, and this repo's whole distribution model is a self-contained exe
(`publish.bat`), so a runtime fix would mean republishing regardless. **The move is to .NET 10 LTS**,
tested, and it is an **L** — nine projects, plus WPF, plus OpenCvSharp / ONNX Runtime / RapidOCRSharp
package versions that must line up with the new TFM. Raised by the 2026-10-06 external audit.

*Verify:* the whole suite green, and a published exe launched on a machine with no .NET installed.
*Start with:* a plan — nine projects' worth of TFM, and the package versions are the risk.

## 2. A full tuner run and a composer cycle in the current build

The residency work rewrote the shared Start/Stop path and only the pet tool has exercised it — see
[HANDOVER.md](../records/HANDOVER.md). No code change is wanted first: this is the check that says
whether one is needed.

*Verify:* one tuner run and one composer cycle, watched, on the live game.

## 3. The UI pass, continued

The WPF-UI rebuild shipped (the plan's steps 0–9) and the `NavigationView` rail was **tried and
reverted** — the tab strip read better, and clicking a rail item did not switch the page. What is left is
the button label/weight convention, plus the measured start in
[ANALYSIS-UI.md](../investigations/ANALYSIS-UI.md): extract the calibration canvas (copy-pasted 5×),
collapse the four button factories into one, and the two cross-cutting items the earlier plan never
covered — accessibility, and the status poll's dependency on a `ToolState` thread-safety claim that does
not hold.

## 4. The tool lifecycle lock has no test, because there is no seam to test it through

The lock itself landed 2026-10-06 (PROGRESS 59) and every access site was checked by grep to be inside
it — but a meaningful concurrency test needs a tool that can be **started and stopped without a board**,
and `LauncherService._toolRunners` closes over `Config`, `Attributes` and the port, so no fake can be
injected. **The fix is the seam, not a test written against the current shape**: inject the runner map
(or extract the registry into Core behind an interface) and then hammer add/remove/read from two threads.
Until then the lock is reasoning, not verification.

---

## Waiting on you — a decision, not work

- **`ReferenceWindowConfig`** is written to `defaults.yaml` but never read (auto-anchor is *not*
  implemented). Remove it, or leave it as a placeholder for the planned feature.
- **Debug Cursor / Debug Physical buttons** in the Gem calibrate tab — keep as diagnostics, or remove.

---

## Not in this file

| what | where |
|---|---|
| defects found, and not yet scheduled | [KNOWN-ISSUES.md](KNOWN-ISSUES.md) |
| ideas, and the wider backlog | [IDEAS.md](IDEAS.md) |
| the plans, and each one's build state | [../plans/README.md](../plans/README.md) |
| decisions NOT to change something, and what a measurement settled | [../spec/REVIEW.md](../spec/REVIEW.md) — "Decisions not to change" |
| what was done, and when | [../records/PROGRESS.md](../records/PROGRESS.md) |
| the older status snapshot, frozen at v2.4 | [../archive/STATUS.md](../archive/STATUS.md) |
