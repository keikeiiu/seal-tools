# Plan — v3: a Rust core, and what would actually make the code solid

Written 2026-10-10. **DESIGN ONLY — nothing built, nothing ported, no project created.**

The ask was two things at once: *"take time to see if we can learn something on the way"*, and
*"make the code more solid and structured better, as well as how the functions are structured."*
Cost is explicitly not the driver, so this document does not argue from megabytes. The earlier
size survey is kept only in §7, where it belongs — as a consequence, not a reason.

**This is not a v3 by the repo's own definition.** `PLAN-PATHS.md:117` and `PROGRESS.md:33`:

> **Not a v3.** … **A major version is for a changed user-visible contract, not for code that got
> better.**

Nothing here changes what a player does. The version number is the least interesting part of the
question, and it should be decided last, not first.

---

## 1. The measured diagnosis

`v2/`, excluding `obj/` and `bin/`:

```
  9058  SealTools.Launcher\MainWindow.xaml.cs
  2025  SealTools.Pet\PetTool.cs
  1448  SealTools.Tests\ConfigLoaderTests.cs
   889  SealTools.Core\Config\AppConfig.cs
   852  SealTools.Launcher\LauncherService.cs
   806  SealTools.Core\Config\ConfigLoader.cs
   548  SealTools.Core\OcrEngine.cs
   ...
  71 files, 22,859 lines

interfaces declared in v2                          none
SealTools.Tests references                         Core, Tuner, Spammer, GemComposer
test cases                                         233
```

Three facts read together are the whole story:

- **Zero interfaces across 22,859 lines.** There are no seams anywhere. That is why `TODO.md` has
  to say *"the tool lifecycle lock has no test, because there is no seam to test it through"* — the
  missing test is a symptom, and the seam is the fix.
- **The test project cannot see the two largest files.** `MainWindow.xaml.cs` (9,058) is in a
  `net8.0-windows` WinExe the test assembly cannot reference; `SealTools.Pet` (2,025), `Shop` and
  `Quest` are simply not referenced. **~12,600 lines have no coverage, and they include the largest
  and most frequently changed files in the repo.**
- **`Core` is genuinely good.** 35 files, 4,828 lines — about 138 lines per file. The organisation
  is not the problem and should not be "fixed".

**So the diagnosis is not "the code is badly organised". It is "the code is badly bounded".**
Domain logic and view logic are fused in the launcher, and the language offers no vocabulary for a
seam, so the most valuable code is untestable *by construction* rather than by neglect.

---

## 2. The shape it should have

Two layers, and the rule is one sentence: **the domain never knows the screen exists.**

| | contains | may touch |
|---|---|---|
| **domain** | the config model, OCR geometry and the read pipeline, `PortGate`/`InputArbiter`, the decision each tool makes, scheduling, the attribute matcher | nothing — no `SerialPort`, no `Window`, no `Dispatcher`, no `AppContext.BaseDirectory` |
| **shell** | WPF views, the calibration canvas, Win32 window/DPI/GDI, HID placement, logging destinations | everything |

`Core` is already most of the way to being the domain layer; what breaks the rule today is
`MainWindow.xaml.cs` (domain decisions inside click handlers) and `PetTool.cs` (a visit loop that
reads, decides and acts in the same methods).

### 2.1 How a function should be structured

The dominant shape today is *one large method that reads, decides and acts, on a class that owns
the port*. That is exactly why it cannot be tested. The target is three separable things:

```
decide(read, cfg) -> Decision            pure; no I/O; trivially testable
enum Decision { Click(Point), Key(char), Wait(f64), Stop(Reason) }
loop { match decide(...) { ... } }        thin; the only part that touches the board
```

- **`decide` is pure.** Same read and same config gives the same `Decision`, forever.
- **`Decision` is data, not an effect.** It can be asserted on, logged, diffed and replayed.
- **The effect loop is boring.** It should contain no `if` about pet rows, gem grades or spammer
  cooldowns — because every branch in it is a branch no test can reach.

**This is not a new idea for this repo — it is the one that already worked.**
`SpammerOrder` is the spammer's pure decision half, and it is the tested part of that tool
(`SpammerOrderTests.cs`). The recommendation is to generalise the single thing that succeeded
rather than invent anything.

### 2.2 What "more solid" means here, concretely

Not "fewer bugs" in the abstract. The repo's own recurring failure is named in
`PROGRESS.md` entry 63: *a message that tells the player the wrong thing.* Nine ranked findings,
all of that class. Every one of them is a **decision made somewhere it could not be tested** —
inside a click handler, in a `catch`, in a string built beside the thing it describes.

