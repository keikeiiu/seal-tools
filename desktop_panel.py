"""
Desktop control panel for Seal Tuner — tkinter, no browser needed.
Run alongside seal_tuner.py.
"""
import json
import yaml
from pathlib import Path
import tkinter as tk
from tkinter import ttk

SCRIPT_DIR = Path(__file__).parent
CONFIG_PATH = SCRIPT_DIR / "config.yaml"
STATE_PATH = SCRIPT_DIR / "state.json"
CONTROL_PATH = SCRIPT_DIR / "control.txt"


class TunerPanel:
    def __init__(self):
        self.root = tk.Tk()
        self.root.title("Seal Online 發條 Auto-Tuner")
        self.root.geometry("480x620")
        self.root.resizable(True, True)
        self.root.configure(bg="#1a1a2e")

        # ── Style ─────────────────────────
        self.bg = "#1a1a2e"
        self.card = "#16213e"
        self.accent = "#53d8fb"
        self.green = "#4ecca3"
        self.red = "#e94560"
        self.fg = "#eee"

        # ── Header ────────────────────────
        tk.Label(self.root, text="發條 Auto-Tuner", font=("Segoe UI", 16, "bold"),
                 fg=self.accent, bg=self.bg).pack(pady=(15, 2))
        tk.Label(self.root, text="Seal Online Magic Tuning", font=("Segoe UI", 9),
                 fg="#888", bg=self.bg).pack()

        # ── Status Frame ──────────────────
        status_frame = tk.Frame(self.root, bg=self.bg)
        status_frame.pack(pady=10, padx=15, fill="x")

        self.status_label = tk.Label(status_frame, text="IDLE", font=("Segoe UI", 20, "bold"),
                                     fg="#888", bg=self.card, width=10, height=2,
                                     relief="flat", bd=0)
        self.status_label.pack(fill="x")

        # Stats row
        stats = tk.Frame(status_frame, bg=self.bg)
        stats.pack(fill="x", pady=5)
        for col, (label, key) in enumerate([
            ("Grade", "grade"), ("Attempts", "attempt"), ("Springs", "remaining")
        ]):
            f = tk.Frame(stats, bg=self.card, relief="flat", bd=0)
            f.pack(side="left", expand=True, fill="both", padx=2)
            tk.Label(f, text=label, font=("Segoe UI", 8), fg="#888", bg=self.card).pack()
            val = tk.Label(f, text="--", font=("Segoe UI", 16, "bold"), fg=self.accent, bg=self.card)
            val.pack()
            setattr(self, f"stat_{key}", val)

        # ── Attributes ───────────────────
        attr_frame = tk.Frame(self.root, bg=self.card, relief="flat", bd=0)
        attr_frame.pack(pady=5, padx=15, fill="x")
        tk.Label(attr_frame, text="Attributes", font=("Segoe UI", 10, "bold"),
                 fg=self.fg, bg=self.card).pack(anchor="w", padx=10, pady=(8, 4))

        self.attr_labels = []
        for i in range(3):
            f = tk.Frame(attr_frame, bg=self.card)
            f.pack(fill="x", padx=10, pady=2)
            tk.Label(f, text=f"#{i+1}", font=("Segoe UI", 9),
                     fg="#888", bg=self.card, width=3).pack(side="left")
            name = tk.Label(f, text="--", font=("Segoe UI", 10),
                            fg=self.fg, bg=self.card, anchor="w")
            name.pack(side="left", fill="x", expand=True)
            val = tk.Label(f, text="", font=("Segoe UI", 10, "bold"),
                           fg=self.accent, bg=self.card, width=6, anchor="e")
            val.pack(side="right")
            self.attr_labels.append((name, val))

        # ── Filter status ────────────────
        self.filter_label = tk.Label(attr_frame, text="Filter: --", font=("Segoe UI", 9),
                                     fg="#888", bg=self.card)
        self.filter_label.pack(anchor="w", padx=10, pady=(2, 8))

        # ── Controls ─────────────────────
        ctrl_frame = tk.Frame(self.root, bg=self.bg)
        ctrl_frame.pack(pady=10, padx=15, fill="x")

        btn_style = {"font": ("Segoe UI", 11, "bold"), "width": 8, "height": 1,
                     "relief": "flat", "bd": 0, "cursor": "hand2"}

        tk.Button(ctrl_frame, text="START", bg=self.green, fg="#111",
                  command=self.start, **btn_style).pack(side="left", padx=3)
        tk.Button(ctrl_frame, text="STOP", bg=self.red, fg="#fff",
                  command=self.stop, **btn_style).pack(side="left", padx=3)
        tk.Button(ctrl_frame, text="QUIT", bg="#484f58", fg="#fff",
                  command=self.quit, **btn_style).pack(side="right", padx=3)

        # ── Config ───────────────────────
        cfg_frame = tk.Frame(self.root, bg=self.card, relief="flat", bd=0)
        cfg_frame.pack(pady=5, padx=15, fill="x", expand=True)
        tk.Label(cfg_frame, text="Filter Config", font=("Segoe UI", 10, "bold"),
                 fg=self.fg, bg=self.card).pack(anchor="w", padx=10, pady=(8, 4))

        # Target grade
        f = tk.Frame(cfg_frame, bg=self.card)
        f.pack(fill="x", padx=10)
        tk.Label(f, text="Target:", font=("Segoe UI", 9), fg="#888", bg=self.card).pack(side="left")
        self.cfg_target = ttk.Combobox(f, values=["N", "G", "DG"], width=5, state="readonly")
        self.cfg_target.set("DG")
        self.cfg_target.pack(side="left", padx=5)
        self.cfg_target.bind("<<ComboboxSelected>>", lambda e: self.save_config())

        # Filter on/off
        self.cfg_filter_on = tk.BooleanVar(value=True)
        tk.Checkbutton(f, text="Filter ON", variable=self.cfg_filter_on,
                       bg=self.card, fg=self.fg, selectcolor=self.card,
                       command=self.save_config).pack(side="right")

        # Rules display (read-only for now — edit config.yaml manually)
        self.rules_text = tk.Text(cfg_frame, height=5, font=("Consolas", 9),
                                  bg="#0d1117", fg=self.accent, relief="flat", bd=0)
        self.rules_text.pack(fill="x", padx=10, pady=(5, 10))

        # ── Start refresh ────────────────
        self.refresh()
        self.root.after(500, self.auto_refresh)

    def auto_refresh(self):
        self.refresh()
        self.root.after(500, self.auto_refresh)

    def write_control(self, action):
        CONTROL_PATH.write_text(action + "\n", encoding="utf-8")

    def start(self):
        self.write_control("start")

    def stop(self):
        self.write_control("stop")

    def quit(self):
        self.write_control("quit")
        self.root.after(500, self.root.destroy)

    def save_config(self):
        cfg = {}
        if CONFIG_PATH.exists():
            with open(CONFIG_PATH, "r", encoding="utf-8") as f:
                cfg = yaml.safe_load(f.read()) or {}

        cfg["target_grade"] = self.cfg_target.get()
        cfg["filter"] = cfg.get("filter", {})
        cfg["filter"]["enabled"] = self.cfg_filter_on.get()

        with open(CONFIG_PATH, "w", encoding="utf-8") as f:
            yaml.dump(cfg, f, allow_unicode=True, default_flow_style=False)

        # Update rules display
        rules = cfg["filter"].get("rules", [])
        text = "\n".join(f"  {r.get('name','?')}: {r.get('min','')}-{r.get('max','')} x{r.get('count',1)}"
                         for r in rules)
        self.rules_text.delete("1.0", "end")
        self.rules_text.insert("1.0", text)

    def refresh(self):
        # Read state
        state = {}
        if STATE_PATH.exists():
            try:
                with open(STATE_PATH, "r", encoding="utf-8") as f:
                    state = json.load(f)
            except (FileNotFoundError, json.JSONDecodeError):
                pass

        running = state.get("running", False)
        grade = state.get("grade") or "--"
        attempt = state.get("attempt", 0)
        remaining = state.get("remaining") or "--"
        attrs = state.get("attrs", [])
        filter_status = state.get("filter_status", "--")

        # Update UI
        self.status_label.config(
            text="RUNNING" if running else "IDLE",
            fg=self.green if running else "#888",
            bg="#0d3320" if running else self.card
        )
        self.stat_grade.config(text=grade)
        self.stat_attempt.config(text=str(attempt))
        self.stat_remaining.config(text=str(remaining))

        for i, (name_lbl, val_lbl) in enumerate(self.attr_labels):
            if i < len(attrs):
                m = attrs[i]
                name_lbl.config(text=m.get("name", "--"))
                v = m.get("value")
                val_lbl.config(text=str(v) if v is not None else "")
            else:
                name_lbl.config(text="--")
                val_lbl.config(text="")

        # Filter status color
        fs = filter_status
        color = self.green if "MATCH" in str(fs).upper() or "matched" in str(fs) else (
                self.red if "no match" in str(fs) else "#888")
        self.filter_label.config(text=f"Filter: {fs}", fg=color)

        # Load config on first refresh
        if not hasattr(self, '_loaded'):
            self._loaded = True
            if CONFIG_PATH.exists():
                with open(CONFIG_PATH, "r", encoding="utf-8") as f:
                    cfg = yaml.safe_load(f.read()) or {}
                self.cfg_target.set(cfg.get("target_grade", "DG"))
                self.cfg_filter_on.set(cfg.get("filter", {}).get("enabled", True))
                rules = cfg.get("filter", {}).get("rules", [])
                text = "\n".join(f"  {r.get('name','?')}: {r.get('min','')}-{r.get('max','')} x{r.get('count',1)}"
                                 for r in rules)
                self.rules_text.insert("1.0", text)

    def run(self):
        self.root.mainloop()


if __name__ == "__main__":
    print("Starting desktop panel...")
    print("Make sure seal_tuner.py is running!")
    TunerPanel().run()
