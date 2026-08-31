"""
Batch test runner — runs all single-rule filter configs in sequence.
One F12 press to start, runs until each test matches, auto-advances.

Usage: python test_runner.py
"""
import sys
import time
import ctypes
import winsound
import json
import yaml
from pathlib import Path
from datetime import datetime
import serial
import serial.tools.list_ports
from ocr_engine import TuningOCR
from attr_matcher import match_attributes, check_filter

sys.stdout.reconfigure(encoding='utf-8')

SCRIPT_DIR = Path(__file__).parent
CONFIG_PATH = SCRIPT_DIR / "config.yaml"
LOG_DIR = SCRIPT_DIR / "test_results"
LOG_DIR.mkdir(exist_ok=True)
user32 = ctypes.windll.user32

GRADE_ORDER = {"N": 0, "G": 1, "DG": 2, "XG": 3, "SG": 4}
MAX_ATTEMPTS_PER_TEST = 200

# ── All 24 single-rule tests ──────────────────
TESTS = [
    # Core stats
    {"id": "01_攻擊力",       "rules": [{"name": "攻擊力", "min": 30}]},
    {"id": "02_魔法力",       "rules": [{"name": "魔法力", "min": 30}]},
    {"id": "03_防禦力",       "rules": [{"name": "防禦力", "min": 30}]},
    # Combat
    {"id": "04_攻擊速度",     "rules": [{"name": "攻擊速度", "min": 5}]},
    {"id": "05_必殺技",       "rules": [{"name": "必殺技", "min": 5}]},
    {"id": "06_命中率",       "rules": [{"name": "命中率", "min": 5}]},
    {"id": "07_迴避率",       "rules": [{"name": "迴避率", "min": 5}]},
    {"id": "08_移動速度",     "rules": [{"name": "移動速度", "min": 5}]},
    # HP/AP
    {"id": "09_HP值",         "rules": [{"name": "HP(值)", "min": 100}]},
    {"id": "10_AP值",         "rules": [{"name": "AP(值)", "min": 100}]},
    {"id": "11_HP%",          "rules": [{"name": "HP(%)", "min": 1}]},
    {"id": "12_AP%",          "rules": [{"name": "AP(%)", "min": 1}]},
    # Per-level
    {"id": "13_力量",         "rules": [{"name": "每級+1力量", "max": 10}]},
    {"id": "14_敏捷",         "rules": [{"name": "每級+1敏捷", "max": 10}]},
    {"id": "15_智力",         "rules": [{"name": "每級+1智力", "max": 10}]},
    {"id": "16_幸運",         "rules": [{"name": "每級+1幸運", "max": 10}]},
    {"id": "17_體力",         "rules": [{"name": "每級+1體力", "max": 40}]},
    {"id": "18_精神",         "rules": [{"name": "每級+1精神", "max": 40}]},
    # Utility
    {"id": "19_減少限制",     "rules": [{"name": "減少道具等級限制"}]},
    {"id": "20_經驗值",       "rules": [{"name": "經驗值獲得量增加"}]},
    {"id": "21_副本傷害",     "rules": [{"name": "副本傷害增加"}]},
    {"id": "22_增加傷害",     "rules": [{"name": "增加傷害"}]},
    {"id": "23_減少傷害",     "rules": [{"name": "減少傷害"}]},
]


def find_arduino():
    for p in serial.tools.list_ports.comports():
        if p.vid == 0x2341 and p.pid in (0x8036, 0x8037):
            return p.device
        if "ARDUINO" in p.description.upper():
            return p.device
    return None


def key(vk):
    return bool(user32.GetAsyncKeyState(vk) & 0x8000)


def write_config(test):
    """Write test config to config.yaml."""
    cfg = {
        "arduino_port": "COM5",
        "target_grade": "DG",
        "max_retries": MAX_ATTEMPTS_PER_TEST,
        "save_captures": True,
        "timing": {"click_enter_delay": 0.8, "ocr_delay": 1.0, "loop_delay": 0.3},
        "filter": {
            "enabled": True,
            "match_mode": "any",
            "require_grade": "DG",
            "rules": test["rules"],
        },
    }
    with open(CONFIG_PATH, "w", encoding="utf-8") as f:
        yaml.dump(cfg, f, allow_unicode=True)


