# Plan — the US client (game variants)

Written 2026-09-28.

**BUILT (2026-09-28): §4.1, §4.2 and §4.5** — `game.variant` + `game.variants`, resolved at load into
`window.title`; the variant's dictionary file; `attributes.us.yaml.example`; and the published-zip copy
of that example. Four tests, one of which was **proven to bite** by dropping `game` from `SaveDefaults`
and watching it fail (`Expected: "US_LIVE" / Actual: "TW_LIVE"`).

**NOT built: §4.3 (the Setup-tab switch — changing clients is still a file edit) and §4.4 (moving the
two phrase sets into the dictionary — so the per-level and time-line phrases are still TW-only).**

And the honest headline is at the end: the code makes a second client *possible*, and only the US client
itself can make it *work*.

The goal: run this suite against the **US client** as well as the TW one, chosen by a setting rather than
by editing files or rebuilding.

---

## 1. What is actually version-specific

The window target is one value. **The text is the work.** Checked by reading every matcher rather than
assuming — and the list is much shorter than it looks:

| | what | why the US client breaks it |
|---|---|---|
| 1 | **`window.title`** (`TW_LIVE`) | already config; `FindByTitle` is a **substring** match |
| 2 | **`attributes.yaml`** — names, OCR variants, `text_fixes` | the dictionary is Chinese **content**. **This is the bulk of the work** |
| 3 | **`AttrMatcher`** — `每\s*(\d+)\s*[等級级]` + a `contains '每'` gate | the Tuner's per-level filter |
| 4 | **`FeederEta`** — `約 N 分`, `完`/`完成`, `所需時間` | the feeder's time line reads nothing → falls back to the food arithmetic |
| 5 | **`tuner.filter.rules[].name`** | the rules in `defaults.yaml` name dictionary entries, so they are per-variant too |
| 6 | **`pet-data.csv`** species names | the dropdown's keys; a US player would pick Chinese-named lines |

**And what is already version-independent** — most of the suite, which is why this is not a rewrite:

- **`PetPanel` parses digits and brackets only** — `（N）` for the stage, `+N`, `[N%]`. No words. The
  hover-panel read works on an English client unchanged.
- **Grade letters** (`N`/`G`/`DG`/`XG`/`SG`) are latin.
- **`TextCleaner`** has no vocabulary of its own; it is driven entirely by the `text_fixes` table.
- **Gem Composer, Buy/Sell, Spammer** match no text — position, colour and keys.
- **Quest Hand-in reads nothing at all**, so it works on the US client as shipped.

## 2. The design

### 2.1 The variant is an explicit switch, resolved at LOAD

```yaml
game:
  variant: tw
  variants:
    tw: { title: TW_LIVE, attributes: attributes.yaml }
    us: { title: US_LIVE, attributes: attributes.us.yaml }
```

**`ConfigLoader` resolves the variant while loading** — it sets `Window.Title` from the variant and
remembers which dictionary file to read. That is the whole trick, and it is why the change is small:
**`Window.Title` stays the single value every tool already reads**, so *none* of the ~15 call sites in
`OcrEngine`, `GemComposer`, `HidPointer` and `MainWindow` change.

**Not a `titles: [TW_LIVE, US_LIVE]` list**, which was the alternative. That would find whichever client
is up without a setting — but it needs a `Find(window)` helper in all fifteen places, and more
importantly it cannot answer the question that actually matters: *which text does the tool match against?*
The title and the dictionary have to be chosen together, and a list answers only the first half.

### 2.2 Geometry is shared — probably, and testably

The calibration is **client-relative physical pixels**, and [COORDINATES.md](COORDINATES.md) records that
the game's UI **does not scale with the window** ("the HP bar keeps its pixel size, the world view
expands"). So the UI is drawn at fixed pixel sizes anchored inside the client area. **If the US client is
the same build with translated text, the same client size gives the same coordinates.**

**The risk, stated plainly:** translation changes text *length*, and any label or control sized to its
text would move. A fixed-layout UI would not care; a content-sized one would.

**So it is one look, not an argument.** The calibrate tabs draw the saved marks on a capture. Switch to
US, open Calibrate Tuner, capture, and see whether the boxes land on the elements. If they do, one
calibration serves both. If they do not, the *manner* of the miss decides what to build: a uniform shift
is a small fix, a re-layout means a calibration per variant — and that is deliberately **not** in this
plan until the boxes say it is needed.

