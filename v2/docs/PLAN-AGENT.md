# Plan — an agent that can act on the game

Written 2026-10-08. **NOT BUILT — plan only.** Nothing here is committed work; the ⚑ questions in §8
decide whether any of it is.

The ask was *"explore ways to have an agent help execute some of the Seal Online actions"*. This is the
exploration: what the code already exposes, what it deliberately does not, four shapes the agent could
take, and the order they can be built in so that each step is verifiable on its own.

**One framing rule, and everything below follows from it: the agent must not be in the reflex loop.**
The evidence is in §1, and it is not a style preference — the suite's own measurements make it
impossible.

---

## 1. Why the agent is not in the loop

| Fact | Where |
|---|---|
| The cursor is the **real OS cursor**, driven by raw-HID Arduino moves in a closed loop against `GetCursorPos` | `SealTools.Core/HidPointer.cs:74-115` |
| A key press is a **30–80 ms** hold; a click must land on a calibrated point | `arduino/seal_mouse/seal_mouse.ino:32-34` |
| The Quest tool **never moves the cursor** — the player positions it during the countdown, and a hand that moves stops the run | `SealTools.Quest/QuestTool.cs:310`, `AppConfig.cs:135` |
| The boarding window and the tuner window **cannot both be open** — there is never a moment where two things drive the game | `PLAN-WATCHER.md:42-49` |
| One pass of capture + OCR + parse is **~166 ms median** (p90 214 ms, max 767 ms), derived from the 9,325-scan `ocr_log.jsonl` | §9, measured |

Perception is fast enough for a supervised loop. A model round-trip is 0.5–3 s, and a gesture is
hundreds of milliseconds — so a "think between clicks" agent would be both too slow and unsafe.

**Therefore: the agent plans, supervises and reads; the suite stays the hands.**

---

## 2. What already exists

**The eyes need no new seam at all.** Every read is public already:

| Need | Public API |
|---|---|
| find the window | `WindowFinder.FindByTitle(string)` — `WindowFinder.cs:94` |
| grab a region | `ScreenCapture.CaptureClientRegion(IntPtr, RegionConfig)` — `ScreenCapture.cs:55` |
| grab the whole client | `ScreenCapture.CaptureClient(IntPtr)` — `:51` |
| read a region | `OcrEngine.ReadLines` / `ReadLinesScored(region, upscale, saveDebug, minScore)` — `OcrEngine.cs:119`, `:139` |
| read a saved image | `OcrEngine.ReadImageFile(path, …)` — `:164` |
| service-level read | `LauncherService.ReadText` :845, `ReadTextScored` :853, `CheckOcr` :836 |

A region is a `RegionConfig { left, top, width, height }` (`AppConfig.cs:772-780`) in
**client-relative physical pixels** — the same space as every calibrated box (`COORDINATES.md:161`).

**The hands are the opposite: private, duplicated, and with no seam.**

- **No public "click this calibrated point."** Five callers each re-implement place-then-click privately —
  `PetTool.cs:2244`/`:2201`, `ShopTool.cs:321`/`:312`, `GemComposer.cs:104`, `MainWindow.xaml.cs:5096`,
  `SealTuner.cs:288`.
- **No public key press at all.** `SkillSpammer.SendKey` and `QuestTool.Send` are both `private static`
  (`SkillSpammer.cs:371`, `QuestTool.cs:324`), and `HidPointer` has no keyboard method.
- **18 raw `SerialPort.Write` sites**, no single write path. The closest shared seam is the static
  `HidPointer` (5 methods), and `E`, `K`/`F`, `Q`/`Z`, `P`/`U`, `V` all bypass it.
- Point names resolve only through `GemRoutes.Resolve(cfg, name)` (`GemRoutes.cs:46`) — `gem.*` keys and
  `Resource1..3`. Every other point is read straight off its config key.

