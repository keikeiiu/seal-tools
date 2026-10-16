# Plan — duplicate: fork a flow, a sequence or a preset

Written 2026-10-17, measured against `feature/frozen-templates`. The ⚑ questions in §10 are the player's to
answer before any of it is built.

One sentence is the whole objective: **an existing preset should be forkable instead of retyped.** The
Quest flow picker, the Quest sequence picker and the Skill Spammer preset picker each get a **Duplicate**
beside Rename.

The answer in brief: all three pickers already have the same shape — an `＋ Add new…` marker, a Rename and
a Delete sharing one inline naming row — so Duplicate is a fourth action on that row plus a deep copy of
one dictionary entry under a generated `"<name> (copy)"` name, and the copy becomes active the way Create
does. The only genuinely new code is a pure static copy helper in `SealTools.Core`, because the Launcher is
excluded from the test project and the two ways this feature can go wrong — a shared list, and a name
collision — are exactly what a test can pin.

---

## 1. Why

| Symptom (measured) | Why it follows |
|---|---|
| A second similar flow means typing every step again | Quest flows are a step list with a per-step delay (`QuestPreset.Steps`, AppConfig.cs:183); a Spammer preset is a key→cooldown map (`SpammerConfig.Presets`, AppConfig.cs:975). Nothing in the UI copies either |
| "A flow like this one, but two steps longer" is a manual re-derivation | The three pickers expose only `＋ Add new…` / Rename / Delete; the Create branch inserts an *empty* preset (`new QuestPreset()`, MainWindow.xaml.cs:6477; `new Dictionary<string, double>()`, :2082) |
| The spec forbids the obvious workaround out loud | "Renaming **moves** the flow rather than copying it — two flows with the same steps is how the wrong one gets run" (USER_GUIDE.md:495, twin at USER_GUIDE.zh-TW.md:452) |

The rule above is not wrong, it is narrow: it is about **renaming**, and it was written when a copy could
only appear by accident. A deliberate Duplicate is a different thing — a starting point you then edit into
something else. The player has decided the prohibition goes (§7), while Rename keeps moving (§6).

## 2. In scope, and deliberately not

**In scope** — every picker in the launcher that already has the `Add / Rename / Delete` shape. Exactly
three exist; that was checked by grep, not by eyeballing (the Rename *button* is created only at
MainWindow.xaml.cs:2114, :6405 and :6596, and the `＋ Add new…` sentinel constants only at :2020, :6322
and :6518).

| Picker | Config | Identity | Row that gets the button |
|---|---|---|---|
| Quest flow | `QuestConfig.Presets` `Dictionary<string, QuestPreset>` (AppConfig.cs:134) | dictionary key = the name | `pickRow` StackPanel, MainWindow.xaml.cs:6492-6497 |
| Quest sequence | `QuestConfig.Sequences` `Dictionary<string, QuestSequence>` (AppConfig.cs:142) | dictionary key = the name | `seqPickRow` StackPanel, MainWindow.xaml.cs:6691-6696 |
| Skill Spammer preset | `SpammerConfig.Presets` `Dictionary<string, Dictionary<string, double>>` (AppConfig.cs:975) | dictionary key = the name | `pickRow` Grid, 3 columns, MainWindow.xaml.cs:2169-2178 |

**Not in scope, each for a stated reason** — so that "every preset picker" is a claim the plan has actually
audited rather than a slogan:

| Left out | Why |
|---|---|
| **Buy preset picker** (MainWindow.xaml.cs:4875-4937) — decided | It is a different shape: a free-text Name box + **Save item** (create-or-overwrite) + Delete item + Reload, with no `＋ Add new…` marker and no rename row. It could not take the same recipe without first being given a rename row, and it writes on the spot rather than on Save. Out of this change; see §11 |
| **Setup tab — Game Variants** (AppConfig.cs:87) | Name-keyed and drawn as one button per variant, but the UI only *switches*: a client is added by editing `defaults.yaml`. Nothing to duplicate |
| **Tuner, GemComposer, Pet, Attributes tabs** | No name-keyed preset collection. Their controls are add/remove row editors — the rules editor (MainWindow.xaml.cs:10175), "New combo" (:2495), "+ Add row" (:6024), the pet queue (:7259) — not named presets |
| **Run-card mirrors** (`_buyPresetCard` :300, `_spammerPresetCard` :308, `_questTargetCard` :322) | The same choice as the tabs, with no Add/Rename/Delete of their own |
| **Sentinel rows** | `＋ Add new…` (:2020, :6322), `＋ Add new sequence…` (:6518), `(none — run the single flow above)` (:6517) are not presets, and are never a Duplicate target or a generated name |

## 3. What must actually be copied

The one real correctness trap. Every one of these is a `List<>` of reference-type objects or a nested map,
so `dict[newName] = dict[oldName]` would give the copy and the original **the same objects** — editing the
copy would silently edit the original, with nothing on screen to say so.

| Type | Member | Shape |
|---|---|---|
| `QuestPreset` | `Steps` (AppConfig.cs:183) | `List<QuestStep>`; each `QuestStep` is Action string, Value string?, DelaySeconds double (:224-233) |
| `QuestSequence` | `Entries` (AppConfig.cs:207) | `List<QuestSequenceEntry>`; each is Preset string, Times int (:212-217) |
| Spammer | `Presets` (AppConfig.cs:975) | value is `Dictionary<string, double>` — refresh the map, the values are scalars |

## 4. Where the code goes

A new pure static class **`SealTools.Core.Config.PresetCopy`**, in Core rather than in the Launcher:

```csharp
public static string CopyName(string source, IEnumerable<string> taken)
public static QuestPreset CopyQuestPreset(QuestPreset source)
public static QuestSequence CopyQuestSequence(QuestSequence source)
public static Dictionary<string, double> CopySpammerKeys(Dictionary<string, double> source)
```

Core, because `SealTools.Tests.csproj:25` keeps the Launcher (`net8.0-windows`) out of the test project,
while the two failure modes above are pure logic over types that already live in Core. The Launcher then
holds only the button, the Click wiring and the existing refresh calls — `RefreshPresetList` (:6345) +
`ShowActive` (:6395) + `RefreshQuestCardTargets` (:5104) for a flow, `RefreshSequenceList` (:6585) +
`ShowSequence` (:6575) + `RefreshQuestCardTargets` for a sequence, and `RefreshPresetList` + `LoadRows`
(:1997) + `RefreshSpammerCard` (:4975) for a Spammer preset. The flow's tail gains `RefreshQuestCardTargets`
that Create does not call: the card's target list is built from the flows **and** the sequences (:5104-5113),
so without it a duplicated flow would not reach the card until Save.

The Spammer handler has one extra step the other two do not: it must `Commit(RowsToKeys())` for the preset
on screen **before** copying, or the copy would be taken from the last save rather than from what the
player is looking at.

## 5. The name

The copy is named `"<name> (copy)"` — one ASCII space before the parenthesis. The generator strips any
trailing `" (copy)"` or `" (copy <n>)"` from the source **first**, which yields a stem and the index that
stem already stands at (0 when there was no suffix, otherwise `n`); it then returns the first free name in
`stem + " (copy <k>)"` for `k = start + 1, start + 2, …`:

```
mine           ->  mine (copy)      mine (copy) taken?  ->  mine (copy 2), then (copy 3) …
mine (copy)    ->  mine (copy 2)
mine (copy 2)  ->  mine (copy 3)                        (never back down to "mine (copy)")
```

The stem strip and the starting index are the two parts that are easy to get wrong, in opposite
directions: without the strip the rule and its own first example contradict each other, and the second
duplicate of a flow produces a name nobody wants; without the index, duplicating `mine (copy 2)` walks
*backwards* into a free `mine (copy)` and produces a name that sorts above its own source. Duplicate never
mutates or removes the source key, and because the suffix is appended it can never collide with a sentinel
row.