So: solid = the decisions are in functions that can be called from a test, and the messages are
produced by the same code path that made the decision, not written next to it.

---

## 3. Why Rust, specifically

The learning value is real, but it is not "Rust is faster". It is that **the invariants this
codebase currently enforces with comments and discipline become compile errors.**

| the invariant, as this repo words it | what enforces it today | what would enforce it in Rust |
|---|---|---|
| *"Unknown boards; **unknown does not skip**"* — a guard may only ever remove a boarding | a comment at the call site | exhaustive `match` on a state enum: a new variant does not compile until it is handled |
| *"**Click, then LOOK.** Every risky action checks its own effect"* | discipline, and the one tool that did not lost food silently | a `Result` that must be handled, plus `#[must_use]` on the effect |
| held keys and buttons must be released on every stop path | `HeldKeys` plus two hand-written `finally` blocks | a guard type whose `Drop` releases — a stuck left button becomes unrepresentable |
| `ToolState`'s *"reads are atomic"* comment is false for `int?` and `DateTime?` (`PROGRESS.md` entry 68) | nothing; the comment is wrong | `Option` behind a synchronization type; the torn read cannot be written |
| the config projection drops a field silently — **four times now** | hand-written field lists plus reflection fences | `serde` derive, or a compile error that names the missing field |
| *"a stop abandons the WORK, not the CLEANUP"* | `WithoutCancellation`, restorable, nestable, easy to forget | an explicit `Cancel`-aware type; forgetting it is visible in the signature |
| the missing test seam (`TODO.md`: *"the fix is the seam, not a test"*) | nothing — the interface does not exist | `trait Tool`; a test implements a fake board and the lifecycle is testable |

Read that column as the learning syllabus: each row is a place where the language would teach
something by refusing to compile.

---

## 4. What Rust + Tauri would and would not fix

**Would fix, if and only if the boundaries change:**

- **Tauri forces the UI boundary.** Front end in web, behaviour behind a command API. That is
  precisely the seam the launcher lacks, imposed by the architecture instead of by discipline.
- **The traits the codebase has none of.** `trait Tool`, `trait Board`, `trait Reader` — and with
  them, the lifecycle, the arbiter and every decision become testable in `cargo test`, with no
  Windows and no board.

**Would not fix, and this is the trap:**

- **A line-by-line port reproduces the problem exactly.** 9,058 lines of code-behind becomes a
  9,000-line `main.rs` and a 9,000-line front end. The port must be through the *boundaries*, not
  through the files.
- **Function structure is a design decision, not a language feature.** Rust makes the good shape
  cheaper to keep and the bad shape more annoying to write; it does not choose for you.

---

## 5. The port sequence

Each step leaves v2 shippable on its own — the repo's own habit (`PLAN-PATHS.md` §5), and the
reason this does not need to be a big-bang project.

1. **Draw the boundaries in C#, in place.** Pull the decisions out of the two god-objects into pure
   `decide` functions returning a `Decision`. No new language, immediate testability, and — the
   point — **a port afterwards is mechanical instead of interpretive.** If Rust never happens, this
   step is still the single biggest win available.
2. **Port the domain to Rust, with the 233 tests as the contract.** A `seal-domain` crate: config
   model, OCR geometry, the arbiter and gate, the matchers, the decisions. The port is *done* when
   the equivalent tests pass — that is the oracle, and it is the reason this is a learning project
   rather than a rewrite: success is measurable at every step.
3. **Keep the WPF shell until the core is proven**, and call into the crate over a C ABI. Two
   languages in one product is a real cost; it is also the only way to learn the Rust half without
   staking a tool that runs someone's game for days at a time.
4. **Then decide the shell on its own merits.** Tauri (rewrite the UI), or WPF over the Rust core
   (keep it). This is the ⚑ in §8.1 and it should be decided *after* step 2, when the Rust side is
   a known quantity rather than an aspiration.

---

## 6. What a port cannot recover

Stated plainly, because it is the real price and it is not money.

- **Those ~12,600 untested lines are where the accumulated wisdom lives, and it lives in code, not
  in tests.** A port re-derives them from the documentation. The docs are unusually good — but they
  are a *summary* of the findings, not a specification of the code.
- **The measured quirks survive as text and must be re-earned as behaviour**: a crop cut to the
  digits' height reads *nothing* while holding a legible number; the feeder count's correct band is
  ~3 native pixels wide and outside it the reader is *confidently wrong*; `PrintWindow` returns a
  black frame; the cursor closed loop's damping; every calibration gotcha in `COORDINATES.md`.