**Coordination exists and is the right door.** `LauncherService.AcquireInputAsync()` claims the gate
*before* opening the port and hands back an `InputLease` carrying the `SerialPort`
(`LauncherService.cs:276-309`, `InputArbiter.cs:80-101`). 18 UI sites already use it. `PortGate` carries
deadlines and a waiting signal; **`ShouldStandDown` still has no production caller** (`PortGate.cs:123`,
`PLAN-ARBITER.md:190-205`).

**Authoring is already data, not code.** A flow is `QuestConfig` → named `QuestPreset` → `QuestStep`
(`Action` = `click|right_click|key|enter|wait`, `DelaySeconds`) with `Loops` and one positioning wait, plus
`QuestSequence` for several flows at one NPC (`AppConfig.cs:101-206`). It lives in `local.yaml`. Spammer
presets and tuner rules are config in the same way.

**The evidence channel is already safe to read from outside.** `FileLogger` opens with
`FileShare.ReadWrite` and `AutoFlush` (`FileLogger.cs:17-18`), so an external process can tail the logs
while the app runs. `ocr_log.jsonl` carries per-scan `timestamp, grade, remaining, gradeLine, attributes,
colorScores`, the client/region rects, `dpi`, `scale`, every detected line with its box and confidence,
and the raw per-character boxes with their bucket key (`OcrEngine.cs:359-384`).

---

## 3. What does not exist, and should not be assumed

- **No external entry point of any kind.** `App.OnStartup` ignores `e.Args`
  (`SealTools.Launcher/App.xaml.cs:15`), and there is **no** `HttpListener`, `NamedPipe`, `TcpListener`,
  socket or IPC anywhere in `v2/`. An agent either runs **in-process** through `LauncherService` (public
  ctor, but the assembly is `net8.0-windows` WinExe) or a channel is added deliberately.
- **Two design principles stand in the way, on purpose** (`v2/README.md:90-94`): *"no `control.txt` /
  `state.json` files (those were v1 leftovers for separate processes)"* and *"logs are never read back to
  control the program."* A read-only consumer is a different thing from a control channel, but the
  amendment should be **decided**, not drifted into.
- **No dry run for quest, pet, spammer, tuner or gem.** The two that exist are private UI handlers —
  `BuyDryRun` (`MainWindow.xaml.cs:5111`) and `SellDryRun` (`:5166`) — and the buy one still sends a real
  focus click (`:5136`).
- **No motion replay at all.** v1 had `macro_recorder` (F9 record → JSON → F10 replay via Arduino);
  v2 never ported it, and Quest deliberately sends no cursor move. The firmware *does* have an unused
  Bezier `H dx dy ms` move and a host-side `W ms` wait (`seal_mouse.ino:196`, `:253`) — available, but
  uncalled, so unverified.

**Blind spots an agent will meet immediately:** no bag OCR (by design, `PLAN-PET-AUTOFEED.md:75`), no
quest-list read (`TODO.md:57-60`), shop and spammer read nothing. And the **foreground guard exists in
only three places** — `PetTool.cs:905`, `:1679`, `GemComposer.cs:197` — so `OcrEngine.ReadLines` will
happily read whatever window is in front and report a confident, wrong answer. The live proof is already
in the log: a non-focused game made both crops score 0.915 and the tool reported "no queued pet in the
bag" — *the right verdict from the wrong image* (`PetTool.cs:1679-1689`).

---

## 4. The four shapes

| | What the agent does | New input path | Size | Risk |
|---|---|---|---|---|
| **A. Author** | Writes flows/presets from a description; the player presses Start | none | S–M | Low |
| **B. Supervise** | Reads ToolState, logs and saved crops; reports, warns, explains a stop | none — read-only | S | ~zero |
| **C. Conduct** | Starts/stops tools and picks presets, as one more arbiter claimant | yes — a channel | M–L | New attack surface |
| **D. Perceive → act** | Screenshot → model → decides and clicks | yes, unmodelled | L | **Highest** |

