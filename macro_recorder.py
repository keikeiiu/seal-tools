"""
Mouse Macro Recorder/Player for Seal Online (Arduino)
=======================================================
Record mouse movements → save as named macros → replay via Arduino.

Controls:
  F9  = Start recording (5s countdown)
  F9  = Stop recording
  F10 = Replay selected macro
  F11 = Quit

Macros saved in macros/ folder as .json files.

Arduino commands (via serial):
  M <dx> <dy>  — move mouse relatively
  C            — left click
  R            — right click
  W <ms>       — wait milliseconds
"""

import sys
import time
import json
import ctypes
from pathlib import Path
from datetime import datetime
import serial
import serial.tools.list_ports

sys.stdout.reconfigure(encoding='utf-8')

SCRIPT_DIR = Path(__file__).parent
MACRO_DIR = SCRIPT_DIR / "macros"
MACRO_DIR.mkdir(exist_ok=True)

user32 = ctypes.windll.user32

# ── Key codes ──────────────────────────────────
VK_F9 = 0x78
VK_F10 = 0x79
VK_F11 = 0x7A


def is_game_focused():
    """Check if TW_LIVE window has focus."""
    hwnd = user32.GetForegroundWindow()
    length = user32.GetWindowTextLengthW(hwnd)
    buf = ctypes.create_unicode_buffer(length + 1)
    user32.GetWindowTextW(hwnd, buf, length + 1)
    return "TW_LIVE" in buf.value


def focus_game():
    """Bring TW_LIVE to foreground."""
    from ctypes import wintypes
    results = []
    def cb(hwnd, _):
        if user32.IsWindowVisible(hwnd):
            n = user32.GetWindowTextLengthW(hwnd)
            if n:
                b = ctypes.create_unicode_buffer(n+1)
                user32.GetWindowTextW(hwnd, b, n+1)
                if "TW_LIVE" in b.value:
                    results.append(hwnd)
        return True
    WEP = ctypes.WINFUNCTYPE(ctypes.c_bool, wintypes.HWND, ctypes.c_int)
    user32.EnumWindows(WEP(cb), 0)
    if results:
        hwnd = results[0]
        if user32.IsIconic(hwnd):
            user32.ShowWindow(hwnd, 9)
        user32.SetForegroundWindow(hwnd)
        time.sleep(0.2)
        return True
    return False


def key_pressed(vk):
    return bool(user32.GetAsyncKeyState(vk) & 0x8000)


def get_cursor():
    """Get current mouse screen position."""
    pt = ctypes.c_long(), ctypes.c_long()
    user32.GetCursorPos(ctypes.byref(pt[0]), ctypes.byref(pt[1]))
    return (pt[0].value, pt[1].value)


def find_arduino():
    for p in serial.tools.list_ports.comports():
        if p.vid == 0x2341 and p.pid in (0x8036, 0x8037):
            return p.device
        if "ARDUINO" in p.description.upper():
            return p.device
    return None


# ── Record ────────────────────────────────────
def record_macro(ser, name, auto_ref=True):
    """Record mouse movements until F9 pressed again."""
    print(f"\n[REC] Recording: {name}")

    # Auto-calibrate: use game window top-left as reference
    from ocr_engine import find_game_window
    win = find_game_window()
    if win and auto_ref:
        ref_x, ref_y = win[0], win[1]
        print(f"      Auto-ref: game window at ({ref_x}, {ref_y})")
    else:
        ref_x, ref_y = 0, 0
        print("      Reference: screen origin (0,0)")

    print("      5s countdown — position mouse, then move/click")
    for i in range(5, 0, -1):
        print(f"      {i}...", flush=True)
        time.sleep(1)
    print("      [RECORDING] — F9 to stop")

    actions = []
    last_pos = get_cursor()
    last_time = time.time()

    f9_was = key_pressed(VK_F9)
    time.sleep(0.3)  # debounce

    while True:
        time.sleep(0.02)  # 50Hz sampling

        f9_now = key_pressed(VK_F9)
        if f9_now and not f9_was:
            print(f"      [STOP] {len(actions)} actions recorded")
            break
        f9_was = f9_now

        pos = get_cursor()
        now = time.time()
        dt = now - last_time

        # Detect left click
        left = bool(user32.GetAsyncKeyState(0x01) & 0x8000)
        right = bool(user32.GetAsyncKeyState(0x02) & 0x8000)

        if pos != last_pos:
            dx, dy = pos[0] - last_pos[0], pos[1] - last_pos[1]
            if abs(dx) > 0 or abs(dy) > 0:
                actions.append({"type": "move", "dx": dx, "dy": dy, "dt": round(dt, 3)})
                last_pos = pos
                last_time = now
        elif dt > 0.1:
            actions.append({"type": "wait", "ms": int(dt * 1000)})
            last_time = now

        if left:
            actions.append({"type": "click"})
            time.sleep(0.1)
            last_time = time.time()
        elif right:
            actions.append({"type": "rclick"})
            time.sleep(0.1)
            last_time = time.time()

    # Save with reference point
    path = MACRO_DIR / f"{name}.json"
    with open(path, "w", encoding="utf-8") as f:
        json.dump({
            "name": name,
            "created": datetime.now().isoformat(),
            "ref": [ref_x, ref_y],
            "actions": len(actions),
            "data": actions
        }, f, indent=2, ensure_ascii=False)
    print(f"      Saved: {path}")
    return actions


