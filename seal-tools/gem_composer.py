"""
Gem Composer — Auto gem combination
=====================================
Uses calibrated D movements from gem_composer_config.yaml

Controls: F12 = start/stop, F11 = quit, F9 = advance grade
"""
import sys
import time
import ctypes
import winsound
import json
from pathlib import Path
import yaml
import serial
import serial.tools.list_ports

sys.stdout.reconfigure(encoding='utf-8')

SCRIPT_DIR = Path(__file__).parent
user32 = ctypes.windll.user32

# Load config with defaults for missing file/keys
_CFG_PATH = SCRIPT_DIR / "gem_composer_config.yaml"
if _CFG_PATH.exists():
    with open(_CFG_PATH, "r", encoding="utf-8") as fh:
        CFG = yaml.safe_load(fh.read()) or {}
else:
    CFG = {}

CFG.setdefault("grade_positions", {
    "N": [727, 696], "G": [777, 696], "DG": [827, 696],
    "Register": [835, 724], "Combine": [785, 871],
})
CFG.setdefault("movements", {
    "radio_to_register": {"N": [65, 20], "G": [35, 20], "DG": [10, 20]},
    "register_combine": [-30, 80], "combine_register": [30, -80],
    "grade_next": [33, 0], "grade_prev": [-33, 0],
    "register_slot1": [-45, -100], "slot1_slot2": [36, 0], "slot2_dg": [8, 87],
})
CFG.setdefault("start_grade", "N")

VK_F12 = 0x7B
VK_F11 = 0x7A
VK_F9  = 0x78

def key(vk): return bool(user32.GetAsyncKeyState(vk) & 0x8000)

class QuitException(Exception):
    pass

def sleep_check(sec, check_f12=False):
    """Sleep in small chunks, checking keys. Raises QuitException on F11."""
    steps = max(1, int(sec / 0.05))
    for _ in range(steps):
        time.sleep(sec / steps)
        if key(VK_F11):
            raise QuitException()
        if check_f12 and key(VK_F12):
            return True  # toggle requested
    return False

def find_arduino():
    for p in serial.tools.list_ports.comports():
        if p.vid == 0x2341 and p.pid in (0x8036, 0x8037):
            return p.device
        if "ARDUINO" in p.description.upper():
            return p.device
    return None


