"""
Seal Online 發條 OCR Engine
============================
Captures the 發條 (magic tuning) window and reads:
  1. GRADE line  — grade letter (N/G/DG/XG/SG)
  2. 3 Chinese attribute lines below the grade

Usage:
  from ocr_engine import TuningOCR
  ocr = TuningOCR()
  result = ocr.scan()  # returns dict with grade + attributes

Requires: rapidocr-onnxruntime, mss, opencv, numpy
"""

import sys
from pathlib import Path
from datetime import datetime
import cv2
import numpy as np
import mss
from rapidocr_onnxruntime import RapidOCR

# ── Configuration ──────────────────────────────────────────
SCRIPT_DIR = Path(__file__).parent
LOG_DIR = SCRIPT_DIR / "logs"
LOG_DIR.mkdir(exist_ok=True)

# Game window name
GAME_WINDOW = "TW_LIVE"

# Tuning window capture region (relative to game window top-left)
TUNING_REGION = {"left": 1140, "top": 840, "width": 300, "height": 320}

# ── OCR text cleanup ────────────────────────────────────────
import re

def clean_text(text):
    """Fix common OCR garbling."""
    t = text.strip()

    # Whole-string fixes
    if t == "GRADB 8": return "GRADE :"
    if t == "GRADB": return "GRADE :"
    if t == "GGRADE 8": return "GRADE :"
    if t == "GADB 8": return "GRADE :"
    if t == "GADB": return "GRADE :"

    # Substring fixes (word-level)
    t = re.sub(r'\bGRADB\b', 'GRADE', t)
    t = re.sub(r'\bRADE\b', 'GRADE', t)
    t = re.sub(r'\bCRA\b', 'GRADE', t)
    t = t.replace("4&", "46").replace("3&", "36")
    t = t.replace("攻孽力", "攻擊力")
    t = t.replace("攻孽遠度", "攻擊速度")
    t = t.replace("攻孽", "攻擊")
    t = t.replace("力星", "力量")   # OCR misreads 量→星
    t = t.replace("防票力", "防禦力")  # RapidOCR reads 禦→票
    t = t.replace("攻速度", "攻擊速度")  # RapidOCR drops 擊
    t = t.replace("攻擎速度", "攻擊速度")  # RapidOCR: 擊→擎
    t = t.replace("攻擎力", "攻擊力")    # RapidOCR: 擊→擎
    t = t.replace("避率", "迴避率")     # RapidOCR drops 迴
    t = t.replace("經值", "經驗值")     # RapidOCR drops 驗
    t = t.replace("減少害力", "減少傷害力")  # RapidOCR drops 傷
    t = t.replace("艘力", "體力")       # RapidOCR: 體→艘
    # Simplified -> Traditional fixes (RapidOCR defaults to simplified Chinese)
    S2T = {
        "减少": "減少", "装备": "裝備", "伤害": "傷害",
        "级": "級", "等级": "等級", "限制等级": "限制等級",
        "配戴限制等级": "配戴限制等級",
        "闭": "閉", "关": "關",
    }
    for s, t_trad in S2T.items():
        t = t.replace(s, t_trad)
    t = t.replace("傷窖", "傷害")
    t = t.replace("惕窖", "傷害")
    t = t.replace("愕窖", "傷害")
    t = t.replace("副本傷窖增加", "副本傷害增加")
    t = t.replace("副本惕窖增加", "副本傷害增加")
    t = t.replace("副本愕窖增加", "副本傷害增加")
    t = t.replace("配域", "配戴")
    t = t.replace("限制等級", "限制等級")

    return t


# ── Grade detection: pixel color thresholds ─────────────────
# GRADE label ends around x≈100, grade letter at x≈170-220
# Capture now at (1140,840), so these are capture-relative
GRADE_AREA = {"x1": 149, "y1": 1, "x2": 232, "y2": 44}

# ── Text line Y ranges (capture coords, origin at 1140,840) ─
# Note: TEXT_LINES kept for reference, actual line detection uses Y-filtering on full OCR
_TEXT_LINES = [
    {"name": "grade",   "y1": 5,  "y2": 28,  "x1": 0, "x2": 300},
    {"name": "attr1",   "y1": 45, "y2": 65,  "x1": 0, "x2": 300},
    {"name": "attr2",   "y1": 72, "y2": 92,  "x1": 0, "x2": 300},
    {"name": "attr3",   "y1": 98, "y2": 118, "x1": 0, "x2": 300},
]


