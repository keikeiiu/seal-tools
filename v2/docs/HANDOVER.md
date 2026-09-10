# Handover — paste this into a new session

Written 2026-09-11, at the end of the session that produced **v2.3**. Copy the block below as the
first message of the next session.

---

> Continue Seal Tools v2. Read `v2/docs/PROGRESS.md` first (newest entry first — it records what was
> decided and why, which the code and commit titles don't), then `v2/docs/TODO.md` for what is open
> and `v2/docs/IDEAS.md` for the wider backlog. `v2/docs/STATUS.md` has the commit-by-commit history
> of each pass. Do not re-derive what those say.
>
> **Where things are.** `main` is at **v2.3** (tagged), the UI work of the last session is merged.
> The launcher runs from `v2/SealTools.Launcher/bin/Debug/net8.0-windows/` — it is very often
> already running, which **locks the build output**, so stop it before building
> (`Stop-Process -Name SealTools.Launcher -Force`) or the build fails with "file is locked".
> `v2/dist/` is the only intact copy of the hand-tuned calibration — never modify it.
>
> **How I want you to work** (this is the part that matters most):
>
> - Commit every small victory as it lands, one concern per commit, with a message that says *why*
>   and what was measured — the style of the existing commits. Split by file where the halves both
>   build.
> - Keep `v2/docs/PROGRESS.md` current in the same session, and tick the relevant checklist in the
>   plan doc in the same commit as the change it describes.
> - For anything UI-shaped, **study and agree the structure before writing code** — layout work is
>   the expensive thing to redo. `PLAN-UI-CLEANUP.md` is the model: inventory, problems, principles,
>   open questions, then one tab per commit.
> - Ask me when a request is ambiguous or when a decision changes what gets built. I would rather
>   answer one question than discard a wrong change.
> - Verify claims. Measure, don't assume: several "obvious" causes in the last sessions were wrong
>   (the Arduino-port theory, the foreground guard, the DPI context), and the real cause was found
>   by logging and comparing pixels. Say plainly what you verified live and what you only reasoned
>   about.
>
> **Open threads, roughly in order of value:**
>
> 1. **The v2.3 zip is not built or published.** `v2/publish.bat` packages it; the root README's
>    download row points at a release that does not exist yet. `v2/dist/` also still holds v2.1
>    copies.
> 2. **The robustness list in `IDEAS.md`** — the stale-calibration guard (refuse to run when the
>    live client size or UI fingerprint doesn't match the calibration) and unified logs + a build
>    stamp are both small. The empty-check hardening is done.
> 3. **`PLAN-TUNER-SPRING.md`** — placing the cursor on the 發條 button with the closed loop, plus a
>    `mouse_guard` toggle (stop when the mouse moves, or re-centre and carry on). Needs my call on
>    the guard default, and one measurement first: does the game warp the cursor during a run?
> 4. **The spammer key pad** (`IDEAS.md` §7b) — designed, not built; the open question is where
>    cooldowns get edited.
> 5. **The tuner's OCR row-bucket question** (`TODO.md`) — needs evidence from `logs/ocr_log.jsonl`
>    before any code changes.
>
> **Traps that have bitten us — check these before doubting your own fix:**
>
> - `Stop-Process` kills the process without firing the window's `Closed` handler, so anything
>   saved there is lost. (That is why window placement now saves on a debounce instead.)
> - WPF silently clamps a window below its content's minimum height — a height constant that looks
>   wrong may be a clamp, not a bug. Measure what the window actually became.
> - `ui:CardControl` measures its content with **unbounded width**, so wrapping text runs past the
>   border. Use `ui:Card` with our own header.
> - The tab strip wraps to a second row below roughly 1100px wide, and in a short window the wrapped
>   rows fall out of view — which looks like "the tabs are gone".
> - Screenshot comparisons must not include a window's drawn border: a one-pixel shift in where the
>   crop lands moves those rows and nothing else. That is what the empty-check inset is for.
> - The game window must be the **foreground** window for a screen-grab check to mean anything;
>   `CopyFromScreen` reports whatever is in front.
