"""
Seal Tools Launcher — Unified web panel for all Arduino tools
==============================================================
Start/stop tuner, gem composer, skill spammer from one panel.

Only one tool runs at a time (shared Arduino COM port).
"""
import sys
import os
import time
import json
import subprocess
import threading
from pathlib import Path
import yaml
from flask import Flask, jsonify, request, send_from_directory

sys.stdout.reconfigure(encoding='utf-8')

SCRIPT_DIR = Path(__file__).parent

app = Flask(__name__)

TOOLS = {
    "tuner": {
        "name": "Magic Tuner",
        "script": "tuner/seal_tuner.py",
        "state_file": "tuner/state.json",
        "control_file": "tuner/control.txt",
        "port": 5000,
        "icon": "🎯",
        "flags": ["--no-browser"],
    },
    "gem": {
        "name": "Gem Composer",
        "script": "gem_composer/gem_composer.py",
        "state_file": "gem_composer/gem_state.json",
        "control_file": "gem_composer/gem_control.txt",
        "port": None,
        "icon": "💎",
    },
    "spammer": {
        "name": "Skill Spammer",
        "script": "skill_spammer/skill_spammer.py",
        "state_file": "skill_spammer/spammer_state.json",
        "control_file": "skill_spammer/spammer_control.txt",
        "port": 5001,
        "icon": "⚔️",
        "flags": ["--no-panel"],
    },
}

# ── Process management ──
_current_tool = None
_current_proc = None
_err_log = None
_lock = threading.Lock()


def kill_current():
    global _current_proc, _current_tool
    with _lock:
        if _current_proc and _current_proc.poll() is None:
            # Kill entire process tree to free COM port
            try:
                subprocess.run(["taskkill", "/F", "/T", "/PID", str(_current_proc.pid)],
                               capture_output=True, timeout=3)
            except Exception:
                _current_proc.kill()
            try:
                _current_proc.wait(timeout=2)
            except subprocess.TimeoutExpired:
                pass
        _current_proc = None
        _current_tool = None
    time.sleep(0.3)  # Let COM port release


def start_tool(tool_id):
    global _current_proc, _current_tool
    kill_current()
    tool = TOOLS[tool_id]
    # Clear any stale control command left over from a previous run
    cf = tool.get("control_file")
    if cf:
        (SCRIPT_DIR / cf).unlink(missing_ok=True)
    script = str(SCRIPT_DIR / tool["script"])
    flags = tool.get("flags", [])
    global _err_log
    if _err_log:
        _err_log.close()
    _err_log = open(str(SCRIPT_DIR / "tool_error.log"), "a", encoding="utf-8")
    with _lock:
        _current_proc = subprocess.Popen(
            [sys.executable, "-u", script] + flags,
            cwd=str(SCRIPT_DIR),
            stdout=_err_log,
            stderr=_err_log,
            creationflags=subprocess.CREATE_NEW_PROCESS_GROUP,
            env={**os.environ, "PYTHONUNBUFFERED": "1"},
        )
        _current_tool = tool_id
    # Verify process didn't crash immediately
    time.sleep(1)
    with _lock:
        if _current_proc is not None and _current_proc.poll() is not None:
            print(f"[!] {tool['name']} crashed on start (exit {_current_proc.returncode})")
            print("    Check tool_error.log for details")
            _current_proc = None
            _current_tool = None
            return False
    print(f"[+] Started {tool['name']}")
    return True


def get_tool_status(tool_id):
    global _current_tool, _current_proc
    with _lock:
        if _current_tool != tool_id or _current_proc is None:
            return {"running": False}
        if _current_proc.poll() is not None:
            _current_tool = None
            _current_proc = None
            return {"running": False}
        return {"running": True}


# ── Routes ──
@app.route("/")
def index():
    return send_from_directory(str(SCRIPT_DIR), "launcher.html")


@app.route("/api/tools")
def api_tools():
    tools = {}
    for tid, t in TOOLS.items():
        st = get_tool_status(tid)
        extra = {}
        if t["state_file"]:
            sf = SCRIPT_DIR / t["state_file"]
            if sf.exists():
                try:
                    extra = json.loads(sf.read_text(encoding="utf-8"))
                except Exception:
                    pass
        process_alive = st["running"]
        tool_active = extra.get("running", False) if process_alive else False
        # Gem composer: when not loaded, show the configured start_grade, not the
        # stale grade left in the state file from a previous run.
        if tid == "gem" and not process_alive:
            try:
                with open(SCRIPT_DIR / "gem_composer/gem_composer_config.yaml", "r", encoding="utf-8") as f:
                    gem_cfg = yaml.safe_load(f.read()) or {}
                extra["grade"] = gem_cfg.get("start_grade", "N")
                extra["cycle"] = 0
            except (yaml.YAMLError, OSError):
                pass
        tools[tid] = {
            "name": t["name"],
            "icon": t["icon"],
            "port": t["port"],
            **extra,
            "loaded": process_alive,    # process is alive (authoritative — overrides stale state)
            "running": tool_active,     # actually working (authoritative — overrides stale state)
        }
    return jsonify(tools)


@app.route("/api/start/<tool_id>")
def api_start(tool_id):
    if tool_id not in TOOLS:
        return jsonify({"error": "unknown tool"}), 404
    success = start_tool(tool_id)
    if not success:
        return jsonify({"ok": False, "error": "Tool crashed on startup. Check tool_error.log"}), 500
    return jsonify({"ok": True, "tool": tool_id})


