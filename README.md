# Seal Online 發條 Auto-Tuner

Automates the **發條 (Spring/Magic Tuning)** system in Seal Online Taiwan server.

## What It Does

1. **Auto-click** — Arduino Pro Micro clicks the confirm button + presses Enter in a loop
2. **OCR scan** — EasyOCR reads the grade (N/G/DG/XG/SG) and 3 attribute lines
3. **Auto-stop** — Stops when target grade (DG) is reached
4. **Attribute filter** — Optional: keep rolling until specific attributes match
5. **Logging** — Compact `.txt` log + detailed `.jsonl` log + screen captures

## Hardware Required

- **Arduino Pro Micro (ATmega32U4)** — acts as a real USB mouse (GameGuard can't block it)
- Upload `arduino/seal_mouse/seal_mouse.ino`

## Quick Start

```bash
# 1. Install dependencies
pip install -r requirements.txt

# 2. Install Tesseract (for EasyOCR)
# Download: https://github.com/UB-Mannheim/tesseract/wiki

# 3. Plug in Arduino, configure config.yaml

# 4. Run
python seal_tuner.py
```

## Controls

| Key | Action |
|-----|--------|
| **F12** | Toggle START / STOP |
| **F11** | Quit |
| **Unplug Arduino** | Hardware emergency stop |

## Configuration (`config.yaml`)

```yaml
arduino_port: COM5
target_grade: DG      # Stop when this grade is reached
max_retries: 500
save_captures: true    # Save screenshots for future OCR testing

# Attribute filter (optional)
filter:
  enabled: false       # Set to true to enable
  match_mode: any      # any = match at least one rule  |  all = match all rules
  rules:
    - name: 魔法力      # Just check attribute exists
      min: 30          # Optional: must be >= this value
    - name: 攻擊力
      min: 25
    - name: 必殺技
      min: 8
      max: 15          # Optional: must be within range
```

## Output

| File | Purpose |
|------|---------|
| `logs/run_*.txt` | Compact readable log |
| `logs/run_*.jsonl` | Full detail log |
| `logs/captures/*.png` | Screen captures for OCR testing |

### TXT Log Format

```
001 N 28 | 防禦力+24 | 迴避率+4 | 限制等級-3
002 N 27 | 魔法力+10 | 限制等級-1 | 必殺技+4
003 G 26 | 攻擊力+28 | 迴避率+1 | 必殺技+5
```

### Filter: All Available Attributes

These are the **exact names** to use in `config.yaml` rules. The matcher handles OCR garbling automatically.

#### Damage Stats
| Name | Description | Typical Value Range |
|------|-------------|-------------------|
| `攻擊力` | Attack Power | 10-120 |
| `魔法力` | Magic Power | 10-120 |
| `防禦力` | Defense | 10-120 |

#### Combat Stats
| Name | Description | Typical Value Range |
|------|-------------|-------------------|
| `攻擊速度` | Attack Speed | 1-20 |
| `必殺技` | Critical Rate | 2-20 |
| `命中率` | Hit Rate | 2-20 |
| `迴避率` | Evasion Rate | 2-20 |
| `移動速度` | Movement Speed | 1-20 |

#### HP / AP
| Name | Description | Typical Value Range |
|------|-------------|-------------------|
| `HP(值)` | HP (flat) | 100-1000 |
| `AP(值)` | AP (flat) | 100-1000 |
| `HP(%)` | HP % | 1-4% |
| `AP(%)` | AP % | 1-4% |

#### Per-Level Stats (two values: X levels + stat)
| Name | Description | Value = X (lower is better) |
|------|-------------|---------------------------|
| `每級+1力量` | STR per X levels | 4-10 |
| `每級+1敏捷` | AGI per X levels | 4-10 |
| `每級+1智力` | INT per X levels | 4-10 |
| `每級+1幸運` | LUK per X levels | 4-10 |
| `每級+1體力` | VIT per X levels | 13-50 |
| `每級+1精神` | SPI per X levels | 14-50 |

#### Utility
| Name | Description | Typical Value Range |
|------|-------------|-------------------|
| `減少道具等級限制` | Reduce equip level req | 1-20 (always negative) |
| `經驗值獲得量增加` | EXP gain increase | 1-15% |
| `副本傷害增加` | Dungeon damage | 1-4% |
| `增加傷害` | Damage increase | 1-4% |
| `減少傷害` | Damage reduction | 1-4% |

### Filter Modes

| Mode | Behavior |
|------|----------|
| `any` | Stop if ANY listed attribute appears with matching value |
| `all` | Stop only if ALL listed attributes appear with matching value |
| No min/max | Just check if attribute exists (any value OK) |
| With min/max | Check attribute value is within range |

### Filter Examples

```yaml
# Example 1: Any of these two with good values → stop
filter:
  enabled: true
  match_mode: any
  rules:
    - name: 魔法力
      min: 40
    - name: 攻擊力
      min: 40

# Example 2: Must have ALL three (with or without values)
filter:
  enabled: true
  match_mode: all
  rules:
    - name: 攻擊速度
      min: 8
    - name: 必殺技
    - name: 迴避率
      min: 5

# Example 3: Just check attribute exists (any value OK)
filter:
  enabled: true
  match_mode: any
  rules:
    - name: 攻擊力
    - name: 魔法力

# Example 4: Per-level stat — check interval is low (good)
filter:
  enabled: true
  match_mode: any
  rules:
    - name: 每級+1智力
      max: 6       # X <= 6 is good (lower = more stats)
```

## Files

```
SEALONLINE SCRIPTS/
├── seal_tuner.py          ← Main tuner (run this)
├── ocr_engine.py          ← OCR engine (EasyOCR EN+CH)
├── attr_matcher.py        ← Attribute name matching + filter
├── config.yaml            ← Settings
├── requirements.txt       ← Python dependencies
├── ATTRIBUTE_REFERENCE.md ← Complete attribute reference
├── arduino/
│   └── seal_mouse/
│       └── seal_mouse.ino ← Arduino sketch
├── logs/
│   ├── run_*.txt
│   ├── run_*.jsonl
│   └── captures/
└── magic tuning/          ← Reference screenshots
```
