"""
Seal Online 發條 Auto-Tuner
==============================
Arduino click+Enter + OCR grade detection. Auto-stops at DG.

Controls: F12 = start, F11 = quit
"""
import sys
import time
import ctypes
import winsound
import json
from pathlib import Path
from datetime import datetime
import serial
import serial.tools.list_ports
from ocr_engine import TuningOCR
from attr_matcher import match_attributes, check_filter
from web_panel import start_panel
import webbrowser
import yaml

sys.stdout.reconfigure(encoding='utf-8')

SCRIPT_DIR = Path(__file__).parent
CONFIG_PATH = SCRIPT_DIR / "config.yaml"
STATE_PATH = SCRIPT_DIR / "state.json"
CONTROL_PATH = SCRIPT_DIR / "control.txt"
LOG_DIR = SCRIPT_DIR / "logs"
LOG_DIR.mkdir(exist_ok=True)
user32 = ctypes.windll.user32

GRADE_ORDER = {"N": 0, "G": 1, "DG": 2, "XG": 3, "SG": 4}


def find_arduino():
    for p in serial.tools.list_ports.comports():
        if p.vid == 0x2341 and p.pid in (0x8036, 0x8037):
            return p.device
        if "ARDUINO" in p.description.upper():
            return p.device
    return None


def key(vk): return bool(user32.GetAsyncKeyState(vk) & 0x8000)

_F11_PRESSED = False

def sleep_check(sec):
    """Sleep in small chunks, checking F11 each tick. Sets _F11_PRESSED if detected."""
    global _F11_PRESSED
    steps = max(1, int(sec / 0.05))
    for _ in range(steps):
        time.sleep(sec / steps)
        if key(0x7A):
            _F11_PRESSED = True
            return