The generated name is then just a name — the existing Rename row renames it, and the existing guard
refuses a name that is already taken (MainWindow.xaml.cs:6461; the sequence's twin is :6655 and the
Spammer's is :2078). A ⚑ question (§10) is whether the *collision scan* is case-sensitive.

## 6. What happens to references

Names are the only cross-reference in the system, so it is worth being explicit about what Duplicate does
**not** touch:

- **A duplicated flow** gets a new, unreferenced name. No sequence entry is re-pointed to it: duplicating
  a flow does not mean every sequence that used the original should now run the copy. The player points a
  sequence at the copy by hand.
- **A duplicated sequence** copies its entries verbatim, so the copy references the same flows as the
  original — `QuestSequenceEntry.Preset` is a flow *name* (AppConfig.cs:212-217) resolved at run time
  (QuestTool.cs:143-148), and nothing needs fixing up.
- **The copy becomes active**, like Create and Rename do (flow :6480, sequence :6672, Spammer :2083) — the
  player's decision (§7). For a flow and a sequence that is the config field itself, so neither can dangle.
  For a Spammer preset it is the tab's `current`, exactly as Create behaves: `Spammer.Active`, which a run
  reads, is written from the tab only by **Save Preset** (:2556), so the card keeps selecting the source
  until the player saves.
- **Rename is untouched.** It still *moves* the object (flow :6471-6473, sequence :6661-6665, Spammer
  :2095-2104). Only the prohibition on copies is retired, not the move.

## 7. Decisions already taken

| Decision | Consequence |
|---|---|
| Duplication is a normal feature — the "renaming MOVES, never copies" warning is retired | The **justification** goes, not the fact: USER_GUIDE.md:495 and USER_GUIDE.zh-TW.md:452 keep "Renaming moves the flow" (still true) and drop "two flows with the same steps is how the wrong one gets run". The code comment at MainWindow.xaml.cs:6469-6470 is rewritten the same way |
| Scope = the three Add/Rename/Delete pickers | Buy and Game Variants are out, with reasons (§2) |
| The copy becomes the active preset | Mirrors Create/Rename. A Quest flow and a sequence set the config field itself (`quest.Active` :6480, `quest.ActiveSequence` :6672); a Spammer preset sets the tab's `current` only, exactly as Create does — it becomes the preset you are editing, and reaches the card and the run at **Save Preset** (:2556, the tab's only writer of `Spammer.Active`). The plan deliberately does not add an earlier write, so one rule covers all three |
| **A Spammer copy carries `Presets` only** — no `Priority`, `Combos` or `Startup` | Exactly what Create does (`presets[name] = new Dictionary<string, double>()`, :2082), and safe: an absent entry resolves to an empty list, which the tool reads as "no ordering given" (`ActivePriority`/`ActiveCombos`, AppConfig.cs:1002, :1013, read at SkillSpammer.cs:59-60). The copy therefore opens with **no key order** — every key presses as soon as its cooldown is up, which is what the UI already calls "no order" (:2310) — **no combos and no opening**. Its key list is identical, so it looks the same in the picker and runs differently, and the hint must say so. The rename path is deliberately not the model here: it *moves* those three maps (:2102-2104) because leaving a live order behind would be the silent bug, and a fresh copy has none to leave |

## 8. Tests

The helper is testable; the buttons are not. `v2/SealTools.Tests/PresetCopyTests.cs` (new):

- `CopyName` returns `mine (copy)`, then `mine (copy 2)`, then `mine (copy 3)` as names are taken.
- `CopyName` on a source already ending in ` (copy)` yields `… (copy 2)` — the stem strip (§5) — and on one
  already at ` (copy <n>)` it continues from that number (`mine (copy 2)` → `mine (copy 3)`) instead of
  walking back to a free ` (copy)`.
- Independence: mutate the copy's `Steps` / `Entries` / keys and assert the source did not move. This is
  the aliasing guard, and it is the reason the helper exists.