@app.route("/api/stop")
def api_stop():
    kill_current()
    return jsonify({"ok": True})

@app.route("/api/control/<tool_id>/<action>")
def api_control(tool_id, action):
    """Send start/stop/quit to a running tool via its control file."""
    if tool_id not in TOOLS:
        return jsonify({"error": "unknown tool"}), 404
    if action not in ("start", "stop", "quit"):
        return jsonify({"error": "unknown action"}), 400
    cf = TOOLS[tool_id].get("control_file")
    if not cf:
        return jsonify({"error": "no control file for this tool"}), 400
    (SCRIPT_DIR / cf).write_text(action, encoding="utf-8")
    # For quit: also force-kill after grace period (prevents zombies)
    if action == "quit":
        def _force_kill():
            time.sleep(1.5)
            global _current_proc, _current_tool
            with _lock:
                if _current_proc and _current_proc.poll() is None:
                    try:
                        subprocess.run(["taskkill", "/F", "/T", "/PID", str(_current_proc.pid)],
                                       capture_output=True, timeout=3)
                    except Exception:
                        pass
                    _current_proc = None
                    _current_tool = None
        threading.Thread(target=_force_kill, daemon=True).start()
    return jsonify({"ok": True, "tool": tool_id, "action": action})


# ── Spammer Config ──
SPAMMER_CONFIG = SCRIPT_DIR / "skill_spammer/skill_spammer_config.yaml"

def load_spammer_config():
    if SPAMMER_CONFIG.exists():
        with open(SPAMMER_CONFIG, "r", encoding="utf-8") as f:
            return yaml.safe_load(f.read()) or {}
    return {"keys": ["F2", "5", "6"], "interval": 1.0}

def save_spammer_config(cfg):
    with open(SPAMMER_CONFIG, "w", encoding="utf-8") as f:
        yaml.dump(cfg, f, allow_unicode=True)

@app.route("/api/spammer_config", methods=["GET", "POST"])
def spammer_config():
    if request.method == "POST":
        data = request.get_json()
        if data:
            cfg = {"keys": data.get("keys", ["F2", "5", "6"]),
                   "interval": float(data.get("interval", 1.0))}
            save_spammer_config(cfg)
            return jsonify({"ok": True, "config": cfg})
    return jsonify(load_spammer_config())


# ── Tuner Config ──
TUNER_CONFIG = SCRIPT_DIR / "tuner/config.yaml"

@app.route("/api/tuner_config", methods=["GET", "POST"])
def tuner_config_route():
    if request.method == "POST":
        data = request.get_json()
        cfg = {}
        if TUNER_CONFIG.exists():
            try:
                with open(TUNER_CONFIG, "r", encoding="utf-8") as f:
                    cfg = yaml.safe_load(f.read()) or {}
            except (yaml.YAMLError, OSError):
                cfg = {}
        if "filter" in data:
            cfg["filter"] = data["filter"]
        if "target_grade" in data:
            cfg["target_grade"] = data["target_grade"]
        if "timing" in data:
            cfg["timing"] = data["timing"]
        if "max_retries" in data:
            try:
                cfg["max_retries"] = int(data["max_retries"])
            except (ValueError, TypeError):
                pass  # keep existing value
        with open(TUNER_CONFIG, "w", encoding="utf-8") as f:
            yaml.dump(cfg, f, allow_unicode=True, default_flow_style=False)
        return jsonify({"ok": True})
    cfg = {}
    if TUNER_CONFIG.exists():
        try:
            with open(TUNER_CONFIG, "r", encoding="utf-8") as f:
                cfg = yaml.safe_load(f.read()) or {}
        except (yaml.YAMLError, OSError):
            cfg = {}
    return jsonify({
        "filter": cfg.get("filter", {}),
        "timing": cfg.get("timing", {}),
        "target_grade": cfg.get("target_grade", "DG"),
        "max_retries": cfg.get("max_retries", 999999),
    })

@app.route("/api/attr_names")
def api_attr_names():
    from tuner.attr_matcher import ATTRIBUTES
    all_names = [n for n, _, _ in ATTRIBUTES]
    return jsonify(all_names)


# ── Gem Config ──
GEM_CONFIG = SCRIPT_DIR / "gem_composer/gem_composer_config.yaml"

@app.route("/api/gem_config", methods=["GET", "POST"])
def gem_config_route():
    cfg = {}
    if GEM_CONFIG.exists():
        try:
            with open(GEM_CONFIG, "r", encoding="utf-8") as f:
                cfg = yaml.safe_load(f.read()) or {}
        except (yaml.YAMLError, OSError):
            cfg = {}

    if request.method == "POST":
        data = request.get_json()
        if data and "start_grade" in data:
            cfg["start_grade"] = data["start_grade"]
            with open(GEM_CONFIG, "w", encoding="utf-8") as f:
                yaml.dump(cfg, f, allow_unicode=True)
            return jsonify({"ok": True})

    return jsonify({
        "start_grade": cfg.get("start_grade", "N"),
    })


# ── Main ──
def main():
    print("Seal Tools Launcher")
    print("http://127.0.0.1:5002\n")

    # Kill any leftover state
    kill_current()

    try:
        app.run(host="127.0.0.1", port=5002, debug=False)
    except KeyboardInterrupt:
        kill_current()


if __name__ == "__main__":
    main()