def find_game_window():
    """Find TW_LIVE game window. Returns (left, top, width, height)."""
    import ctypes
    from ctypes import wintypes
    user32 = ctypes.windll.user32
    results = []

    def cb(hwnd, _):
        if user32.IsWindowVisible(hwnd):
            n = user32.GetWindowTextLengthW(hwnd)
            if n:
                buf = ctypes.create_unicode_buffer(n + 1)
                user32.GetWindowTextW(hwnd, buf, n + 1)
                if GAME_WINDOW in buf.value:
                    r = wintypes.RECT()
                    user32.GetWindowRect(hwnd, ctypes.byref(r))
                    results.append((r.left, r.top, r.right - r.left, r.bottom - r.top))
        return True

    WEP = ctypes.WINFUNCTYPE(ctypes.c_bool, wintypes.HWND, ctypes.c_int)
    user32.EnumWindows(WEP(cb), 0)
    return results[0] if results else None


class TuningOCR:
    """Reads the 發條 window: grade + 3 Chinese attribute lines."""

    def __init__(self):
        self.window = None
        self.engine = None

    def _init_ocr(self):
        """Lazy-load OCR models."""
        if self.engine is None:
            self.engine = RapidOCR()

    def _ocr(self, img):
        """Run RapidOCR on an image. Returns list of (bbox, text, conf)."""
        result, _ = self.engine(img)
        if result is None:
            return []
        return [(box, text, conf) for box, text, conf in result]

    def _locate_window(self):
        """Find game window position. Returns True if found."""
        self.window = find_game_window()
        return self.window is not None

    def _capture(self):
        """Capture the tuning window. Returns BGR numpy array."""
        L, T, W, H = self.window
        region = {
            "left": L + TUNING_REGION["left"],
            "top": T + TUNING_REGION["top"],
            "width": TUNING_REGION["width"],
            "height": TUNING_REGION["height"],
        }
        with mss.mss() as sct:
            img = sct.grab(region)
            return np.array(img)[:, :, :3]

    def _detect_grade_color(self, img):
        """Detect grade letter by pixel color.
        N = white text (black border), G = blue, DG = yellow, XG = red, SG = purple.
        """
        a = GRADE_AREA
        crop = img[a["y1"]:a["y2"], a["x1"]:a["x2"]]
        B = crop[:, :, 0].astype(int)
        G_ch = crop[:, :, 1].astype(int)
        R = crop[:, :, 2].astype(int)

        # Masks for each color
        m_yellow = (R > G_ch + 20) & (G_ch > B + 20) & (R > 120) & (G_ch > 120)
        m_blue   = (B > G_ch + 25) & (B > R + 25) & (B > 80)
        m_red    = (R > G_ch + 40) & (R > B + 40) & (R > 150)
        m_purple = (R > B + 30) & (B > G_ch + 30) & (R > 80) & (B > 80)
        m_white  = (R > 180) & (G_ch > 180) & (B > 180)

        # White only: exclude pixels already counted as colored
        m_any_color = m_yellow | m_blue | m_red | m_purple
        white = np.sum(m_white & ~m_any_color)
        yellow = np.sum(m_yellow)
        blue = np.sum(m_blue)
        red = np.sum(m_red)
        purple = np.sum(m_purple)

        # Decision: use ratio, not absolute count (white label always leaks)
        # N = mostly white, little yellow   |   DG = lots of yellow, less white
        total = white + yellow + blue + red + purple
        if total < 10:
            return None, {"N": int(white), "G": int(blue), "DG": int(yellow), "XG": int(red), "SG": int(purple)}

        if yellow > 80 and yellow > white * 0.3:
            grade = "DG"
        elif blue > 60 and blue > white * 0.5:
            grade = "G"
        elif red > 60:
            grade = "XG"
        elif purple > 60:
            grade = "SG"
        elif white > 100 and yellow < 60 and blue < 60:
            grade = "N"
        else:
            grade = None

        scores = {"N": int(white), "G": int(blue), "DG": int(yellow), "XG": int(red), "SG": int(purple)}
        return grade, scores

    def _ocr_line(self, img, y1, y2, x1, x2):
        """OCR a single text line using raw EasyOCR (no preprocessing)."""
        crop = img[y1:y2, x1:x2]
        if crop.size == 0:
            return []

        # Raw OCR — CLAHE destroys text on wide strips
        en_results = self._ocr(crop)
        ch_results = self._ocr(crop)

        texts = []
        for bbox, text, conf in en_results:
            if conf > 0.15 and len(text.strip()) > 1:
                texts.append({"text": text.strip(), "conf": conf, "lang": "en"})

        for bbox, text, conf in ch_results:
            t = text.strip()
            if conf > 0.1 and len(t) > 1:
                # Avoid duplicates
                if not any(x["text"] == t for x in texts):
                    has_ch = any('一' <= c <= '鿿' for c in t)
                    texts.append({"text": t, "conf": conf, "lang": "ch" if has_ch else "en"})

        return texts

    def scan(self):
        """
        Capture and scan the 發條 window.
        Returns:
          dict with 'grade', 'attributes', 'all_text', 'timestamp', 'window'
          or None if game window not found.
        """
        if not self._locate_window():
            return None

        self._init_ocr()
        img = self._capture()

        # Save capture for future OCR testing / model comparison
        import yaml
        cfg_path = SCRIPT_DIR / "config.yaml"
        save_captures = True  # default
        if cfg_path.exists():
            with open(cfg_path, "r", encoding="utf-8") as f:
                cfg_data = yaml.safe_load(f.read()) or {}
            save_captures = cfg_data.get("save_captures", True)

        CAPTURE_DIR = LOG_DIR / "captures"
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S_%f")[:19]
        capture_path = CAPTURE_DIR / f"capture_{timestamp}.png"
        if save_captures:
            CAPTURE_DIR.mkdir(parents=True, exist_ok=True)
            cv2.imwrite(str(capture_path), img)

        # 1. Grade: try OCR first (more reliable for DG/G), fall back to color
        grade_area = img[1:44, 149:232]  # exact grade letter area
        grade_ocr = self._ocr(grade_area)
        grade = None
        for bbox, text, conf in grade_ocr:
            t = text.strip().upper()
            if conf > 0.5:
                if t in ["DG", "XG", "SG"]:
                    grade = t; break
                elif t == "N": grade = "N"
                elif t == "G": grade = "G"
                elif t == "D" and not grade: grade = "DG"  # "D" = first char of "DG"

        # 2. Fallback: color detection (always compute for logging)
        _, color_scores = self._detect_grade_color(img)
        if not grade:
            # Fall back to color if OCR found nothing
            color_grade, _ = self._detect_grade_color(img)
            if color_grade:
                grade = color_grade

        # 2. OCR: EN on full image for grade line, CH on attr area for labels
        # Full image is now 300x320, all coords are capture-relative (origin at 1140,840)
        H, W = img.shape[:2]
        all_results = self._ocr(img)  # Single OCR pass for everything

        # Grade line: y=1-44 in capture (game y=840-880)
        grade_line = []
        for bbox, text, conf in all_results:
            y = int(min(p[1] for p in bbox))
            if 1 <= y <= 44 and conf > 0.1 and len(text.strip()) > 1:
                t = clean_text(text.strip())
                if t and t != '300':
                    grade_line.append(t)

        # Attributes: full-image OCR + Y-filter (simpler & works with RapidOCR)
        rows = {}
        for bbox, text, conf in all_results:
            y = int(min(p[1] for p in bbox))
            t = clean_text(text.strip())
            # Only capture attribute zone (y=45-135), skip grade/header/count/buttons
            if conf < 0.3 or len(t) < 2 or y < 42 or y > 140:
                continue
            row_key = y // 25
            if row_key not in rows:
                rows[row_key] = []
            if t not in rows[row_key]:
                rows[row_key].append(t)

        # Extract label + value per row
        attr_lines = []
        for rk in sorted(rows.keys()):
            texts = rows[rk]
            labels, values = [], []
            for t in texts:
                t = t.strip()
                if not t: continue
                has_ch = any('一' <= c <= '鿿' for c in t)
                has_sign = '+' in t or '-' in t
                is_num = t.replace(',','').replace('.','').lstrip('+-').isdigit()
                if has_ch: labels.append(t)
                elif has_sign or (is_num and len(t) <= 3): values.append(t)
                else: labels.append(t)

            y_approx = rk * 25  # capture-relative
            attr_lines.append({
                "y": y_approx,
                "labels": labels,
                "values": values
            })

        # Attributes that are always negative
        NEGATIVE_ATTRS = ["減少", "限制等級", "配戴限制"]

        def fix_sign(labels, values):
            """Apply correct sign based on attribute type."""
            result = []
            for v in values:
                v = v.strip().lstrip('+')
                is_neg = any(n in ''.join(labels) for n in NEGATIVE_ATTRS)
                if is_neg and not v.startswith('-'):
                    v = '-' + v
                elif not is_neg and v.isdigit():
                    v = '+' + v
                result.append(v)
            return labels + result

        # Take first 3 lines (skip grade at y<5)
        attr_lines = [a for a in attr_lines if a["y"] >= 10][:3]
        attr1_ch = fix_sign(attr_lines[0]["labels"], attr_lines[0]["values"]) if len(attr_lines) > 0 else []
        attr2_ch = fix_sign(attr_lines[1]["labels"], attr_lines[1]["values"]) if len(attr_lines) > 1 else []
        attr3_ch = fix_sign(attr_lines[2]["labels"], attr_lines[2]["values"]) if len(attr_lines) > 2 else []

        grade_line = list(dict.fromkeys(grade_line))

        # 3. Remaining spring count (game y=1040, capture y=200)
        remaining = None
        for bbox, text, conf in all_results:
            y = int(min(p[1] for p in bbox))
            t = text.strip()
            if 190 <= y <= 235 and conf > 0.4 and len(t) <= 6:
                parts = t.split()
                for p in reversed(parts):
                    if p.isdigit() and 1 <= int(p) <= 99999:
                        remaining = int(p)
                        break
                if remaining is not None:
                    break

        # 4. Build result
        result = {
            "timestamp": timestamp,
            "grade": grade,
            "grade_color_scores": {k: int(v) for k, v in color_scores.items()},
            "remaining": remaining,
            "grade_line": grade_line,
            "attributes": [attr1_ch, attr2_ch, attr3_ch],
            "capture": str(capture_path),
            "window": {
                "left": self.window[0],
                "top": self.window[1],
                "width": self.window[2],
                "height": self.window[3],
            } if self.window else {},
        }

        # 4. Save to log
        log_path = LOG_DIR / "ocr_log.jsonl"
        with open(log_path, "a", encoding="utf-8") as f:
            import json
            f.write(json.dumps(result, ensure_ascii=False, default=str) + "\n")

        return result

    def scan_pretty(self):
        """Scan and return a formatted string for display."""
        r = self.scan()
        if r is None:
            return "ERROR: Game window (TW_LIVE) not found!"

        lines = []
        lines.append(f"Grade: {r['grade']}  (scores: N={r['grade_color_scores']['N']} G={r['grade_color_scores']['G']} DG={r['grade_color_scores']['DG']})")
        lines.append(f"Grade line OCR: {r['grade_line']}")
        for i, attr in enumerate(r['attributes'], 1):
            lines.append(f"Attr{i}: {attr}")
        lines.append(f"Capture: {r['capture']}")
        return "\n".join(lines)


# ── Standalone test ────────────────────────────────────────
if __name__ == "__main__":
    sys.stdout.reconfigure(encoding='utf-8')

    print("Seal Online 發條 OCR Engine")
    print("=" * 40)

    ocr = TuningOCR()
    print("Scanning...")
    result = ocr.scan()

    if result is None:
        print("ERROR: TW_LIVE window not found! Is the game running?")
    else:
        print(f"\nGrade: {result['grade']}")
        print(f"Color scores: {result['grade_color_scores']}")
        print(f"\nGrade line text: {result['grade_line']}")
        for i, attr in enumerate(result['attributes'], 1):
            print(f"Attr {i}: {attr}")
        print(f"\nCapture saved: {result['capture']}")
        print(f"Log: {LOG_DIR / 'ocr_log.jsonl'}")