- Round-trip persistence in the style of the existing tests, reusing `MakeTempConfigDirWithLocal`
  (ConfigLoaderTests.cs:809): a duplicated flow, sequence and Spammer preset survive `UpdateLocal` + load
  — beside `QuestFlowsRoundTripWithTheirSteps` (:1348), `QuestSequencesRoundTripWithTheirEntries` (:1415)
  and `UpdateLocalPersistsSpammerPresets` (:2176).

What cannot be unit-tested: the button's placement (the Spammer row is a Grid rather than a StackPanel
precisely because a horizontal row clipped Delete at 619 logical px — comment at :2164-2168), and that the
copy appears on the run cards. Those want the launcher run at the narrowest window, read rather than
eyeballed.

## 9. Docs and the gate

| File | Change |
|---|---|
| `USER_GUIDE.md` | Flow table (:494-497), sequence table (:504-509) and Spammer table (:246-248) each get a Duplicate row, and the warning at :495 is rewritten as §7 describes. The sequence table documents neither Rename nor Delete today even though the buttons exist (:6596) — add those rows while adding this one |
| `USER_GUIDE.zh-TW.md` | The twin: :452, :451-454, :462, :227-229 |
| `MainWindow.xaml.cs:6469-6470` | The comment, in the same commit as the behaviour |
| `plans/README.md` | The board row for this plan — the status lives there, never in this file |
| `records/PROGRESS.md` | One entry per build commit |

Not needed: `TOOL-FLOWS.md` and `CONFIG.md` describe run-time and config semantics rather than picker
buttons, and `DESIGN.md` §3 is untouched because Duplicate adds a dictionary **key**, not a field — the
loader's projection and its two-way pinning are unaffected. No firmware change, so `FirmwareSketchTests`
and `FirmwareVersionTests` are not in play.

The gate is unchanged: `dotnet restore/build v2/SealTools.sln` and `dotnet test`. `TreatWarningsAsErrors`
+ `AnalysisLevel=latest` (Directory.Build.props:6-8) means an unused private field or `using` in the new
file fails the build — fix the cause, never widen `NoWarn`.

## 10. Open questions for the player ⚑

1. **Case-sensitivity of the collision scan.** The preset dictionaries use the default (ordinal,
   case-sensitive) comparer (AppConfig.cs:134, :142, :975), the pickers sort `OrdinalIgnoreCase`
   (MainWindow.xaml.cs:6348, :6589, :2024), and the create/rename guards are case-sensitive
   (`ContainsKey`, :6461). A case-insensitive scan stops the *button* from producing two rows that look
   identical; it still lets a hand-typed `Mine (copy)` sit beside `mine (copy)`. Match the guards, or
   match what the player reads?
2. **The dangling-entry defect.** `MainWindow.xaml.cs:6878-6880` silently re-points a sequence entry whose
   flow name no longer exists to `flowNames[0]` on Save. Duplicate does not cause it, but it adds more
   flows, so it is easier to hit. Fix it in this change, or keep it a separate defect?
3. **Three buttons or one helper.** The three pickers are three hand-rolled copies of the same shape,
   and `investigations/ANALYSIS-UI.md` records a copy-paste complaint about the calibration canvas (5×)
   and the button factories, but says nothing about the pickers — so nothing in the repo obliges a shared
   helper here. Add a fourth copy-paste sibling, or extract the picker once and have all three (and
   whatever picker comes next) use it?

## 11. What a later pass could pick up

- **Buy.** It wants a rename row before it can take a Duplicate the way the other three do; the lighter
  alternative is a Duplicate that only prefills the Name box with `"<picked> (copy)"` and lets Save item
  write it.
- **Game Variants.** Duplicating a client (theme, coordinates, OCR bands) would be the same idea against
  `GameConfig.Variants` — but that collection is `OrdinalIgnoreCase` and edited in `defaults.yaml`, not in
  the UI, so it is a different change.
- **Onboarding templates.** The strongest form of this feature is not a copy of *one of yours* but a
  shipped starting point. The template mechanism built by PLAN-DEPLOY is the natural carrier.
