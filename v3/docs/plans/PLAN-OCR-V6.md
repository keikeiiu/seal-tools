# PLAN-OCR-V6 — evaluate and ship PP-OCRv6

**Decision.** If OCR is updated, the target is **PP-OCRv6 small**, not PP-OCRv5. A measured dictionary
check already disqualified **tiny** for full Traditional Chinese: its recogniser has 6,904 entries and
lacks common Traditional characters such as 擊 and 預, while small has 18,708 entries and contains
them. v6 **medium** remains an accuracy reference only. No model is switched until the corpus gate
passes.

## Goal

Replace the current PP-OCRv4 models with a PP-OCRv6 det/rec set that reads Seal Online text and
attributes better — especially Traditional Chinese — without regressing latency, memory or the
existing OCR API.

## Architecture

The application does **not** call PaddleOCR's Python API. `seal-ocr` runs ONNX sessions through
`ort` and owns its own det/rec pre/post-processing. Only the model files, the model profile
(preprocessing constants, dictionary source, post-processing parameters) and the ONNX graphs change;
the `seal-ocr` API and every `seal-core` caller stay the same.

```text
seal-core tools / dispatch
        ↓  OcrPipeline::scan_rgba / read_line_rgba / modes::apply   (unchanged)
seal-ocr
  OcrPipeline → ModelProfile (v4 | v6-small; v6-tiny reference)
                 ├── det ONNX + detector profile
                 ├── cls ONNX (optional)
                 └── rec ONNX + dictionary source + rec profile
```

## Global constraints

- Windows-only; ONNX Runtime with the DirectML distribution already used by the release.
- Models are not committed; tests skip when `SEAL_OCR_MODELS` is unset.
- Public `seal-ocr` types/functions remain compatible; no protocol or tool changes.
- Thread counts stay pinned for corpus comparisons.
- The v4 models remain the fallback until v6 passes the acceptance gate.
- No model download in CI; release staging continues to copy an explicit `models/` directory.

## Review Focus

- **Traditional Chinese characters** that v4 currently repairs by hand must be read correctly.
- **Tiny game text** after the existing calibration crop/upscale path must not lose accuracy.
- **Tiny rec dictionary gap** — 6,904 entries, missing common Traditional characters; it must not be
  selected for the TW client even if its Latin/Simplified output looks good.
- **Detector behaviour** on dense attribute rows; v6's detector profile must not merge or split lines.
- **Latency** on the tool's real crops, not a vendor benchmark.
- **Model load failure** must be a named `ocr_unavailable`, never a silent fallback to another model.

---

## 1. What stays the same

| Public surface | Status |
|---|---|
| `OcrPipeline::load(models_dir, attributes, row_height)` | unchanged |
| `OcrPipeline::load_for_root(root, attributes, row_height)` | unchanged |
| `OcrPipeline::scan_rgba` / `scan_rgba_upscaled` / `read_line_rgba` | unchanged |
| `seal_ocr::pipeline::{RecognizedLine, Line, Scan}` | unchanged |
| `seal_ocr::modes::apply` / `read_grade` / `read_attributes` / `read_remaining` | unchanged |
| `seal-core` dispatch, tuner, pet callers | unchanged |
| Protocol fixtures | unchanged |

## 2. What is different (model/API layer)

These are internal `seal-ocr` changes, not caller API changes.

### 2.1 Model files

PaddleOCR v6 names its models per tier, for example:

```text
PP-OCRv6_tiny_det_infer.onnx
PP-OCRv6_tiny_rec_infer.onnx
PP-OCRv6_small_det_infer.onnx
PP-OCRv6_small_rec_infer.onnx
PP-OCRv6_medium_det_infer.onnx
PP-OCRv6_medium_rec_infer.onnx
```

Current v4 names are `ch_PP-OCRv4_det_infer.onnx`, `ch_PP-OCRv4_rec_infer.onnx` and
`ch_ppocr_mobile_v2.0_cls_infer.onnx`. The model set must be selected by a profile, not by guessing
filenames.

### 2.2 Dictionary source

Current `RecModel::load` reads the dictionary only from the ONNX model's
`metadata_props["character"]` and fails with `MissingDictionary` when absent. v5/v6 ONNX exports often
ship the dictionary as a **separate file** (or in a different metadata key).

Required loader change:

- read embedded `character` metadata when present;
- otherwise read the profile's external dictionary file from `models/` (e.g. `ppocrv6_dict.txt`);
- still fail loudly, naming every path/key tried, when neither exists.

