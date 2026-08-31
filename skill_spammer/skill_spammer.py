"""
Skill Spammer — Auto-press skill keys via Arduino
==================================================
Cycles through configured keys at fixed interval.

Controls: F12 = start/stop, F11 = quit
Web panel: http://127.0.0.1:5001
"""
import sys
import time
import ctypes
import winsound
import json
import threading
from pathlib import Path
import yaml
import serial
import serial.tools.list_ports
from http.server import HTTPServer, BaseHTTPRequestHandler

sys.stdout.reconfigure(encoding='utf-8')

SCRIPT_DIR = Path(__file__).parent
CONFIG_PATH = SCRIPT_DIR / "skill_spammer_config.yaml"
STATE_PATH = SCRIPT_DIR / "spammer_state.json"
user32 = ctypes.windll.user32

def key(vk): return bool(user32.GetAsyncKeyState(vk) & 0x8000)

_F11_PRESSED = False

def sleep_check(sec):
    global _F11_PRESSED
    steps = max(1, int(sec / 0.05))
    for _ in range(steps):
        time.sleep(sec / steps)
        if key(0x7A):
            _F11_PRESSED = True
            return

def find_arduino():
    for p in serial.tools.list_ports.comports():
        if p.vid == 0x2341 and p.pid in (0x8036, 0x8037):
            return p.device
        if "ARDUINO" in p.description.upper():
            return p.device
    return None

# ── Shared state for web panel ──
_shared = {"running": False, "keys": [], "interval": 1.0, "current": "", "count": 0}

class SpammerHandler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass  # quiet

    def do_GET(self):
        if self.path == "/api/state":
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Access-Control-Allow-Origin", "*")
            self.end_headers()
            self.wfile.write(json.dumps(_shared, ensure_ascii=False).encode())
        elif self.path == "/api/start":
            _shared["running"] = True
            self._ok()
        elif self.path == "/api/stop":
            _shared["running"] = False
            self._ok()
        else:
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.send_header("Access-Control-Allow-Origin", "*")
            self.end_headers()
            self.wfile.write(HTML.encode())

    def _ok(self):
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Access-Control-Allow-Origin", "*")
        self.end_headers()
        self.wfile.write(b'{"ok":true}')

HTML = """<!DOCTYPE html>
<html lang="zh">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Skill Spammer</title>
<style>
  * { margin:0; padding:0; box-sizing:border-box; }
  body { font-family:Segoe UI,sans-serif; background:#1a1a2e; color:#eee;
         display:flex; justify-content:center; align-items:center; min-height:100vh; }
  .box { background:#16213e; border-radius:12px; padding:24px; width:340px; text-align:center; }
  h1 { font-size:20px; margin-bottom:12px; }
  .status { font-size:48px; font-weight:bold; margin:16px 0; }
  .running { color:#00e676; } .stopped { color:#ff5252; }
  .info { font-size:14px; color:#aaa; margin:8px 0; }
  .btn { border:none; border-radius:8px; padding:14px 32px; font-size:18px; cursor:pointer;
         margin:8px; font-weight:bold; transition:0.15s; }
  .start { background:#00c853; color:#000; } .start:hover { background:#00e676; }
  .stop  { background:#d50000; color:#fff; } .stop:hover  { background:#ff1744; }
</style>
</head>
<body>
<div class="box">
  <h1>Skill Spammer</h1>
  <div class="status" id="s">STOPPED</div>
  <div class="info" id="keys">Keys: -</div>
  <div class="info" id="cur">Current: -</div>
  <div class="info" id="cnt">Count: 0</div>
  <button class="btn start" onclick="fetch('/api/start')">Start</button>
  <button class="btn stop"  onclick="fetch('/api/stop')">Stop</button>
</div>
<script>
setInterval(async()=>{
  try{ let r=await fetch('/api/state'); let d=await r.json();
    s.textContent=d.running?'RUNNING':'STOPPED';
    s.className='status '+(d.running?'running':'stopped');
    keys.textContent='Keys: '+d.keys.join(' → ');
    cur.textContent='Current: '+(d.current||'-');
    cnt.textContent='Count: '+d.count;
  }catch(e){}
},500);
</script>
</body>
</html>"""