def main():
    port = find_arduino()
    if not port:
        print("[!] Arduino not found")
        return
    ser = serial.Serial(port, 115200, timeout=1)
    time.sleep(2)
    print("[OK] Arduino on {}".format(port))

    # Current grade — read from config or default N
    grades = ["N", "G", "DG"]
    start_grade = CFG.get("start_grade", "N")
    gidx = grades.index(start_grade) if start_grade in grades else 0
    running = False
    f12_was = key(VK_F12)
    f9_was = key(VK_F9)

    print("\nGem Composer")
    print("[F12] start/stop  [F9] advance grade  [F11] quit\n")

    # Control file for launcher trigger + state for UI
    CONTROL_PATH = SCRIPT_DIR / "gem_control.txt"
    STATE_PATH = SCRIPT_DIR / "gem_state.json"

    def write_state(active):
        try:
            with open(STATE_PATH, "w", encoding="utf-8") as fh:
                json.dump({"running": active, "grade": grades[gidx], "cycle": cycle}, fh)
        except Exception:
            pass

    # Cursor positioned on-demand when starting (not on load)

    cycle = 0
    write_state(False)  # reset stale "running" state on load so launcher shows IDLE

    try:
        while True:
            sleep_check(0.05, check_f12=True)
            if key(VK_F11):
                raise QuitException()

            # Check control file from launcher
            if CONTROL_PATH.exists():
                try:
                    cmd = CONTROL_PATH.read_text(encoding="utf-8").strip()
                    if cmd == "quit":
                        print("[Panel] QUIT")
                        CONTROL_PATH.unlink(missing_ok=True)
                        raise QuitException()
                    elif cmd == "start" and not running:
                        running = True
                        f12_was = True  # prevent F12 toggle immediately after panel start
                        write_state(True)
                        print("[Panel] START")
                        winsound.Beep(523, 100)
                        # First cycle setup: select grade + register
                        gx, gy = CFG["grade_positions"][grades[gidx]]
                        user32.SetCursorPos(gx, gy)
                        sleep_check(0.3)
                        ser.write(b'C\n')
                        sleep_check(0.5)
                        d = CFG["movements"]["radio_to_register"][grades[gidx]]
                        ser.write('D {} {}\n'.format(*d).encode())
                        sleep_check(0.3)
                        ser.write(b'C\n')
                        sleep_check(0.5)
                    elif cmd == "stop" and running:
                        running = False
                        write_state(False)
                        print("[Panel] STOP")
                        winsound.Beep(1000, 150)
                    CONTROL_PATH.unlink(missing_ok=True)
                except QuitException:
                    raise  # re-raise to outer handler
                except Exception:
                    pass

            f12_n = key(VK_F12)
            f9_n = key(VK_F9)

            if f12_n and not f12_was:
                running = not running
                write_state(running)
                if running:
                    print("[GO] Grade: {}".format(grades[gidx]))
                    winsound.Beep(523, 100)
                    # First cycle setup: select grade + register
                    gx, gy = CFG["grade_positions"][grades[gidx]]
                    user32.SetCursorPos(gx, gy)
                    sleep_check(0.3)
                    ser.write(b'C\n')
                    sleep_check(0.5)
                    d = CFG["movements"]["radio_to_register"][grades[gidx]]
                    ser.write('D {} {}\n'.format(*d).encode())
                    sleep_check(0.3)
                    ser.write(b'C\n')
                    sleep_check(0.5)
                else:
                    print("[STOP]")
                    winsound.Beep(1000, 150)

            if f9_n and not f9_was:
                gidx = (gidx + 1) % 3
                print("[GRADE] -> {}".format(grades[gidx]))
                # Select new grade
                gx, gy = CFG["grade_positions"][grades[gidx]]
                user32.SetCursorPos(gx, gy)
                sleep_check(0.3)
                ser.write(b'C\n')
                sleep_check(0.5)
                # Move to Register
                d = CFG["movements"]["radio_to_register"][grades[gidx]]
                ser.write('D {} {}\n'.format(*d).encode())
                sleep_check(0.3)
                ser.write(b'C\n')
                write_state(running)

            f12_was = f12_n
            f9_was = f9_n

            if not running:
                continue

            cycle += 1
            write_state(True)  # keep panel cycle count live

            # Check keys before each serial/mouse action
            if key(VK_F11):
                raise QuitException()
            if key(VK_F12):
                # Toggle stop mid-cycle
                f12_was = True  # prevent re-trigger on next loop
                running = False
                write_state(False)
                print("[STOP]")
                winsound.Beep(1000, 150)
                continue

            # Combine
            d = CFG["movements"]["register_combine"]
            ser.write('D {} {}\n'.format(*d).encode())
            sleep_check(0.2)
            ser.write(b'C\n')
            sleep_check(0.8)

            # Back to Register, dereg + register
            if key(VK_F11):
                raise QuitException()
            d = CFG["movements"]["combine_register"]
            ser.write('D {} {}\n'.format(*d).encode())
            sleep_check(0.2)
            ser.write(b'C\n')  # deregister
            sleep_check(0.3)
            ser.write(b'C\n')  # register new
            sleep_check(0.5)

            if cycle % 10 == 0:
                print("  Cycle: {}".format(cycle))

    except QuitException:
        print("[QUIT]")
    except Exception:
        print("[!] Unexpected error:", sys.exc_info()[1])
    finally:
        write_state(False)  # clean exit — never leave stale "running" behind
        ser.close()
    print("\nDone. {} cycles.".format(cycle))


if __name__ == "__main__":
    main()
