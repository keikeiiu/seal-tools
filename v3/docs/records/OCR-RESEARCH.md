# OCR port research — models, runtime, pipeline constants, parity corpus

**Status: research complete (2026-10-25). No code changed.** This answers what the `seal-ocr` port
needs to know before it is planned: where the model weights and the character dictionary actually
are, which runtime to bind, the exact preprocessing/post-processing constants v2 ran, and what the
recorded corpus can and cannot prove. Every fact below was measured on this checkout or the installed
NuGet/PyPI packages; source links are to the upstream repository at the commit the NuGet package
records.

Method: `Get-FileHash`/sizes on the model files and packages, .NET reflection over the
`RapidOCRSharpOnnx.dll` manifest resources, byte-parsing the rec ONNX `metadata_props`, Python JSON
parsing of `v2/logs/ocr_log.jsonl`, N=3,366 PNG headers read directly, and reading the v2 sources and
the upstream C# sources. Reproduce commands are listed in §6.

---

## 1. Where the models and dictionary are

### 1.1 Model files

They live in `v2/models/` (**untracked** — `v2/.gitignore:16-17`, "Bundled OCR models (large binaries,
copied from rapidocr-onnxruntime at setup)"). They are byte-identical to the models bundled with the
PyPI package `rapidocr-onnxruntime 1.4.4` (Apache-2.0, `RapidAI/RapidOCR`,
`%LOCALAPPDATA%\Programs\Python\Python312\Lib\site-packages\rapidocr_onnxruntime\models\`) and are
shipped inside the release zip (`v2/dist/SealTools-v2.10.zip` carries `models\*.onnx` with exactly
these sizes).

| file (v2/models/) | bytes | SHA-256 | source |
|---|---:|---|---|
| `ch_PP-OCRv4_det_infer.onnx` | 4,745,517 | `D2A7720D45A54257208B1E13E36A8479894CB74155A5EFE29462512D42F49DA9` | rapidocr-onnxruntime 1.4.4, identical hash |
| `ch_PP-OCRv4_rec_infer.onnx` | 10,857,958 | `48FC40F24F6D2A207A2B1091D3437EB3CC3EB6B676DC3EF9C37384005483683B` | same |
| `ch_ppocr_mobile_v2.0_cls_infer.onnx` | 585,532 | `E47ACEDF663230F8863FF1AB0E64DD2D82B838FCEB5957146DAB185A89D6215C` | same |
| **total** | **16,189,007** | | matches the release payload's `models/` figure in `PLAN-V3-CORE.md:36-37` |

v2 loads them from config paths, not from the executable: `v2/config/defaults.yaml:70-73` sets
`tuner.models.detector/recognizer/classifier` to `models/*.onnx`, resolved against the root by
`OcrEngine.Resolve` (`v2/SealTools.Core/OcrEngine.cs:76-78`: rooted path wins, otherwise relative to
the root), and the engine is built lazily on the first read (`InitOcr`, `OcrEngine.cs:66-76`). v2's
README states the same models as v1 (`v2/README.md:71`), i.e. PaddleOCR PP-OCRv4 converted to ONNX.

**Licence/provenance:** `rapidocr-onnxruntime 1.4.4` declares `License: Apache-2.0` (dist-info
METADATA); the PP-OCRv4 models are PaddleOCR models redistributed by the RapidOCR project
(<https://github.com/RapidAI/RapidOCR>). Shipping the three files in a v3 payload needs an attribution
NOTICE and a confirmed redistribution grant (the RapidOCR repo's Apache-2.0 declaration is the
evidence; it should be checked in the release commit, not assumed from this file).

### 1.2 The character dictionary

**Not in `models/`, and not needed from the DLL for v2's PP-OCRv4 configuration.** The recogniser's
class list is read from the REC model's own ONNX metadata: `TextRecognizerBase` constructor calls
`GetCharacterList()`, which first checks `_session.ModelMetadata.CustomMetadataMap["character"]`
(upstream `RapidOCRSharpOnnx/Inference/PPOCR-Rec/TextRecognizerBase.cs`). The two dictionaries embedded
in the DLL (`ppocrv6_dict.txt` 93,655 B, `ppocrv6_tiny_dict.txt` 34,060 B) are only fallbacks for
`OCRVersion.PPOCRV6`/`PPOCRV6Tiny` and are **unused by v2**.

Measured from `ch_PP-OCRv4_rec_infer.onnx` (ModelProto `metadata_props`, key `character`):

| property | value |
|---|---:|
| dictionary bytes (UTF-8, newline-separated) | 26,249 |
| dictionary entries | 6,623 |
| SHA-256 of the dictionary text | `28B2362AD4AB2DC38769AA72FEB535E3A9DDB3FD2A7585A05920E6393B1DC7F7` |
| class slots after the library's decorations | 6,625 (`"blank"` inserted at index 0, `" "` appended — `TextRecognizerBase` ctor) |

The extraction step for the port is therefore: parse `metadata_props["character"]` from the rec ONNX
(any ONNX metadata reader will do; no DLL reflection), split on `\n`, then prepend `blank` and append
space exactly as the C# does. **This corrects `PLAN-V3-CORE.md:273-280`** ("the recognizer's character
dictionary is not in `models/` … extracting it is a step-5 precondition"): it is in the rec model's
metadata, and the 18.9 MB DLL does not have to be unpacked.

### 1.3 The DLL (for completeness only — the port should not need it)

`RapidOCRSharpOnnx 1.3.0` (`v2/SealTools.Core/SealTools.Core.csproj:12`), nuspec: Apache-2.0,
`github.com/meloht/RapidOCRSharpOnnx`, commit `af4d3638f99d6c896b3b7dd084bf94d3728e5b0`; managed
runtime dependency `Microsoft.ML.OnnxRuntime.Managed >= 1.28` while v2 also references
`Microsoft.ML.OnnxRuntime 1.29.0` directly (`SealTools.Core.csproj:10`), and the shipped native
`onnxruntime.dll` is **16,149,344 B** with `onnxruntime_providers_shared.dll` 21,856 B (measured in
the v2.10 release zip and in the 1.29.0 NuGet win-x64 runtime folder).

- `lib/net8.0/RapidOCRSharpOnnx.dll`: 18,914,304 B, SHA-256 `41FD3F2B063E0307566729076639C0F3CA88B5AE0A45847CA5CF473587BC46B8`.
- 15 embedded resources: the two PP-OCRv6 dictionaries above + 13 fonts (drawing debug output only,
  0.18–7.4 MB each: fonts for latin/arabic/cyrillic/devanagari/el/japan/kannada/korean/tamil/telugu/th,
  chinese_cht.ttf and FZYTK.ttf). The fonts and SkiaSharp/OpenCVSharp drawing are **not** part of the
  read path v2 relies on (`ReturnWordBox = true` returns words + boxes + scores).

---

## 2. The runtime choice

### 2.1 Candidates

| | `ort` (ONNX Runtime bindings) | `tract-onnx` (pure Rust) | notes on others |
|---|---|---|---|
| Windows delivery | binds Microsoft's ONNX Runtime C API; `download-binaries` + `copy-dylibs` copy `onnxruntime.dll` next to the exe at build | statically linked pure Rust; no DLL | `candle`/`burn` have no general ONNX graph import suitable for PP-OCR; `wonxx` is WebGPU-first; no crate named `tune` provides an ONNX runtime (checked) |
| Zip size | +16,149,344 B `onnxruntime.dll` (+21,856 B providers shim), already in v2's payload list | no DLL; binary grows by the compiled op coverage — **measure in the spike, do not guess** | — |
| Startup | session load of 3 models (16.2 MB total weights); v2 pays it lazily on first scan (`OcrEngine.cs:66-76`) | graph parse/optimise on first load; no runtime DLL init | — |
| CPU determinism | deterministic for a pinned version, model and thread config; **version changes may change kernels** (v2 ran ORT 1.29; current ort targets 1.31) | self-consistent for a pinned crate; **not bit-identical to ORT** — different kernels/fusions, so the v2-recorded confidences must be re-baselined | parity corpus is the arbiter either way |
| Licence (crate) | `MIT OR Apache-2.0` | `MIT OR Apache-2.0` | ONNX Runtime itself MIT; models Apache-2.0 |
| Latest version (crates.io, checked 2026-10-25) | `2.0.0-rc.15` (2026-10-19; ORT 1.31, rust 1.92; defaults include `download-binaries`, `copy-dylibs`, `tls-native`) | `0.23.9` (2026-10-15; rust 1.91, edition 2024); `0.21.19`/`0.22.5` also live | — |

### 2.2 Recommendation

**Use `ort`, pinned exactly: `ort = "=2.0.0-rc.15"`** (fallback pin `=2.0.0-rc.13`, rust 1.88, if the
toolchain must be older), CPU execution provider only, default features (which provide
`download-binaries` + `copy-dylibs` — the DLL is copied beside `seal-core.exe` and travels in the
release zip; `load-dynamic` is an alternative if the DLL path must be controlled at runtime).

Reasons: (a) it is the same runtime family v2 shipped, so the parity corpus — which is a record of
ORT's own outputs — is the closest thing to an oracle; (b) the 16 MB DLL is already part of v2's
published payload, so it adds no distribution objection (unlike OpenCVSharp, which v3 does not need);
(c) it is a Windows-first, widely deployed binding with a bounded API.

`tract` is the fallback only if shipping a native DLL becomes unacceptable. It would remove the DLL but
inherit three new risks: PP-OCRv4 op coverage must be proven (the rec model is CRNN-style with LSTM),
CPU speed on three models per scan is likely worse, and every recorded confidence must be re-baselined
because tract's arithmetic is not ORT's.

Determinism policy for parity (whatever runtime): run CPU with `intra_op_num_threads = 1` and
`inter_op_num_threads = 1` in the parity runner (v2's Python origin leaves them at `-1`, but the C#
library builds its own `SessionOptions`; the runner must pin them), record the runtime version and the
model hashes in the test output, require **exact text equality** and allow confidence deltas (proposal
±1e-3; revisit after the first runtime spike) because ORT 1.29 → 1.31 can move the last decimals.

---

## 3. The pipeline to reproduce

v2 runs **RapidOCRSharpOnnx 1.3.0's** pipeline with `OCRVersion.PPOCRV4` and
`ReturnWordBox = true` (`OcrEngine.cs:66-76`), then does its own line/grade/attribute reconstruction.
The constants must be taken from **that C# library**, not from the Python `rapidocr_onnxruntime`
config: the two differ materially (see the callout). Unless marked v2, citations below are files in
the upstream repo at the nuspec commit.

### 3.1 Detector

| constant | value | citation |
|---|---|---|
| model input | RGB, CHW float, size = resized image (1,3,H,W) | `Inference/PPOCR-Det/DetPreprocess.cs`, `PreprocessImage` |
| pre-resize bounds | max side 2,000, min side 30; sizes rounded to multiples of 32 | `DetPreprocess.cs` `ResizeImageWithinBounds`/`ReduceMaxSide`/`IncreaseMinSide` |
| vertical padding | pad to height `2*max(w/8, 30)` when `h<=30` or `w/h>8`; black | `DetPreprocess.cs` `ApplyVerticalPadding`/`GetPaddingH` |
| side limit | `limit_side_len = 736`, `limit_type = min` for PP-OCRv4 (config default; PPOCRv4 default set does NOT change it) | `Configurations/DetectorConfig.cs:14-15`; `PPOCRv4DefaultConfig.cs` |
| normalisation | `(pixel/255 - mean)/std`, CHW, RGB | `DetPreprocess.cs` `ToCHW_RGB_NormalizedV4` |
| mean / std (PP-OCRv4) | `[0.485, 0.456, 0.406]` / `[0.229, 0.224, 0.225]` | `PPOCRv4DefaultConfig.cs:11-12` |
| binary threshold | 0.3 on the probability map | `DetectorConfig.Thresh`; `PPOCRv4DefaultConfig.cs:14`; `DetPostprocess.cs` `DBPostProcess` |
| dilation | 2×2 rectangle before contours | `DetPostprocess.cs` `DBPostProcess` |
| min box side | 3 px (contour min side, and after unclip `+2`) | `DetPostprocess.cs` `_minSize` |
| box score | mean probability over the min-area-rect mask; reject `< 0.6` | `DetectorConfig.BoxThresh = 0.6`; `PPOCRv4DefaultConfig.BoxThresh = 0.6` |
| unclip ratio | **1.5** (PP-OCRv4 default config), round-join inflate via Clipper | `PPOCRv4DefaultConfig.cs:18`; `DetPostprocess.cs` `UnclipBox` |
| max candidates | 1,000 (PP-OCRv4 set) | `PPOCRv4DefaultConfig.cs:17` |
| post-filter | clip to image; drop w or h ≤ 3 | `DetPostprocess.cs` `FilterBoxes` |
| ordering | sort by first-point Y; row id = previous + (dy ≥ 10 ? 1 : 0); then row id, then X | `DetPostprocess.cs` `_BOX_SORT_Y_THRESHOLD = 10`, `SortedBoxes` |
| crop | perspective warp to the box (cubic, replicate border); rotate 90° CW when `h/w ≥ 1.5` (vertical text) | `DetPostprocess.cs` `GetRotateCropImage` |

### 3.2 Orientation classifier (optional in the library, **enabled by v2**)

| constant | value | citation |
|---|---|---|
| model enabled | v2 passes `Resolve(m.Classifier)` | `OcrEngine.cs:66-73` |
| input shape (v4) | `[3, 48, 192]` | `Inference/PPOCR-Cls/ClsPreprocess.cs:32` |
| resize | height 48, keep ratio, width ≤ 192, right-pad zeros | `ClsPreprocess.cs` `ResizeNormImg`/`GetResizedW`; `Inference/PreprocessBatchCore.cs` `ConvertToNormImg` |
| normalisation | `pixel * 2/255 - 1` per channel, CHW, RGB | `PreprocessBatchCore.cs` `ConvertToNormImg` |
| decision | argmax over 2 classes, labels `["0","180"]`; if `180` and score > 0.9 → rotate crop 180° | `Configurations/ClassifierConfig.cs:9-10`; `Inference/PPOCR-Cls/ClsPostprocess.cs` |
| text score gate | 0.5 for blocks (v2 uses `WordResults`, see §3.4) | `Configurations/RecognizerConfig.TextScore = 0.5` |

### 3.3 Recogniser

| constant | value | citation |
|---|---|---|
| input shape | `[3, 48, 320]` target | `Configurations/RecognizerConfig.cs:8` |
| batch width rule | per batch: `max_wh_ratio = max(320/48, each crop w/h)`; tensor width = `round(48 * max_wh_ratio)` | `Inference/PPOCR-Rec/RecPreprocess.cs` |
| resize | height 48, keep ratio, `resized_w = min(ceil(48*w/h), img_width)`; right-pad zeros to img_width | `RecPreprocess.cs` `ResizeNormImg`; `PreprocessBatchCore.cs` |
| normalisation | `pixel * 2/255 - 1`, CHW, RGB; padding exactly 0.0 | `PreprocessBatchCore.cs` `ConvertToNormImg` |
| batch size | 6 | `RecognizerConfig.cs:9` |
| decode | per timestep argmax; skip index 0 (CTC blank) and repeats; map remaining indices to the class list | `RecPostprocess.cs` `GetSelection`/`GetCharList`/`getIgnoredTokens` |
| class list | `["blank"]` + rec ONNX metadata `character` split `\n` + `[" "]` | `TextRecognizerBase.cs` (ctor + `GetCharacterList`) |
| score per line | mean of the selected timestep maxima, rounded to 5 decimals | `RecPostprocess.cs` `BuildRecResult` |

### 3.4 What v2 does around/after the library

| step | v2 behaviour | citation |
|---|---|---|
| engine init | lazy on first read; `OcrConfig(det, rec, LangRec.CH, PPOCRV4, cls) { ReturnWordBox = true }` | `OcrEngine.cs:66-76` |
| retries | `retries = max(1, tuner.ocr_retries)`; rescan until `IsConfirmed` (grade + 3 non-empty attribute rows + remaining); final attempt forces a capture | `OcrEngine.cs:84-100`, `389-395` |
| region capture | raw region capture per scan, physical pixels; no upscale in the scan path | `OcrEngine.cs:227-263` |
| `ReadLines*` upscale | default **3×**, cubic; debug crop written at the upscaled size | `OcrEngine.cs:119-121`, `140-142`, `164-166`, `179-198` |
| word boxes | `ocrResult.WordResults` (per detected word: text, box in original coords, score) | `OcrEngine.cs:203-204`, `262-263` |
| row grouping | bucket `y / rowHeight`, sort by min X, concatenate words, line conf = mean of item scores; `rowHeight` used is `max(8, ocr.row_height)` | `OcrEngine.cs:218`, `546-569`; `BuildLines` |
| X/Y filtering | item kept when its Y is in the band and its X range overlaps (X band optional for old configs) | `OcrEngine.cs:571-585` `FilterItems` |
| cleaning | `TextCleaner` with `text_fixes` from `attributes.yaml` | `OcrEngine.cs:214`, `284`, `303` |
| grade line | Y-band items, `conf > 0.1`, length > 1, text ≠ "300", deduplicated | `OcrEngine.cs:280-288` |
| grade letter | cut the line at the last `：`/`:` (or strip a leading `GRADE`/`等級`), take the rightmost longest known grade token; colour fallback always computed and used when the letter is absent | `OcrEngine.cs:290-297`, `438-470`, `469-543` |
| attributes | X+Y-filtered rows bucketed by `rowHeight`; skip `conf < 0.3` or length < 2; label/value split by CJK/sign/digits; first 3 rows with Y ≥ 10; `FixSign` with negative markers | `OcrEngine.cs:299-331`, `415-427` |
| remaining | X+Y-filtered lines, `conf > 0.4`, first integer matching `\d+` in 1..=99,999 | `OcrEngine.cs:333-345` |
| logging | every scan to `logs/ocr_log.jsonl` (record shape below); persistently unconfirmed scans + capture to `logs/ocr_errors.jsonl` | `OcrEngine.cs:347-413` |
| capture policy | `tuner.save_captures` or the final forced attempt; the library is asked to save a **drawn debug image**, and the forced attempt additionally saves the whole client with the region outlined | `OcrEngine.cs:236-260` |

> **Constants divergence, load-bearing.** The Python `rapidocr_onnxruntime 1.4.4` `config.yaml` that
> the models came from uses det `mean/std = [0.5,0.5,0.5]`, `box_thresh 0.5`, `unclip_ratio 1.6`; the
> C# library v2 actually ran uses `[0.485,0.456,0.406]/[0.229,0.224,0.225]`, `box_thresh 0.6`,
> `unclip_ratio 1.5`. A port that copies the Python pipeline constants will not reproduce v2's
> boxes or confidence values. Port the C# constants above (they are what generated the corpus).

---

## 4. The parity corpus

### 4.1 What exists (`v2/logs/`, 253,284,449 B total; logs are untracked, `v2/.gitignore:14`)

| artifact | count | bytes | content |
|---|---:|---:|---|
| `ocr_log.jsonl` | **9,326 records** (9,325 well-formed; 1 malformed fragment line) | 23,881,151 | every tuner scan: `timestamp, grade, remaining, gradeLine, attributes[3], colorScores, client, region, dpi, scale, rowHeight, lines[{y,x,x2,text,conf}], rawItems[{word,x,x2,y,rowKey}]` |
| `logs/captures/capture_*.png` | **3,356 files** | 226,681,437 | debug images written by the library, see the warning below |
| `logs/captures/full_*.png` | 10 files | — | raw full-client frames (2,865×1,789 physical) with a red rectangle drawn where the OCR region was |
| `ocr_errors.jsonl` | 23 records | 8,479 | persistently unconfirmed scans, each with `capture` path + window rect |
| `run_*.jsonl` + `run_*.txt` | 22 + 22 files, 9,254 JSON lines | — | tuner attempt traces per run (`attempt, grade, remaining, attributes[Name,Value]`) |
| other | `holdspace.log`, `gem.log`, `error.log`, `arduino_debug.txt`, … | — | not OCR parity material |

Record dates: 2026-09-07 (3), 09-09 (38), **09-11 (7,469)**, 09-13 (6), 09-14 (1,809). Grades:
`DG` 5,174 / `G` 4,099 / `N` 38 / `XG` 6 / null 9; `remaining` present in 9,248; three complete
attribute rows in 9,292. 56,415 logged line entries (6.05 per record, conf 0.047–0.999). Region sizes
mostly 280×232 (5,185) and 273×220 (4,051); DPI 144 for 9,320 records (96 for 5), i.e. the corpus is
physical-pixel data at 150% scaling, matching `DESIGN.md`'s DPI note. Client rect 2,865×1,789
physical (= 1,910×1,193 logical).

### 4.2 The serious caveat: the captures are not the OCR inputs

The files named `capture_*.png` are **JPEG byte streams under a `.png` name** (magic `FF D8 FF E0`,
verified) and are twice the region width: e.g. 556×223 for a 278×223 region, 560×232 for 280×232.
`OcrDrawerSkia.DrawTextBlockSKBitmap` (upstream `Utils/OcrDrawerSkia.cs`) draws a side-by-side debug
image — left: the input blended with filled coloured box polygons; right: white with boxes and text —
and encodes it as `SKEncodedImageFormat.Jpeg`. So the 3,356 "captures" are visual evidence of what was
read, **not** the pixels the model was given. All 3,356 pair with a log record by timestamp (0 orphans),
but only the 10 `full_*.png` frames are raw, and they are full-client frames with a red region
rectangle drawn on them.

Consequence: **the corpus can validate v2's post-processing and expected text, but it cannot replay the
detector/recogniser end-to-end.** A raw-input corpus has to be recorded (§4.4).

### 4.3 Coverage

Covered: the tuner read path only — grade (text line + colour fallback inputs are logged as
`colorScores` but the colour decision needs the image), three attribute rows, remaining count, and the
line/word boxes with confidences, at 96 and 144 DPI (mostly 144).

Not covered (no persisted expected outputs at all): pet feeder food-stack counts
(`ReadLinesScored`/`ReadLines` callers — no log), pet panel/queue/ETA reads, the attribute matcher's
output (only raw OCR lines are logged), gem composer colour/result reads (`gem.log` is 1,585 B),
holdspace (no OCR), shop/item-name OCR, quest window text. `logs/reads/` (the 3,373-file evidence
folder `PLAN-DEPLOY.md:206` measured on the development machine) is **not present in this checkout**.

### 4.4 Replay plan and the recording pass

1. **Cheapest first real check (no inference, no runtime choice needed):** replay v3's ported
   post-processing over the corpus log. Feed each record's `rawItems` (word + box + rowKey) and
   `lines` through the ported `BuildLines`/`FilterItems`/grade-line/`FixSign`/remaining logic
   (`OcrEngine.cs:546-569`, `280-345`) and compare `grade`, `attributes`, `remaining`, and line text
   exactly. This exercises 9,326 real records and the `text_fixes` dictionary, and would catch a
   constant drift in rowHeight bucketing, DPI/rounding, grade cutting or sign fixing. It does NOT test
   det/cls/rec.
2. **Record raw inputs.** Add a raw-crop save (PNG, unmodified region) beside the ocr record in v2, or
   use v3's `capture` once `seal-win` is wired, and record a balanced session: every grade
   (N/G/DG/XG/**SG** — corpus has no SG and only 6 XG), 96/120/144/150% DPI, both window sizes,
   3-attribute rows and empty rows, remaining values, and the text-fix edge cases. Target a few hundred
   images with the identical `region`/`dpi`/`scale` fields.
3. **Runtime spike:** run the three pinned models on one recorded raw crop with the chosen `ort`
   version, reproduce a record's `lines`/`attributes`/`grade`/`remaining`, and compare. This is the
   first proof that the port's preprocessing constants and the runtime agree with v2.
4. **Full runner:** a headless `seal-ocr` test that loads the raw-image corpus + expected records,
   runs the full pipeline, and asserts exact text with a stated confidence tolerance; run it in CI
   offline (models fetched at build or vendored as release assets — see §5).
5. **Live recording gaps** (screens/moments to capture, each with a raw crop + record):
   tuner window at each grade with all three attribute rows and the remaining count visible; the pet
   boarding/food window showing the food stack counts at several stack sizes and empty; the pet queue
   panel (rows, queue times, boarding states); the attributes tab list (all supported attribute
   variants and a garbled read); the gem composer result box at each grade colour and empty mode; the
   shop/bag item list (if an OCR path uses it); the quest window text; and the same screens at 120% and
   150% DPI with the two window sizes.

---

## 5. What blocks it, and the order of work

**Blockers / risks**

1. **Raw-input corpus is missing** (biggest). The 3,356 "captures" are drawn JPEG debug images, not
   model inputs; only 10 full-client frames are raw and they carry a red rectangle. End-to-end parity
   cannot be measured until a recording pass saves unmodified region crops (§4.4 step 2).
2. **The image-processing dependency is not scoped.** The det post-processing uses OpenCV-specific
   operations: `Threshold`, `Dilate`, `FindContours`/`ApproxSimple`, `MinAreaRect`/`BoxPoints`,
   `FillPoly`/`Mean`, `WarpPerspective` (cubic, replicate border), `ContourArea`/`ArcLength`, plus
   Clipper's `InflatePaths` for unclip. Pure Rust equivalents (`image`/`imageproc`/`geo`/`i_overlay`)
   exist for most, but min-area-rect + contour extraction + polygon inflate are hand-port work, and the
   `opencv` crate pulls a native OpenCV build the v3 pure-crate design avoids. This is a planning
   decision and a measurable spike, not a footnote.
3. **Runtime/version drift changes confidences.** v2 ran ONNX Runtime 1.29; current `ort` targets 1.31;
   `tract` is numerically different. Exact text should hold, but confidence values and near-threshold
   detections can move. Calibrated gates in the tools (e.g. the feeder's confidence gate) must be
   re-validated against the corpus, and the version must be pinned and recorded (see §2.2 policy).
4. Model licensing/attribution needs the release-time check (§1.1); models are untracked in git, so the
   v3 build must either fetch them from a pinned source at build time or vendor them as release assets
   (do not commit 16 MB binaries without a decision).

**Recommended order**

1. Ported post-processing parity over `ocr_log.jsonl` (9,326 records) — cheapest, no runtime, no
   images; proves the constants that v2's own code contributes.
2. Raw-crop recording pass for the tuner path (a few hundred images) — unblocks all inference parity.
3. Image-processing spike for det post-processing (contours/min-area-rect/inflate/perspective) with the
   chosen Rust crates; decide `tract` vs `ort` only after the runtime spike has a real crop to run on.
4. `ort` runtime spike on a recorded crop: reproduce one record end-to-end; pin the version and thread
   settings; write the tolerance policy.
5. Full parity runner in CI + live recordings for the feeder/pet/attribute/gem paths.

**Three biggest risks, in order:** (1) no raw-input corpus today, so "parity" would be measured against
reconstructions rather than the actual pixels; (2) the OpenCV-shaped post-processing has no single pure
Rust home and its port cost is unbounded until the spike; (3) runtime version drift (or a tract
switch) moves confidence values near the tools' gates, so the corpus tolerances and the tool gates must
be revisited together.

---

## 6. Reproduction commands (all read-only)

From `C:\Git_repo\seal-tools`:

- Model sizes/hashes: `Get-FileHash v2/models/*.onnx -Algorithm SHA256`; sizes as listed.
- Provenance: compare with
  `%LOCALAPPDATA%\Programs\Python\Python312\Lib\site-packages\rapidocr_onnxruntime\models\` (identical
  hashes) and `rapidocr_onnxruntime-1.4.4.dist-info\METADATA`.
- DLL resources: load `…\.nuget\packages\rapidocrsharponnx\1.3.0\lib\net8.0\RapidOCRSharpOnnx.dll` and
  enumerate `GetManifestResourceNames()`; sizes/hashes from the resource streams.
- Dictionary: byte-search the rec ONNX for the `character` metadata map entry (the value is a
  length-delimited protobuf string) and hash the value.
- Corpus: Python `json.JSONDecoder.raw_decode` per line of
  `v2/logs/ocr_log.jsonl` (one line is a malformed fragment beginning mid-JSON), `os.listdir` +
  `struct.unpack(">II", png[16:24])` for the capture dimensions, and the `timestamp` set intersection
  for pairing (3,356 = all captures).
- Release payload: `System.IO.Compression.ZipFile` over `v2/dist/SealTools-v2.10.zip` entries
  `models\*.onnx` and `onnxruntime.dll` (sizes in §1.1/§3).

---

## Runtime spike results (task-28, measured 2026-10-25, commit `e106c2b`)

The runtime choice was proven on the real models before any pipeline work:

- **The dictionary finding is now a runtime fact.** The rec model's only custom metadata key is
  `character` (6,623 entries; `blank` + dict + space = 6,625 slots; SHA-256 `28b2362a…d7f7`), and the
  model's own output is `softmax_11.tmp_0 [-1, -1, 6625]` — the class axis equals the decorated
  dictionary, so no file-based dictionary can be involved.
- **Shapes:** rec input `x` f32 `[-1,3,-1,-1]`; det input `x` f32 `[-1,3,-1,-1]`, det output
  `sigmoid_0.tmp_0 [-1,1,-1,-1]`. **Correction:** the documented 48-px rec height is a preprocessing
  contract the model does NOT enforce (its height axis is dynamic) — the port must impose it.
- **Determinism:** one synthetic `[1,3,48,320]` inference, two runs with intra/inter threads = 1,
  produced bit-identical logits and identical CTC decode; pure decode tests pin blank-skip, repeat
  collapse, the confidence mean and the blank+repeat+space case. Measured on this machine: rec load
  **173 ms**, inference **17 ms**, det load **154 ms**.
- **Shipping reality changed twice:** `download-binaries` on `x86_64-pc-windows-msvc` resolves to the
  **DirectML static distribution** — the runtime is linked in and the payload is **`DirectML.dll`
  (18,527,776 B)**, so a release zip must carry it and its redistribution terms need confirming; a
  clean build also needs network for the ORT download or a seeded `ORT_CACHE_DIR`.
- **Det post-processing** remains the spike it was: threshold -> 2x2 dilate -> `FindContours` ->
  `MinAreaRect`/`BoxPoints` -> `FillPoly`+`Mean` >= 0.6 -> `ContourArea`/`ArcLength` + Clipper
  `InflatePaths` (1.5) -> `WarpPerspective` (cubic, replicate) -> Y-sort with the 10 px row
  threshold. Threshold/dilate/masked-mean/crop are easy pure Rust; contours, min-area-rect and
  polygon offset are the parts no maintained pure-Rust crate covers directly (task-29).
