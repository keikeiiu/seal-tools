# PP-OCRv6 model study — tiny vs small for Traditional Chinese + English

**Status: model-package inspection complete (2026-11-09). No ONNX export or live OCR run yet.**
This is evidence for [PLAN-OCR-V6](../plans/PLAN-OCR-V6.md), not a claim that v6 works in the app.

The question: v6 is advertised as one multilingual model. Does its recogniser dictionary actually
cover the Traditional Chinese used by the TW client alongside English?

## 1. Packages inspected

Both are official PaddleOCR 3.x inference packages; only the recogniser was downloaded for this
study.

| Model | Archive | bytes | SHA-256 |
|---|---|---:|---|
| `PP-OCRv6_tiny_rec_infer.tar` | official Paddle model URL | 4,597,760 | `54403af57bc53981d3dce091c6c80dfc5ae30d58495541777126a54bac450604` |
| `PP-OCRv6_small_rec_infer.tar` | official Paddle model URL | 21,442,560 | `da460f968ce9f88325ac3a34fa302077d6e9b0dcefb16ba3137cd7796f879d06` |

Each archive contains:

```text
PP-OCRv6_<tier>_rec_infer/inference.json
PP-OCRv6_<tier>_rec_infer/inference.pdiparams
PP-OCRv6_<tier>_rec_infer/inference.yml
```

The `inference.yml` carries the preprocessing (`RecResizeImg`, image shape `[3,48,320]`) and a
`PostProcess: CTCLabelDecode` block with `character_dict` embedded as YAML.

## 2. Dictionary measurements

Parsed from `inference.yml`'s `character_dict` list, UTF-8:

| Tier | dictionary entries | model class axis | English/ASCII present | Traditional probes |
|---|---:|---:|---|---|
| tiny | **6,904** | **6,906** (`+ blank + space`) | yes | **missing 擊 (U+64CA), 預 (U+9810)** |
| small | **18,708** | **18,710** (`+ blank + space`) | yes | all probes present |

Probes included: 寵物飼料裝備攻擊防禦經驗藍天龍劍預約剩餘. Tiny contains all except 擊 and 預;
small contains all of them.

**Finding:** tiny's 6,904-entry dictionary is not a complete Traditional Chinese dictionary. The
“single model, 49/50 languages” claim does not make the tiny tier sufficient for the TW client.
**Small is the minimum viable tier for Traditional Chinese + English.**

## 3. Recognition configuration

From the tiny `inference.yml` (small has the same preprocessing shape):

```yaml
PreProcess:
  - DecodeImage: {img_mode: BGR}
  - RecResizeImg:
      image_shape: [3, 48, 320]
PostProcess:
  name: CTCLabelDecode
  character_dict: [...]   # embedded in the YAML
```

The model's output class axis is the dictionary + blank + space (6,906 tiny, 18,710 small). The
current `seal-ocr` `RecModel` reads the dictionary from ONNX metadata, not from YAML. Any ONNX export
must carry the dictionary in metadata, or the adapter must add an external dictionary file. This is a
real integration change, not a model swap.

## 4. What this study does not prove

- That the Paddle inference graph exports cleanly to ONNX.
- That ONNX Runtime + DirectML support every operator in the v6 graph.
- That v6 small actually recognises TW text better than v4 on live crops — that needs the corpus and
  A/B run.
- That the detector tier/parameters (v6 det) behave like v4's detector on dense attribute rows.

Those are Task 0 and the corpus gate in the plan.

## 5. Consequence for the plan

- **Target: v6 small.**
- **Tiny: reference only**, disqualified for the TW client by the dictionary gap.
- **Medium: accuracy reference**, likely too large for the release payload.
- The ONNX export must preserve or supply the 18,708-entry dictionary; missing it is a hard failure,
  never a silent v4 fallback.

## 6. Reproduce

```powershell
# in a scratch directory
$url = 'https://paddle-model-ecology.bj.bcebos.com/paddlex/official_inference_model/paddle3.0.0/PP-OCRv6_small_rec_infer.tar'
Invoke-WebRequest -Uri $url -OutFile PP-OCRv6_small_rec_infer.tar
tar -xf PP-OCRv6_small_rec_infer.tar
# then parse PostProcess.character_dict from inference.yml and count entries
```

The archive URLs and sizes above are the evidence; the models themselves are not committed.