def main():
    port = find_arduino()
    if not port:
        print("[!] Arduino not found"); return
    ser = serial.Serial(port, 115200, timeout=1)
    time.sleep(2)
    print(f"[OK] Arduino on {port}")

    ocr = TuningOCR()
    ocr._init_ocr()
    print("[OK] OCR ready")

    results = []
    total_tests = len(TESTS)

    # ── Wait for F12 once ──────────────────────
    print(f"\n{'='*50}")
    print(f"  BATCH TEST RUNNER — {total_tests} tests")
    print(f"{'='*50}")
    print("\nPosition mouse over confirm button -> F12 to start ALL tests")
    print("F11 = quit\n")

    running = False
    f12_was = key(0x7B)
    while not running:
        time.sleep(0.05)
        if key(0x7A): ser.close(); return
        f12_now = key(0x7B)
        if f12_now and not f12_was:
            running = True
            print("[GO] Starting batch...")
            winsound.Beep(523, 100)
        f12_was = f12_now

    # ── Run each test ──────────────────────────
    for test_idx, test in enumerate(TESTS):
        test_id = test["id"]
        write_config(test)

        with open(CONFIG_PATH, "r", encoding="utf-8") as f:
            cfg = yaml.safe_load(f.read()) or {}
        filter_cfg = cfg.get("filter", {})

        print(f"\n{'='*50}")
        print(f"  TEST {test_idx+1}/{total_tests}: {test_id}")
        print(f"  Rules: {test['rules']}")
        print(f"{'='*50}")

        match_found = False
        attempt = 0

        while not match_found and attempt < MAX_ATTEMPTS_PER_TEST:
            if key(0x7A):
                print("[QUIT]"); ser.close(); return

            attempt += 1

            # Click + Enter
            ser.write(b'C')
            time.sleep(0.8)
            ser.write(b'E')
            time.sleep(1.0)

            # OCR
            result = ocr.scan()
            grade = result["grade"] if result else None
            matched = match_attributes(result["attributes"]) if result else []
            filter_pass, hits, reason = check_filter(matched, filter_cfg)

            if filter_pass:
                match_found = True
                print(f"  [{attempt:03d}] MATCH! {hits}")
                results.append({
                    "test": test_id,
                    "status": "PASS",
                    "attempts": attempt,
                    "grade": grade,
                    "matched": hits,
                    "capture": result.get("capture", ""),
                    "attrs": [m["name"] for m in matched],
                    "timestamp": datetime.now().isoformat(),
                })
                winsound.Beep(1200, 200)
                break
            elif attempt % 20 == 0:
                # Progress every 20 attempts
                attrs = [m["name"] for m in matched[:2]]
                print(f"  [{attempt:03d}] Grade:{grade} Attrs:{attrs} — still hunting...")

        if not match_found:
            print(f"  [!] TIMEOUT after {MAX_ATTEMPTS_PER_TEST} attempts")
            results.append({
                "test": test_id,
                "status": "TIMEOUT",
                "attempts": MAX_ATTEMPTS_PER_TEST,
            })

        # Save intermediate results
        with open(LOG_DIR / "batch_results.json", "w") as f:
            json.dump(results, f, indent=2, ensure_ascii=False)

    # ── Summary ────────────────────────────────
    passed = sum(1 for r in results if r["status"] == "PASS")
    print(f"\n{'='*50}")
    print(f"  BATCH COMPLETE: {passed}/{total_tests} passed")
    print(f"  Results: {LOG_DIR / 'batch_results.json'}")
    print(f"{'='*50}")

    for r in results:
        status = "✅" if r["status"] == "PASS" else "❌"
        cap = Path(r.get("capture", "")).name if r.get("capture") else "?"
        print(f"  {status} {r['test']:15s} #{r.get('attempts','?')}  capture: {cap}")

    ser.close()
    for _ in range(3): winsound.Beep(800, 200); time.sleep(0.1)


if __name__ == "__main__":
    main()