def main():
    global _F11_PRESSED
    # Arduino
    port = find_arduino()
    if not port:
        print("[!] Arduino not found")
        for p in serial.tools.list_ports.comports():
            print(f"    {p.device} - {p.description}")
        return
    ser = serial.Serial(port, 115200, timeout=1)
    time.sleep(2)
    print(f"[OK] Arduino on {port}")

    # OCR engine
    print("[i] Loading OCR...")
    ocr = TuningOCR()
    ocr._init_ocr()
    print("[OK] Ready")

    # Start web panel (always), skip browser auto-open if --no-browser
    no_browser = "--no-browser" in sys.argv
    start_panel()
    if not no_browser:
        webbrowser.open("http://127.0.0.1:5000")

    # Log
    timestamp = datetime.now().strftime('%Y%m%d_%H%M%S')
    run_log = LOG_DIR / f"run_{timestamp}.jsonl"
    txt_log = LOG_DIR / f"run_{timestamp}.txt"

    if CONFIG_PATH.exists():
        with open(CONFIG_PATH, "r", encoding="utf-8") as fh:
            cfg = yaml.safe_load(fh.read()) or {}
    else:
        cfg = {}
    print(f"\nTarget: {cfg.get('target_grade', 'DG')}")
    print("Controls: [F12] start  [F11] quit")
    print("Position mouse over confirm button -> F12\n")

    def _read_state():
        if STATE_PATH.exists():
            try:
                with open(STATE_PATH, "r", encoding="utf-8") as f:
                    return json.load(f)
            except (json.JSONDecodeError, OSError):
                pass
        return {}

    running = False
    countdown = 0
    attempt = 0
    f12_was = key(0x7B)
    prev_sig = None   # for detecting stuck rolls (out of springs → same screen)
    same_count = 0

    while True:
        sleep_check(0.05)

        if key(0x7A) or _F11_PRESSED:
            print("[QUIT]")
            break

        # Check web panel control commands
        if CONTROL_PATH.exists():
            try:
                cmd = CONTROL_PATH.read_text(encoding="utf-8").strip()
                if cmd == "start" and not running:
                    running = True
                    countdown = 5
                    print("[Panel] START")
                    # Merge: keep last known attrs if available
                    st = _read_state()
                    st.update({"running": True, "attempt": attempt})
                    with open(STATE_PATH, "w", encoding="utf-8") as f:
                        json.dump(st, f)
                elif cmd == "stop" and running:
                    running = False
                    print("[Panel] STOP")
                    st = _read_state()
                    st.update({"running": False, "attempt": attempt})
                    with open(STATE_PATH, "w", encoding="utf-8") as f:
                        json.dump(st, f)
                elif cmd == "quit":
                    print("[Panel] QUIT")
                    CONTROL_PATH.unlink(missing_ok=True)
                    break
                CONTROL_PATH.unlink(missing_ok=True)
            except Exception:
                pass

        f12_now = key(0x7B)
        if f12_now and not f12_was:
            # Don't override F11 quit signal
            if _F11_PRESSED:
                continue
            running = not running
            if running:
                _F11_PRESSED = False
                print("[GO]")
                countdown = 5
                winsound.Beep(523, 100)
            else:
                print("[STOP]")
                countdown = 0
                winsound.Beep(1000, 150)
        f12_was = f12_now

        # Countdown
        if countdown > 0:
            print(f"  {countdown}...", flush=True)
            for _ in range(20):
                sleep_check(0.05)
                if key(0x7B):
                    print("[STOP]")
                    countdown = 0
                    running = False
                    break
            countdown -= 1
            if countdown == 0:
                print("[>] RUNNING")
                winsound.Beep(1500, 150)
            continue

        if not running:
            continue

        attempt += 1

        # Arduino: click + Enter (with configurable timing)
        t = cfg.get("timing", {})
        try:
            ser.write(b'C\n')
            sleep_check(t.get("click_enter_delay", 0.8))
            if _F11_PRESSED:
                break
            ser.write(b'E\n')
            sleep_check(t.get("ocr_delay", 1.0))
            if _F11_PRESSED:
                break
        except serial.SerialException:
            print("[!] Arduino disconnected — stopping")
            running = False
            break
        sleep_check(t.get("ocr_delay", 1.0))
        if _F11_PRESSED:
            break

        # OCR scan
        result = ocr.scan()
        grade = result["grade"] if result else None
        remaining = result.get("remaining") if result else None

        # Match attributes to standard names + check filter
        matched = match_attributes(result["attributes"]) if result else []
        # Load filter (guard against concurrent writes from launcher)
        try:
            with open(CONFIG_PATH, "r", encoding="utf-8") as f:
                cfg = yaml.safe_load(f.read()) or {}
        except (yaml.YAMLError, OSError):
            pass  # keep previous cfg, skip this iteration's filter check
        filter_cfg = cfg.get("filter", {})
        filter_pass, _filter_hits, filter_reason = check_filter(matched, filter_cfg)

        # Detect stuck rolls (out of springs → same result read repeatedly)
        sig = (grade, remaining, tuple((m["name"], m["value"]) for m in matched))
        if sig == prev_sig:
            same_count += 1
        else:
            same_count = 0
        prev_sig = sig

        # Display — 5 data points
        print(f"\n[{attempt:04d}] {'='*35}")
        if result:
            print(f"  Grade:     {grade or '?'}")
            print(f"  Remaining: {remaining or '?'}")
            for i, m in enumerate(matched, 1):
                val_str = f" = {m['value']}" if m['value'] is not None else ""
                print(f"  Attr {i}:    {m['name']}{val_str}")
            if filter_cfg.get("enabled"):
                status = "MATCH" if filter_pass else "no match"
                print(f"  Filter:    {status} ({filter_reason})")
        else:
            print("  Grade: ? (window not found)")
            remaining = None

        # Write state for web panel
        with open(STATE_PATH, "w", encoding="utf-8") as f:
            state = {
                "running": running,
                "attempt": attempt,
                "grade": grade,
                "remaining": remaining,
                "attrs": [{"name": m["name"], "value": m["value"]} for m in matched],
                "filter_status": filter_reason if filter_cfg.get("enabled") else "disabled",
            }
            json.dump(state, f, ensure_ascii=False)

        # Brief pause so frontend can display before next click
        sleep_check(0.3)
        if _F11_PRESSED:
            break

        # Stop condition: grade + filter
        target = cfg.get("target_grade", "DG")
        require_grade = filter_cfg.get("require_grade", None)
        # Normalize: "false" string from YAML → treated as no requirement
        if require_grade and str(require_grade).lower() != "false":
            effective_grade = require_grade  # filter's require_grade takes priority
        else:
            effective_grade = target
        filter_ok = filter_pass if filter_cfg.get("enabled") else True
        grade_ok = grade and GRADE_ORDER.get(grade, -1) >= GRADE_ORDER.get(effective_grade, 99)

        if grade_ok and filter_ok:
            reason = "+ FILTER MATCH" if filter_cfg.get("enabled") else "(no filter)"
            print(f"\n  >>> {grade} REACHED {reason} <<<")
            running = False
            for _ in range(5):
                winsound.Beep(1200, 200)
                sleep_check(0.1)
            break
        elif not require_grade and filter_pass:
            print(f"\n  >>> FILTER MATCHED at grade {grade} (no grade requirement) <<<")
            running = False
            for _ in range(5):
                winsound.Beep(1200, 200)
                sleep_check(0.1)
            break
        elif grade_ok and not filter_pass:
            print(f"  >>> {grade} reached but filter not met ({filter_reason}) — continuing <<<")
        elif filter_pass and not grade_ok and require_grade:
            print(f"  Filter matched but grade {grade} < {require_grade} — continuing <<<")

        # Stop if out of springs
        if remaining is not None and remaining <= 0:
            print("\n  >>> OUT OF SPRINGS <<<")
            running = False
            break

        # Stop if stuck — same roll result repeated (out of springs → screen frozen)
        if same_count >= 2:
            print("\n  >>> STUCK / OUT OF SPRINGS (same result x3) <<<")
            running = False
            break

        # Save text log (Option B — compact, readable)
        try:
            if result:
                attrs = []
                for a in result.get('attributes', []):
                    if a and isinstance(a, (list, tuple)) and len(a) > 0:
                        clean = str(a[0]).replace(' | ', ' ')
                        for item in a[1:3]:
                            item = str(item).strip().lstrip('+- ')
                            if item and len(item) <= 3:
                                clean = clean.rstrip(' +-0123456789') + item
                        attrs.append(clean)
                txt_line = f"{attempt:03d} {grade or '?'} {remaining or '?'} | " + " | ".join(attrs)
                with open(txt_log, "a", encoding="utf-8") as f:
                    f.write(txt_line + "\n")
        except (OSError, AttributeError, IndexError):
            pass  # don't crash on log write failure

        # Save to JSON log
        try:
            if result:
                with open(run_log, "a", encoding="utf-8") as f:
                    entry = {
                        "attempt": attempt,
                        "grade": grade,
                        "grade_line": result.get("grade_line", []),
                        "attributes": result.get("attributes", []),
                    }
                    f.write(json.dumps(entry, ensure_ascii=False) + "\n")
        except OSError:
            pass  # don't crash on log write failure

        max_r = cfg.get("max_retries", 500)
        if attempt >= max_r:
            print(f"[!] Max {max_r}")
            running = False
            break

        if key(0x7A) or _F11_PRESSED:
            print("[QUIT]")
            break

    with open(STATE_PATH, "w", encoding="utf-8") as f:
        json.dump({"running": False, "attempt": attempt}, f)
    ser.close()
    print(f"\nDone. {attempt} attempts. Log: {run_log}")


if __name__ == "__main__":
    main()
