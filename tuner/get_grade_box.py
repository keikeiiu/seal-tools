"""Recalibrate grade_box — TWO presses of F12."""
import ctypes
import yaml
import time
from pathlib import Path

class POINT(ctypes.Structure):
    _fields_ = [("x", ctypes.c_long), ("y", ctypes.c_long)]

user32 = ctypes.windll.user32

def pos():
    pt = POINT()
    user32.GetCursorPos(ctypes.byref(pt))
    return (pt.x, pt.y)

def key(vk):
    return bool(user32.GetAsyncKeyState(vk) & 0x8000)

print("=== Grade Box Recalibration ===")
print()
print("Step 1: Hover over TOP-LEFT of grade letter -> press F12")
print("Step 2: Hover over BOTTOM-RIGHT of grade letter -> press F12")
print("F11 = quit")
print()

f12_was = key(0x7B)
corners = []

while len(corners) < 2:
    time.sleep(0.05)
    if key(0x7A): print("Quit."); exit()
    f12_now = key(0x7B)
    if f12_now and not f12_was:
        x, y = pos()
        corners.append((x, y))
        print(f"Captured #{len(corners)}: ({x}, {y})")
        if len(corners) == 2:
            break
    f12_was = f12_now

x1, y1 = corners[0]
x2, y2 = corners[1]
box = [min(x1,x2), min(y1,y2), max(x1,x2), max(y1,y2)]

config_path = Path(__file__).parent / "config.yaml"
cfg = {}
if config_path.exists():
    with open(config_path) as f:
        cfg = yaml.safe_load(f) or {}
cfg["grade_box"] = box
with open(config_path, "w") as f:
    yaml.dump(cfg, f, allow_unicode=True)

print(f"\ngrade_box: {box}")
print("Saved to config.yaml!")
input("Press ENTER to exit...")