The **Setup** tab already warns when the live client size differs from the stored one, so a size mismatch
announces itself rather than being discovered later.

### 2.3 The phrase sets become dictionary entries

Items 3 and 4 are the only two **code** items on the list, and the dictionary file is already the right
home: `TextFixesConfig` carries `Whole` / `Substring` / `SimplifiedTraditional` / `Final` / **`Regex`**,
and `AttrMatcher` already receives the whole `AttributesConfig`. So:

- the **per-level interval** phrases move into the dictionary beside the other fixes;
- **`FeederEta`'s** phrases move the same way — it is a static class today, so its reader takes the phrase
  set as an argument instead of compiling them in.

That makes a variant **one file** for everything text: names, variants, fixes, and the phrases the code
matches on. Swapping clients swaps a file, and the two cannot drift.

## 3. The OCR reality

Two separate questions, and only one of them is work.

**The recogniser model is a measurement.** The shipped models are **Chinese** (`ch_PP-OCRv4_rec_infer`).
PP-OCR's `ch` recogniser includes Latin and digits, so it *will* read English — whether it reads English
**attribute names** accurately enough for an exact dictionary match is unmeasured. The good news is that
the model paths are already config (`tuner.models.recognizer`), so swapping in an English model is a
config line plus shipping the file — **not a code change**, and in the right place to discover.

**The dictionary content is the real work, and it is grown rather than written.** The TW dictionary's
value is not its list of names but its **variants table**, built from real misreads over many sessions
(`每`→国/盘/地, `幸運`→幸莲, `必殺技`→必毅技). English OCR fails *completely differently* — `l`/`I`/`1`,
`O`/`0`, `rn`/`m` — so none of it transfers, and the English table starts empty.

**That makes US support a process rather than a one-off**, and the machinery to run that process already
exists: the tuner writes `ocr_log.jsonl` and `logs/reads/`, and the **Attributes** tab shows the
dictionary. That is exactly how the TW table was built.

## 4. What gets built, in order

1. **`GameConfig`** — `variant` + per-variant `title` and `attributes`, resolved at load into
   `Window.Title` + the dictionary filename.
   - goes in **`defaults.yaml`**, and therefore into **`SaveDefaults`' explicit field list** — a field
     missing from that list is written once and dropped by the next save from any tab. Pinned by
     extending `SaveDefaultsPreservesEveryPortableField`.
2. **The switch itself** — `LauncherService.SwitchVariant`, which re-resolves the title, re-loads the
   dictionary, and persists. `Attributes` is loaded once at construction today and is get-only, so it
   becomes replaceable; the tools receive it per run, so the next Start picks it up.
3. **A control on the Setup tab** — it already owns the environment, and the switch is part of "what am I
   pointed at".
4. **The phrase sets moved into the dictionary** (§2.3).
5. **An empty `attributes.us.yaml`** with the structure and a comment saying how it gets filled — so the
   US side has somewhere to grow before any content exists.

## 5. What is blocked on the US client

Nothing in §4. Everything in §1's items 2, 5 and 6:

- the **English attribute dictionary** — names, categories, OCR variants, fixes. Cannot be written here;
  the repo's own rule is that game text is never matched from a scraped source, and for a dictionary it
  must come from the client;
- the **English filter rules**, which name dictionary entries;
- the **English pet table** names, if the species list is to mean anything to a US player;
- the **model answer** in §3 — one capture away once the client is open.

## 6. Not in scope

- **A per-variant calibration**, unless §2.2's test says the geometry differs.
- **Running both clients at once.** One target, one dictionary, chosen by the setting.
- **Translating the launcher's own UI.** It is English already.

## 7. Risks

- **The variant resolution is a load-time rewrite of `Window.Title`.** Anything that reads
  `window.title` *before* the variant is applied would see the raw value — the resolution belongs where
  the config is deserialised, not in a later fixup, and that placement wants a test.
- **`game.variant` in `defaults.yaml`** inherits that file's trap: it is the file `publish.bat` ships, so
  a public zip would carry the *builder's* client choice. The default must stay `tw`.
- **A dictionary file that does not exist** must fail loudly or fall back — a missing
  `attributes.us.yaml` should say so on the card, not silently match nothing. That is the same
  silent-empty failure the attribute tab already reports.
