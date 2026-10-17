# v2.16 — an update can no longer cover your config, and a flow can be copied

Two changes with the same theme: what you own stops being something you retype or protect by hand. An
update now ships **program files and nothing else** — your settings live in `config\local.yaml` and are
carried across on the first launch. And a **quest flow, a quest sequence or a spammer preset can be
duplicated** instead of rebuilt click by click.

**Nothing here needs a reflash.** The Arduino sketch is unchanged since v2.15 (protocol level **2**),
confirmed with `git diff v2.15...HEAD -- arduino/` rather than remembered. It still ships with the
release, for a new board.

**What was verified, and what was not.** Stated honestly, because two of these changes only prove
themselves on a real machine.

| | |
|---|---|
| **The build and the tests** | `dotnet build v2/SealTools.sln` — 0 warnings, 0 errors. `dotnet test` — **351 tests pass**. |
| **Adversarial reviews** | Three rounds over the config/deploy work and five over the duplication work, with every finding independently checked. Later rounds kept finding real defects in the previous round's fixes, including a value-less YAML key that could stop the launcher from opening a window at all. |
| **The duplication feature** | Built, reviewed and gate-green; **not yet exercised in the game** — the test list is in the pull request, and the player runs it. |
| **The config fold** | Every fold test builds its own fixture. A **real updated install's first launch** has not been watched yet. The fold runs once, carries only the values you changed, and leaves everything you never touched tracking the shipped template. |
| **The gem cycle counts** | Covered by unit tests over the pure counter. The card row and `logs\gem.log` have not been watched in a live run. |

---

## 1. An update can no longer cover your config

**The zip carries program files only.** The shipped template goes out as `config\defaults.yaml.example`
— the base the app reads — instead of `config\defaults.yaml`. Nothing the app writes is in the archive,
so unzipping a new version over the old folder cannot overwrite a calibration, a preset or a choice.

**Your settings live in `config\local.yaml`.** The portable settings that used to be saved into
`defaults.yaml` now go into a `settings:` block in your own file, beside the calibration. An existing
`defaults.yaml` is read **once**, on the first launch after the update, and the values you changed are
folded into `local.yaml`; after that it is never read again, and deleting it loses nothing.

**The fold carries what you changed, not the whole file.** It compares section by section against the
shipped template, so a default that a later release changes still arrives for every value you never
touched — and a value you did change, including a deliberate zero, is kept.

**A save is one gated read-modify-write.** Every writer — the calibrators, the Spammer tab, the first
launch fold — goes through the same lock and the same atomic replace, so two saves cannot interleave and
a failed write cannot leave a half-written config.

**The packaging check now actually runs.** `publish.bat` opens the finished zip and fails on any member
that is not a declared program file. The previous check was written as several separately quoted
fragments, which `cmd` passed to PowerShell as separate arguments — only the first ran, and the check
silently passed everything. It is now one command, exercised against a planted `config/local.yaml`.

**Also in the package:** `config\attributes.us.yaml` no longer ships (a missing dictionary is seeded from
its `.example`), and `docs\pet-data.csv` now does — which is why a fresh install has never had a feeding
estimate until now.

**Updating is still "unzip over the old folder".** See **Updating** in `docs\spec\INSTALL.md`.

## 2. Duplicate instead of retype

The pickers for a **quest flow**, a **quest sequence** and a **spammer preset** each have a **Duplicate**
button beside Create / Rename / Delete.

- The name is generated: `name (copy)`, then `(copy 2)` and so on, resuming from a suffix the source
  already carries. It refuses, case-insensitively, to produce a name that differs from an existing one
  only by capitals.
- The copy is built from **what is on screen**, including edits you have not saved yet — not from the
  last saved config.
- The copy becomes the one you are on, exactly as Create does.
- A duplicated **sequence keeps pointing at the same flows**; duplicating it is not a reason to duplicate
  every flow inside it.
- A duplicated **spammer preset carries `Presets` only** — no key order, combos or opening step — and
  the status line says so. This matches what Create has always done.
- The old warning about "two flows with the same steps" is retired: duplication is a normal thing to do.

Buy presets and game variants are not part of this.

## 3. One Save for the whole Quest tab

**Save quest** writes everything on the tab: the active flow, every sequence, which sequence is active,
and the mouse guard. The success bar says what was written, including the step and sequence counts.

- The **Flow** and **Sequence** pickers now share one control with the Spammer tab, and Create, Rename,
  Duplicate and Delete answer in a bar beside the buttons instead of only in the line at the bottom.
- **Renaming a flow re-points every sequence entry that named it**, and the entry rows are rebuilt so the
  new name is selectable everywhere.
- **Save refuses while a sequence entry names a flow that does not exist**, listing each entry with its
  position. It does not silently re-point the entry to another flow — that would be a different quest's
  dialogue, with nothing on screen to say so.
- Delete commits the rows on screen before rebuilding, so it can no longer discard an unsaved edit.
- A value-less key in `local.yaml` (`run:`) is repaired once at load instead of crashing: YamlDotNet
  stores it as a null dictionary value that the dictionary still reports as present.
- The Quest card's loop count is a per-run override. On Save it follows the saved value only when the
  card targets the item you saved **and** the box still holds the value it was last seeded with; a count
  you dialled in on the card survives.

## 4. Gem Composer: cycles per grade

The Cycle counter said how much work a run had done, not where it went. The card now shows **one count
per grade** — `N`, `G`, `DG` and the rest — including grades not reached yet, because a list that grows
as the run proceeds moves under your eye. Each Start (the panel's and F12's) resets the counts.

`logs\gem.log` records the run starting, each grade finishing (or being left by hand with F9, which the
line says), and the run's whole tally when it ends, including a stop and a pause. The total is the sum of
the grades rather than a counter of its own, so the two cannot disagree.

While wiring this, the composer's captures and its empty-check log were moved under the install root, so
the launcher's **Delete captures** button now clears the same folder it names.