- **The bug tail restarts.** `PROGRESS.md` entry 63 found nine "says the wrong thing" defects in the
  launcher alone. A new shell starts that count at zero with the same class of mistakes available.

---

## 7. The size survey, kept only as a consequence

Not a reason to do this, but worth knowing because it is often assumed to be the reason: a
Rust + Tauri app would replace the 174.4 MB self-contained `.exe` with a small binary plus WebView2.
OpenCV native (92.7 MB), ONNX Runtime (15.4 MB) and the models (15.4 MB) stay unless the Rust stack
replaces them, so the honest estimate is roughly a halving — **not** the 10× that "Tauri" suggests.
See the measurement in `PROGRESS.md` for 2026-10-10 if it is ever wanted.

Separately, and worth doing regardless: **`opencv_videoio_ffmpeg` (27.3 MB) and `libSkiaSharp`
(11.7 MB) are pulled in for capabilities this app does not use** (video I/O; Skia drawing). Whether
they are P/Invoked at all is a one-hour check, and if they are not, ~39 MB comes off v2 today with
no rewrite.

---

## 8. Open questions ⚑

### 8.1 Rust core, or Rust app?

| | Reading A — Rust core behind the existing shell | Reading B — Rust app (Tauri) |
|---|---|---|
| what is ported | the domain | the domain and the 9,058-line launcher UI |
| learning | Rust types, traits, `Result`, testing | all of the above, plus a web front end and IPC |
| risk to the working tool | low — the shell keeps running while the core is proven | high — nothing runs until the shell works |
| when | now | after A |

**A first, B as the destination** — the same reasoning `PLAN-ARBITER.md` §6 used for the arbiter:
the cheap reading lands the policy, and the expensive reading is kept as a named later stage rather
than abandoned.

### 8.2 Hard requirements, not preferences

- **`local.yaml` must keep loading.** It holds the player's per-machine calibration; a v3 that
  cannot read the existing file throws away months of measuring. `defaults.yaml`,
  `attributes.yaml` and `local.yaml` are read by a port, not replaced by it.
- **The Arduino protocol does not change.** It is at level 2 and pinned by `FirmwareSketchTests`.
  A v3 that needs a new firmware level forces every player to reflash, which is a user-visible
  contract change — the one thing that *would* make this a real v3, and it should be avoided.
- **Windows-only stays Windows-only.** `AGENTS.md`: *"Don't add cross-platform abstractions."* The
  Rust core is not a step toward Linux; the shell is Win32 and GDI by measurement, not by accident.

### 8.3 Still open

1. **Does the `.NET 8` EOL (2026-11-10) force a decision now?** A TFM bump to .NET 10 is already a
   dated item in `TODO.md` and is independent of this plan. Doing the TFM bump *first* keeps them
   from being confused with each other.
2. **Do the 233 tests travel, or are they rewritten?** Translating them is honest but doubles the
   work; writing Rust tests from the docs is faster and loses the regression value of the existing
   ones. Step 2 assumes translation of the ones that describe *decisions*, and dropping the ones
   that test C#-specific plumbing.
3. **How is parser/Ocr parity verified?** OCR is the highest-risk port: `RapidOCRSharp` wraps the
   PP-OCRv4 det/rec/cls pipeline, and in Rust that is `ort` plus re-implementing DB post-processing,
   CTC decode and the dictionary. The cheap answer is to keep the OCR **out of the port** initially
   and call the existing pipeline through the shell. That deserves its own ⚑ before step 2.

---

## 9. What was measured, and what was only reasoned about

**Measured:** every line count in §1; the absence of interfaces; the test project's references and
that it cannot see `Launcher` or `Pet`; the 233 test cases; `Core`'s 138 lines per file; the
174.4 MB / 92.7 MB / 15.4 MB / 15.4 MB breakdown and the two unused natives in §7; the quoted
invariants, each of which is a line in this repo's own documentation.

**Read out of the source, not executed:** that `MainWindow.xaml.cs` mixes domain decisions into
handlers, and that `PetTool`'s visit loop reads, decides and acts in the same methods — both are
stated by the repo's own audits (`ANALYSIS-UI.md`, `AUDIT-PET-FLOW.md`) and by entry 63.

**Reasoned, not measured:** that Rust would make each invariant in §3 a compile error — true of the
language, untested in this codebase; the sizes in §7's estimate; the step costs in §5; and the claim
that the "says the wrong thing" class would restart with a new shell, which is inference from entry
63 rather than a measurement of a shell that does not exist.

**Not attempted:** no Rust project created, no crate chosen, no `.csproj` or config file touched, no
branch cut. This document changes nothing.