### 2.3 Recognition class list

The v4 dictionary has 6,623 entries → 6,625 class slots after the C# `"blank"` prefix and `" "`
suffix. v6's multilingual dictionary is much larger (v5 is ~18k). `decode_ctc` is generic over the
class list, but the blank/space decoration and the class axis must be confirmed against the v6 rec
model rather than assumed.

### 2.4 Detector profile

Current detector constants are PP-OCRv4-era (`box_thresh` 0.6, `unclip_ratio` 1.5, mean/std
0.485/0.456/0.406). v6 uses a different detector architecture (`RepLKFPN`, `DiceBCE`); the output is
still a probability map, but thresholds, unclip and mask geometry must be measured. Add a
`DetectorProfile` rather than editing the v4 constants in place.

### 2.5 Classifier

v6's pipeline may not require the v4 textline classifier. Keep the cls stage optional per profile and
measure whether it improves the game's short single-line crops; do not assume either way.

### 2.6 PaddleOCR Python API

Not a runtime dependency. It is used only to download/export the ONNX models. The v6 pipeline API and
config names differ from v4/v5; record the exact export command and model hashes in the corpus
README, but do not port Python API code into Rust.

## Trial status (2026-11-10)

The trial path is now implemented, not a proposal only:

- Official PaddlePaddle HF ONNX exports are used; local conversion is unnecessary.
- `OcrModelVersion`/`DetectorProfile` and `OcrPipeline::load_version` exist; v4 stays the default.
- `RecModel::load_with_dictionary` + `parse_dictionary_text` load the v6 sidecar without trimming
  spaces/U+3000.
- `SEAL_OCR_MODEL_VERSION=v6-small` selects v6 for the app path; `ocr_eval --model-version v6-small`
  selects it for corpus runs. An invalid value fails loudly.
- Setup now exposes `ocr.model_version` (`v4` / `v6-small`), persisted in `setup/common.yaml`; the core
  rebuilds the pipeline when it changes and writes `logs/ocr_model.log`.
- One tuner-panel fixture run: v4 exact 0/1, CER 0.2105, 679 ms; v6 exact 0/1, CER 0.2895, 806 ms.
  Too little data for a verdict — the live corpus and A/B gate remain required.

## 3. Tasks

### Task 0 — export and pin the candidate models

**Files:** `v3/docs/records/OCR-V6-MODELS.md` (new, evidence), no code.

- Download v6 small det+rec (and cls if used); export to ONNX with the pinned PaddleOCR release.
  Tiny rec is kept as a measured reference only (see `../investigations/OCR-V6-MODELS.md`).
- Record SHA-256, byte sizes, input/output shapes, opset, metadata keys and dictionary source.
- Verify each graph loads through `ort` and runs one synthetic tensor on this machine/DirectML.
- Record the license/provenance for the models and the export tool.

**Gate:** all candidate files load and run; unsupported ops or missing ONNX exports stop the plan.

### Task 1 — live labelled corpus

**Files:** `v3/core/fixtures/ocr/` (new cases), `v3/core/fixtures/ocr/README.md` update.

- Record live crops for `tuner-region`, `lines`, `grade-box`, `feeder-count`, `eta`,
  `tooltip-panel`; 10–20 cases per path.
- Hand-correct expected text; include Traditional characters and known v4 failures.
- Run the v4 pipeline through `ocr_eval` to record the baseline.

**Gate:** corpus covers every live read path; baseline v4 has numbers before any v6 comparison.

### Task 2 — model profile and dictionary loader

**Files:**

- Create: `v3/core/crates/seal-ocr/src/profile.rs`
- Modify: `v3/core/crates/seal-ocr/src/model.rs`
- Modify: `v3/core/crates/seal-ocr/src/pipeline.rs`
- Test: `v3/core/crates/seal-ocr/tests/models_resolution.rs`, unit tests in `model.rs`

Interfaces:

```rust
pub enum OcrModelVersion { V4, V6Tiny, V6Small, V6Medium }

pub struct OcrModelProfile {
    pub version: OcrModelVersion,
    pub detector: DetectorProfile,
    pub recognizer: RecognizerProfile,
}

pub fn resolve_profile(models_dir: &Path, version: OcrModelVersion)
    -> Result<OcrModelProfile, OcrError>;
```

- Resolve filenames and the dictionary source per profile.
- Add `RecModel::load_with_profile` while keeping `load` as the v4 path.
- Tests: v4 profile still resolves; missing v6 file/dictionary names the path; external dictionary
  decodes; `MissingDictionary` only when both sources are absent.

