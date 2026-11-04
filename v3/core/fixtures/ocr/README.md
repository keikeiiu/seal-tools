# OCR text-level regression corpus (OCR-F11)

This folder is the input to `cargo run -p seal-core --bin ocr_eval -- --root <install-root>`.
It is intentionally **empty in git**: the cases are real game captures and their OCR text, and the
models cannot run in CI. Record cases on a machine with the models installed, then review the diff
before committing them.

## Layout

```text
fixtures/ocr/<read-path>/<case>.png
fixtures/ocr/<read-path>/<case>.expected.json
```

`<read-path>` names the reader under test (`lines`, `tuner-region`, `feeder-count`, `eta`,
`tooltip-panel`, `grade-box`). `<case>` is a stable machine-readable name.

`<case>.expected.json`:

```json
{
  "lines": ["DG", "attack speed +10", "25000"]
}
```

`lines` is the detector's full text in reading order and is what exact-match/CER compare against.
Mode-specific expected fields (`grade`, `remaining`, `attributes`, `cells`) can be added later;
the evaluator ignores unknown fields.

## Running

```powershell
$env:SEAL_OCR_MODELS = 'C:\path\to\models'
cd v3/core
.\cargo.cmd run -p seal-core --bin ocr_eval -- --root C:\path\to\install --corpus fixtures\ocr --baseline ..\..\.scratch\ocr-baseline.json
```

- exit 0: every case matched exactly;
- exit 1: at least one mismatch;
- exit 2: models/corpus could not load.

The JSON baseline carries per-case expected/actual text, exact-match, CER and latency, plus the
summary's exact rate, mean CER and p50/p95 latency. Keep the baseline with the release's evidence.