**A** is the cheapest real win: *"hand in the daily ×20"* becomes a flow the agent writes into
`local.yaml`, refused by `Validate` before anything is clicked if a step is unsendable. Its honest
limitation is the tool's own: Quest reads nothing, so a wrong flow does `Loops` wrong clicks and only the
player notices (`TOOL-FLOWS.md:1181-1183`). A **dry run** closes most of that gap and does not exist yet.

**B** needs no code in the app at all — §2's logs are already externally readable. It answers *"is the pet
fed?"*, *"why did the tuner stop?"*, *"what grade did it land?"*, and it is the half that makes C safe.

**C** is defensible because of `PLAN-ARBITER.md`'s boundary: **the arbiter governs access, never
behaviour** (`:101-109`). An agent claims input exactly as a tool does, and never learns what a pet row
is. Two things it must get right: **loopback-only with an explicit token** (this channel can press keys
in the player's game), and the fact that the launcher is sometimes run **as Administrator**
(`INSTALL.md`), which decides how the pipe ACL works. One genuine advantage: an IPC message is not a
synthesized keypress, so the anti-cheat that blocks background hotkeys is not in the way — the Arduino
stays the only input path.

**D** has the highest ceiling and is the one to push back on as stated. The repo's invariants exist
because acting on an unverified guess loses things — *click, then LOOK*; *unknown does not skip*; *a MOVE
is not a CLICK* (`DESIGN.md:134-142`). A model is a guess generator. Where it **is** defensible is
bounded recovery: classify a state the tools do not model, then either pick among **enumerated safe
recoveries** or **stop and tell the player**. Never free-form clicking. Its prerequisites are already
listed in `IDEAS.md:33-41` — a **global human override** and a **panic stop that works in-game**.

---

## 5. The seams any of A–C needs

Small, independent of the agent, and worth doing on their own merits — five private copies of one
operation is how they drift apart.

1. **`HidPointer.Key(SerialPort, char, bool fast)`** plus a hold/release pair. Today the only two writers
   are `private static`, and the fast/normal distinction (lowercase/uppercase opcode → 10 ms / 30–80 ms)
   is re-encoded at each site.
2. **`TryClickPoint(SerialPort, DisplayInfo, int physicalX, int physicalY)`** — the place-then-click that
   five callers each wrote.
3. **A foreground guard on every read that judges pixels.** Three of the twelve named reads have one
   (§3). This is the fix for the failure that produced a confident wrong answer, and an agent reader
   needs it more than a tool does, because the agent cannot see the screen.
4. **A quest dry run**, mirroring `SellDryRun`: validate, place, and log every step that *would* be sent
   without clicking. `Test one loop` already runs identical code (`TOOL-FLOWS.md:1200-1201`), so this is
   the same pattern.

---

## 6. The observation channel needs no code

Option B is available today. An external reader tails, with no change to the app:

- `logs/ocr_log.jsonl`, `ocr_errors.jsonl`, `run_<ts>.jsonl` — tuner, everything about every scan;
- `logs/pet.log` — the feeder's one-line-per-decision record, including the icon-scan winner **and
  runner-up**, and the landed cursor positions;
- `logs/quest.log` — every step of every flow, the durable record of a self-ended run;
- `logs/reads/` — the crops themselves, each with a sibling `.txt` of `score\ttext`.

**One trap, and it is easy to hit:** the logs are written to **two different directories** —
`<root>/logs/` for `OcrEngine`, `AppContext.BaseDirectory/logs/` for the launcher, `PetTool`,
`QuestTool` and `GemComposer` (`MainWindow.xaml.cs:3611-3613` says runtime logs must *not* go to
`AppContext.BaseDirectory`, and `PetTool.cs:984` does it anyway). A reader must look in both. In a
published build they are the same folder; in a dev tree they are not.

---

## 7. What the agent is genuinely better at

Ranked by value over what exists, not by novelty:

1. **Growing the US OCR dictionary.** `PLAN-US-CLIENT.md:108-115` is explicit that the dictionary's value
   is its **variants table**, that it is *"grown rather than written"*, that English fails completely
   differently (`l/I/1`, `O/0`, `rn/m`) so nothing transfers — and that *"the machinery to run that
   process already exists"*. That is `ocr_log.jsonl` + `logs/reads/` + `attributes.yaml`. This is the
   loop the TW table was built by, done by hand, and it is currently blocking a shipped feature.
2. **Authoring flows and presets** from a description (Option A).
3. **Explaining a stop.** *Done / StoppedByGuard / Cancelled* are told apart only by `Message` and
   `logs/quest.log` (`TOOL-FLOWS.md:1189-1191`) — correlating that with the saved crops is exactly the
   work that has repeatedly needed a human.
4. **Reporting** — "pet fed at 14:02, food count 3, next due 17:40".
5. **Choosing among presets**, which is judgement over slow things and needs no new capability.

---

## 8. Open questions for the player ⚑

1. **Attended or unattended?** With the player at the keyboard, the mouse guard is a feature and A+B are
   enough. Unattended puts D on the table — and then the panic stop from `IDEAS.md:40-41` is a
   prerequisite, not a follow-up.
2. **Where does the agent run?** Same PC in-process (reference the WinExe), or out-of-process via a
   channel that does not exist yet? If the player is willing to press Start, **A+B need no channel at
   all**, which removes the entire security question.
3. **May the agent start and stop tools, or only read and advise?** That is the line between B and C.
4. **Does the "logs are never read back" principle get amended?** A read-only consumer is not a control
   channel; the amendment should be written down in `v2/README.md:90-94` rather than left implicit.

---

## 9. What was measured, and what was only reasoned about

**Measured (from the code and the logs in this checkout):** the 18 raw write sites and the
public/private split of every read and action; the absence of any IPC, socket or CLI; `FileLogger`'s
`FileShare.ReadWrite` + autoflush; the 9,325-line `ocr_log.jsonl`; the two-directory log split; the three
foreground guards; the two private dry runs; the firmware's unused `H`/`T`/`S`/`X`/`W` opcodes.

**Derived, not documented:** the **~166 ms** capture+OCR+parse pass. It comes from the deltas between
consecutive `ocr_log.jsonl` entries taken at the start of each `ScanOnce` (`OcrEngine.cs:236`), n=54
back-to-back retries. Nothing in the codebase wraps a capture or an OCR call in a `Stopwatch`.

**Reasoned, not measured:** that a vision model could read the US attribute lines — `PLAN-US-CLIENT.md:100-106`
says explicitly the English read is **unmeasured**; the latency budgets in §1; the effort sizes, which
follow `IDEAS.md`'s convention rather than an estimate from having built it; and every claim about what a
model would decide, which no run here can support.

**Not verified, and known to be stale:** the `PrintWindow` evidence PNGs cited by `STATUS.md:177-178` are
no longer on disk (only the conclusion survives); `FeederCount.MinScore` says 0.75 while the live
`local.yaml:355` sets 0.6; and the gem empty threshold is 0.01 in code and config while `CONFIG.md:99`
still says 0.18. An agent that trusts the documents over the config will be wrong in these three places.

---

## 10. Recommended order

Each step is verifiable on its own, and the first two cannot regress a working tool.

1. **The seams (§5)** — S. No behaviour change, testable with no board.
2. **The observation reader (§6)** — S, and *outside* the app: no code in the suite, no risk. This is the
   half that delivers value immediately and makes everything after it safer.
3. **Dictionary growth (§7.1)** — S–M, and it unblocks `PLAN-US-CLIENT`'s blocker rather than adding to it.
4. **Option A with the dry run (§4)** — S–M. The first shape where the agent changes what gets sent.
5. **Option C (§4)** — M–L, and only after ⚑ 2, 3 and 4 in §8 are answered. Needs the arbiter's own step
   3 (operation-level leases, and the normal-stop / emergency-stop split) to exist first.
6. **Option D (§4)** — L, and only as bounded recovery, and only after a panic stop exists.