### Task 3 — pipeline wiring

**Files:**

- Modify: `v3/core/crates/seal-ocr/src/pipeline.rs`
- Modify: `v3/core/crates/seal-core/src/dispatch.rs` (model selection only)
- Modify: `v3/core/crates/seal-config/src/registry.rs` (OCR model version setting if exposed)
- Test: pipeline tests with fake models and real-model skip

- `OcrPipeline::load_for_root` reads the configured OCR version (default v4 during evaluation).
- Preprocessing and detector post-processing take the profile values; v4 constants remain the
  default so the current release is unchanged.
- Missing v6 models fail with `ocr_unavailable` naming the expected files.
- No caller signature changes.

### Task 4 — A/B evaluation

**Files:** `v3/docs/investigations/OCR-V6-EVAL.md` (new, evidence).

Compare on the same corpus:

| Candidate | Det | Rec |
|---|---|---|
| v4 (current) | `ch_PP-OCRv4_det` | `ch_PP-OCRv4_rec` |
| v6 tiny | reference only | 6,904-entry dict; missing 擊/預 |
| v6 small | `PP-OCRv6_small_det` | `PP-OCRv6_small_rec` |
| v6 medium | reference only | reference only |

Report exact-match rate, CER, attribute accuracy (matcher), Traditional-character failures, p50/p95
latency, peak memory and model bytes.

**Acceptance gate:**

- v6 small improves overall OCR and Traditional Chinese against v4;
- no read path regresses more than 1 percentage point;
- p95 latency stays within the existing live budget;
- model set size and memory are acceptable for the release zip;
- all current tests and fixture gates pass.

If v6 does not clear the gate, keep v4; do not ship v5 as a consolation.

### Task 5 — ship or stop

If accepted:

- stage v6 models in the release `models/` directory;
- update `OCR-RESEARCH.md`/README with hashes and the pinned export version;
- update `CONFIG.md`/`PROTOCOL.md` if the model-version setting is exposed;
- add the release note and a PROGRESS entry;
- re-run the corpus baseline and record it with the release evidence.

If rejected: record the measurements and leave v4 in place.

## 4. Test plan

| Area | Test |
|---|---|
| Model resolution | v4, v6 tiny/small paths; missing file/ dictionary; external dict |
| Dictionary | embedded vs external; class count; blank/space decoration |
| Detector | profile thresholds; score map shape; no merged rows on corpus |
| Recognizer | CTC decode with v6 class count; confidence |
| Pipeline | `scan_rgba` unchanged; `read_line_rgba` unchanged; modes unchanged |
| Failure | `ocr_unavailable` names every expected model path; no fallback |
| Regression | existing OCR tests, fixtures and `ocr_eval` baseline |
| Performance | p50/p95 latency, model load time, memory |

## 5. Risks

| Risk | Mitigation / stop condition |
|---|---|
| v6 has no usable ONNX export | Task 0 stop; stay on v4 |
| DirectML unsupported ops | CPU fallback measurement; if CPU too slow, stop |
| External dictionary differs | loader fallback + hash pinning; fail loudly |
| Detector post-processing differs | profile values measured on corpus, never guessed |
| Accuracy improves only on vendor data | corpus gate decides; no ship on benchmark claims |
| Model size/latency grows too much | small only; tiny/medium reference |
| Licence/provenance unclear | record before staging; stop if not redistributable |

## 6. Open questions

1. Ship **small** as the default if it passes; tiny stays reference-only because of its dictionary gap?
2. Is a model-version setting player-facing, or a release-time choice only?
3. Should the v4 models remain in the zip as a fallback, or is one pinned set enough?
4. What is the maximum acceptable model-set size/latency for the release?
5. Is the v6 dictionary file redistributable under the same terms as the models?

## 7. References

- [PP-OCRv6 introduction](https://www.paddleocr.ai/main/version3.x/algorithm/PP-OCRv6/PP-OCRv6.html)
- [PP-OCRv6 text recognition models](https://www.paddleocr.ai/main/version3.x/module_usage/text_recognition.html)
- [OCR research record](../records/OCR-RESEARCH.md)
- [OCR v6 model study](../investigations/OCR-V6-MODELS.md)
- [OCR evaluation corpus](../../core/fixtures/ocr/README.md)
- Current API: `v3/core/crates/seal-ocr/src/{pipeline,model,det}.rs`