# ── Replay ────────────────────────────────────
def replay_macro(ser, name):
    """Replay a saved macro via Arduino."""
    path = MACRO_DIR / f"{name}.json"
    if not path.exists():
        print(f"[!] Macro not found: {name}")
        return

    with open(path, "r", encoding="utf-8") as f:
        macro = json.load(f)

    data = macro["data"]
    ref = macro.get("ref", [0, 0])
    print(f"\n[PLAY] {name} ({len(data)} actions)")
    print(f"      Ref: ({ref[0]}, {ref[1]})")

    # Focus game if not active — click at game center via Arduino
    if not is_game_focused():
        print("      Game not focused — clicking to focus...")
        user32.SetCursorPos(960, 616)  # game center, definitely in content area
        time.sleep(0.1)
        ser.write(b'C\n')
        time.sleep(0.5)
        print(f"      Focused: {is_game_focused()}")

    # Move cursor to reference point
    user32.SetCursorPos(ref[0], ref[1])
    print("      Cursor at ref. 3s countdown...")
    for i in range(3, 0, -1):
        print(f"      {i}...", flush=True)
        time.sleep(1)
    print("      [PLAYING]")

    for i, action in enumerate(data):
        if key_pressed(VK_F11):
            print("      [ABORTED]")
            return

        t = action["type"]
        if t == "move":
            ser.write(f"M {action['dx']} {action['dy']}\n".encode())
        elif t == "click":
            ser.write(b"C\n")
        elif t == "rclick":
            ser.write(b"R\n")
        elif t == "wait":
            ms = min(action["ms"], 5000)
            ser.write(f"W {ms}\n".encode())

        # Show progress
        if i % 50 == 0:
            print(f"      {i}/{len(data)}")

    print("      [DONE]")


# ── List macros ───────────────────────────────
def list_macros():
    files = sorted(MACRO_DIR.glob("*.json"))
    if not files:
        print("  (no macros saved)")
        return []
    print("\n  Saved macros:")
    for i, f in enumerate(files, 1):
        with open(f, "r", encoding="utf-8") as fp:
            m = json.load(fp)
        print(f"  {i}. {f.stem:30s} ({m.get('actions', '?')} actions)")
    return [f.stem for f in files]


# ── Main ──────────────────────────────────────
def main():
    port = find_arduino()
    if not port:
        print("[!] Arduino not found"); return
    ser = serial.Serial(port, 115200, timeout=1)
    time.sleep(2)
    print(f"[OK] Arduino on {port}")

    print("\nMacro Recorder/Player")
    print("[F9] Record  [F10] Replay  [F11] Quit")
    print("Macros saved in macros/\n")

    macros = list_macros()
    current_macro = macros[0] if macros else None

    f9_was = key_pressed(VK_F9)
    f10_was = key_pressed(VK_F10)

    while True:
        time.sleep(0.05)

        f9_now = key_pressed(VK_F9)
        f10_now = key_pressed(VK_F10)

        if key_pressed(VK_F11):
            print("[QUIT]"); break

        if f9_now and not f9_was:
            name = f"macro_{datetime.now().strftime('%H%M%S')}"
            record_macro(ser, name)
            macros = list_macros()
            if name in macros:
                current_macro = name

        if f10_now and not f10_was:
            macros = list_macros()
            if not macros:
                print("  No macros to replay")
                continue
            # Replay the most recently recorded
            macro_name = macros[-1]
            replay_macro(ser, macro_name)
            current_macro = macro_name

        f9_was = f9_now
        f10_was = f10_now

    ser.close()


if __name__ == "__main__":
    main()