def main():
    global _F11_PRESSED

    # Load or create config
    if CONFIG_PATH.exists():
        with open(CONFIG_PATH, "r", encoding="utf-8") as f:
            cfg = yaml.safe_load(f.read()) or {}
    else:
        cfg = {}

    keys = cfg.get("keys", ["F2", "5", "6"])
    if not keys:
        print("[!] No keys configured — using defaults")
        keys = ["F2", "5", "6"]
    interval = cfg.get("interval", 1.0)

    if not CONFIG_PATH.exists():
        with open(CONFIG_PATH, "w", encoding="utf-8") as f:
            yaml.dump({"keys": keys, "interval": interval}, f, allow_unicode=True)

    _shared["keys"] = keys
    _shared["interval"] = interval

    def write_state():
        try:
            with open(STATE_PATH, "w", encoding="utf-8") as fh:
                json.dump(dict(_shared), fh)
        except Exception:
            pass

    port = find_arduino()
    if not port:
        print("[!] Arduino not found")
        return
    ser = serial.Serial(port, 115200, timeout=1)
    time.sleep(2)
    print(f"[OK] Arduino on {port}")

    # Start web panel (skip if --no-panel)
    no_panel = "--no-panel" in sys.argv
    server = None
    if not no_panel:
        server = HTTPServer(("127.0.0.1", 5001), SpammerHandler)
        t = threading.Thread(target=server.serve_forever, daemon=True)
        t.start()
        print("[Panel] http://127.0.0.1:5001")

    # Control file for launcher
    CONTROL_PATH = SCRIPT_DIR / "spammer_control.txt"

    running = False
    f12_was = key(0x7B)
    idx = 0
    count = 0

    print("\nSkill Spammer")
    print(f"Keys: {' -> '.join(keys)}  |  Interval: {interval}s")
    print("[F12] start/stop  [F11] quit  [Web] http://127.0.0.1:5001\n")

    while True:
        sleep_check(0.05)
        if key(0x7A) or _F11_PRESSED:
            print("[QUIT]")
            break

        # Check control file from launcher
        if CONTROL_PATH.exists():
            try:
                cmd = CONTROL_PATH.read_text(encoding="utf-8").strip()
                if cmd == "start" and not running:
                    running = True
                    _shared["running"] = True
                    _F11_PRESSED = False
                    idx = 0
                    count = 0
                    write_state()
                    print("[Panel] START")
                    winsound.Beep(523, 100)
                elif cmd == "stop" and running:
                    running = False
                    _shared["running"] = False
                    write_state()
                    print("[Panel] STOP")
                    winsound.Beep(1000, 150)
                elif cmd == "quit":
                    print("[Panel] QUIT")
                    CONTROL_PATH.unlink(missing_ok=True)
                    break
                CONTROL_PATH.unlink(missing_ok=True)
            except Exception:
                pass

        # Check web panel commands
        if _shared["running"] and not running:
            running = True
            _F11_PRESSED = False
            idx = 0
            count = 0
            write_state()
            print("[Panel] START")
            winsound.Beep(523, 100)
        elif not _shared["running"] and running:
            running = False
            write_state()
            print("[Panel] STOP")
            winsound.Beep(1000, 150)

        f12_n = key(0x7B)
        if f12_n and not f12_was:
            running = not running
            _shared["running"] = running
            write_state()
            if running:
                _F11_PRESSED = False
                idx = 0
                count = 0
                print(f"[GO] {keys}")
                winsound.Beep(523, 100)
            else:
                print("[STOP]")
                winsound.Beep(1000, 150)
        f12_was = f12_n

        if not running:
            continue

        count += 1
        _shared["count"] = count

        # Press current key
        k = keys[idx]
        _shared["current"] = k
        try:
            if k.startswith("F"):
                n = int(k[1:])
                ser.write(f"F {n}\n".encode())
            else:
                ser.write(f"K {k}\n".encode())
        except (ValueError, serial.SerialException):
            print(f"[!] Invalid key '{k}' or serial error — skipping")

        idx = (idx + 1) % len(keys)

        # Wait for interval, checking keys and panel
        elapsed = 0
        interval_done = False
        while elapsed < interval and not interval_done:
            chunk = min(0.05, interval - elapsed)
            sleep_check(chunk)
            if _F11_PRESSED or key(0x7A):
                running = False
                _shared["running"] = False
                write_state()
                print("[QUIT]")
                ser.close()
                if server:
                    server.shutdown()
                return
            if key(0x7B):
                f12_was = True
                running = False
                _shared["running"] = False
                write_state()
                print("[STOP]")
                winsound.Beep(1000, 150)
                interval_done = True
            elif not _shared["running"] and running:
                running = False
                write_state()
                print("[Panel] STOP")
                winsound.Beep(1000, 150)
                interval_done = True
            else:
                elapsed += chunk

    _shared["running"] = False
    write_state()
    ser.close()
    if server:
        server.shutdown()
    print("\nDone.")


if __name__ == "__main__":
    main()
