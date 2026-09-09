"""Flask Control Panel — http://localhost:5000"""
import json
import yaml
from pathlib import Path
from flask import Flask, render_template_string, request, jsonify

SCRIPT_DIR = Path(__file__).parent
CONFIG_PATH = SCRIPT_DIR / "config.yaml"
STATE_PATH = SCRIPT_DIR / "state.json"
CONTROL_PATH = SCRIPT_DIR / "control.txt"

app = Flask(__name__)

HTML = r"""<!DOCTYPE html>
<html lang="zh">
<head>
<meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>發條 Tuner</title>
<style>
*{box-sizing:border-box;margin:0;padding:0}
body{font:14px Segoe UI,sans-serif;background:#0d1117;color:#c9d1d9;padding:20px;max-width:900px;margin:auto}
h1{font-size:20px;color:#58a6ff;margin-bottom:4px}
h3{font-size:14px;color:#8b949e;margin:12px 0 6px}
.card{background:#161b22;border:1px solid #30363d;border-radius:6px;padding:16px;margin:10px 0}
.row{display:flex;gap:12px;flex-wrap:wrap;align-items:center}
.stat{flex:1;min-width:100px;background:#0d1117;border:1px solid #30363d;border-radius:4px;padding:10px;text-align:center}
.stat .val{font-size:22px;font-weight:700}
.stat .lbl{font-size:11px;color:#8b949e;margin-top:2px}
.live{border-color:#58a6ff;box-shadow:0 0 8px #58a6ff44}
.good{color:#3fb950}.warn{color:#d2991d}.bad{color:#f85149}
.btn{padding:8px 18px;border:none;border-radius:4px;font-size:13px;font-weight:600;cursor:pointer}
.btn-go{background:#238636;color:#fff}.btn-go:hover{background:#2ea043}
.btn-stop{background:#da3633;color:#fff}.btn-stop:hover{background:#f85149}
.btn-kill{background:#484f58;color:#fff}
table{width:100%;border-collapse:collapse;font-size:13px}
td,th{padding:6px 10px;text-align:left;border-bottom:1px solid #21262d}
th{color:#8b949e;font-weight:400}
select,input[type=number]{background:#0d1117;border:1px solid #30363d;color:#c9d1d9;padding:5px 8px;border-radius:4px;font-size:13px}
select{min-width:160px} input[type=number]{width:60px}
.rule-line{display:flex;gap:8px;align-items:center;margin:4px 0}
.note{font-size:11px;color:#8b949e;margin-top:4px}
.flex{display:flex;gap:8px;align-items:center}
</style>
</head>
<body>
<h1>&#x767C;&#x689D; Auto-Tuner</h1>
<div style="color:#8b949e;font-size:12px;margin-bottom:16px">localhost:5000 | Seal Online Magic Tuning</div>

<!-- Status -->
<div class="card">
  <div class="row">
    <div class="stat" id="stat_status"><div class="val">--</div><div class="lbl">Status</div></div>
    <div class="stat"><div class="val" id="stat_grade">--</div><div class="lbl">Grade</div></div>
    <div class="stat"><div class="val" id="stat_attempt">0</div><div class="lbl">Attempts</div></div>
    <div class="stat"><div class="val" id="stat_remaining">--</div><div class="lbl">Springs</div></div>
    <div class="stat"><div class="val" id="stat_filter">--</div><div class="lbl">Filter</div></div>
  </div>
</div>

<!-- Attributes -->
<div class="card">
  <h3>Current Attributes</h3>
  <table>
    <tr><th>#</th><th>Attribute</th><th>Value</th></tr>
    <tr><td>1</td><td id="a1_name">--</td><td id="a1_val"></td></tr>
    <tr><td>2</td><td id="a2_name">--</td><td id="a2_val"></td></tr>
    <tr><td>3</td><td id="a3_name">--</td><td id="a3_val"></td></tr>
  </table>
</div>

<!-- Controls -->
<div class="card">
  <h3>Controls</h3>
  <div class="flex">
    <button class="btn btn-go" onclick="ctrl('start')">&#x25B6; Start</button>
    <button class="btn btn-stop" onclick="ctrl('stop')">&#x25A0; Stop</button>
    <button class="btn btn-kill" onclick="ctrl('quit')">Quit</button>
  </div>
</div>

<!-- Filter Config -->
<div class="card">
  <h3>Filter Rules</h3>
  <div class="row">
    <label>Enable <select id="cfg_on" onchange="push()"><option value="1">ON</option><option value="0">OFF</option></select></label>
    <label>Target Grade <select id="cfg_grade" onchange="push()"><option>N</option><option>G</option><option>DG</option></select></label>
    <label>Mode <select id="cfg_mode" onchange="push()"><option value="any">Any rule matches</option><option value="all">All rules match</option><option value="per_attr">Each attr matches >=1 rule</option></select></label>
  </div>
  <div class="row" style="margin-top:8px">
    <label>Require grade <select id="cfg_req" onchange="push()"><option>DG</option><option>G</option><option>N</option><option value="false">No requirement</option></select></label>
  </div>
  <div id="rules" style="margin-top:10px"></div>
  <div class="row" style="margin-top:8px">
    <button class="btn btn-go" onclick="addRule()">+ Add Rule</button>
    <span class="note">max 3 rules (one per attr slot)</span>
  </div>

  <h3 style="margin-top:16px">Superior Rules (override — stop immediately if matched)</h3>
  <div id="override_rules" style="margin-top:6px"></div>
  <div class="row" style="margin-top:8px">
    <button class="btn btn-go" onclick="addOverride()">+ Add Override</button>
    <span class="note">too good to miss — bypasses main filter</span>
  </div>
</div>

<script>
let _rules=[], _ovr=[], _filter={}, names=[];

async function loadNames(){
  try{names=await(await fetch('/api/attr_names?grade=DG')).json()}catch(e){}
  if(!names.length) names=['攻擊力'];
  renderRules();
}

async function refresh(){
  let s={}; try{s=await(await fetch('/api/state')).json()}catch(e){}
  document.getElementById('stat_status').textContent=s.running?'RUNNING':'IDLE';
  document.getElementById('stat_status').className='stat'+(s.running?' live':'');
  document.getElementById('stat_grade').textContent=s.grade||'?';
  document.getElementById('stat_attempt').textContent=s.attempt||0;
  document.getElementById('stat_remaining').textContent=s.remaining||'?';
  let fs=s.filter_status||'--';
  document.getElementById('stat_filter').textContent=fs;
  document.getElementById('stat_filter').className='val'+(fs.indexOf('no match')>=0?' bad':fs.indexOf('MATCH')>=0?' good':'');
  let a=s.attrs||[];
  for(let i=0;i<3;i++){
    let m=a[i]||{};
    document.getElementById('a'+(i+1)+'_name').textContent=m.name||'--';
    document.getElementById('a'+(i+1)+'_val').textContent=m.value!=null?m.value:'';
  }
  // Load config on first run
  if(!names.length){
    try{names=await(await fetch('/api/attr_names')).json()}catch(e){}
    let cfg={}; try{cfg=await(await fetch('/api/config')).json()}catch(e){}
    _filter=cfg.filter||{};
    document.getElementById('cfg_on').value=_filter.enabled?'1':'0';
    document.getElementById('cfg_grade').value=cfg.target_grade||'DG';
    document.getElementById('cfg_mode').value=_filter.match_mode||'any';
    document.getElementById('cfg_req').value=_filter.require_grade||'DG';
    _rules=(_filter.rules||[]).slice();
    _ovr=(_filter.override_rules||[]).slice();
    renderRules();renderOverride();
  }
}

function renderRules(){
  let d=document.getElementById('rules');
  d.innerHTML=_rules.map((r,i)=>'<div class="rule-line">'+
    '<select id="rn_'+i+'" onchange="push()">'+names.map(n=>'<option '+(r.name==n?'selected':'')+'>'+n+'</option>').join('')+'</select>'+
    ' Count <input type=number id="rc_'+i+'" value="'+(r.count||1)+'" min=1 max=3 onchange="push()" style="width:40px">'+
    ' Min <input type=number id="rmin_'+i+'" value="'+(r.min||'')+'" onchange="push()">'+
    ' Max <input type=number id="rmax_'+i+'" value="'+(r.max||'')+'" onchange="push()">'+
    ' <button class="btn btn-stop" onclick="del('+i+')" style="padding:4px 8px;font-size:11px">X</button>'+
    '</div>').join('');
}

function addRule(){if(_rules.length<3){_rules.push({name:names[0]});renderRules();push()}}
function del(i){_rules.splice(i,1);renderRules();push()}

function addOverride(){if(_ovr.length<3){_ovr.push({name:names[0],count:1});renderOverride();push()}}
function delOvr(i){_ovr.splice(i,1);renderOverride();push()}

function renderOverride(){
  let d=document.getElementById('override_rules');
  d.innerHTML=_ovr.map((r,i)=>'<div class="rule-line">'+
    '<select id="ovn_'+i+'" onchange="push()">'+names.map(n=>'<option '+(r.name==n?'selected':'')+'>'+n+'</option>').join('')+'</select>'+
    ' Count <input type=number id="ovc_'+i+'" value="'+(r.count||1)+'" min=1 max=3 onchange="push()" style="width:40px">'+
    ' Min <input type=number id="ovmin_'+i+'" value="'+(r.min||'')+'" onchange="push()">'+
    ' Max <input type=number id="ovmax_'+i+'" value="'+(r.max||'')+'" onchange="push()">'+
    ' <button class="btn btn-stop" onclick="delOvr('+i+')" style="padding:4px 8px;font-size:11px">X</button>'+
    '</div>').join('');
}

function push(){
  let total=0;
  for(let i=0;i<_rules.length;i++){
    let s=document.getElementById('rn_'+i);if(s)_rules[i].name=s.value;
    let c=document.getElementById('rc_'+i);let ct=c?parseInt(c.value)||1:1;
    if(ct<1)ct=1;if(ct>3)ct=3;_rules[i].count=ct;total+=ct;
    let mi=document.getElementById('rmin_'+i);let mx=document.getElementById('rmax_'+i);
    if(mi&&mi.value) _rules[i].min=parseInt(mi.value); else delete _rules[i].min;
    if(mx&&mx.value) _rules[i].max=parseInt(mx.value); else delete _rules[i].max;
  }
  // Validate: sum of counts <= 3
  if(total>3){
    document.getElementById('stat_filter').textContent='Sum of counts > 3!';
    document.getElementById('stat_filter').className='val bad';return;
  }
  // Read override rules
  for(let i=0;i<_ovr.length;i++){
    let s=document.getElementById('ovn_'+i);if(s)_ovr[i].name=s.value;
    let c=document.getElementById('ovc_'+i);_ovr[i].count=c?parseInt(c.value)||1:1;
    let mi=document.getElementById('ovmin_'+i);let mx=document.getElementById('ovmax_'+i);
    if(mi&&mi.value) _ovr[i].min=parseInt(mi.value); else delete _ovr[i].min;
    if(mx&&mx.value) _ovr[i].max=parseInt(mx.value); else delete _ovr[i].max;
  }
  let cfg={
    target_grade: document.getElementById('cfg_grade').value,
    filter:{
      enabled: document.getElementById('cfg_on').value=='1',
      match_mode: document.getElementById('cfg_mode').value,
      require_grade: document.getElementById('cfg_req').value,
      rules: _rules.slice(),
      override_rules: _ovr.slice()
    }
  };
  fetch('/api/config',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(cfg)});
}

function ctrl(a){fetch('/api/control',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:a})})}

setInterval(refresh,800);refresh();
</script>
</body></html>"""

@app.route("/")
def index(): return render_template_string(HTML)

@app.route("/api/state")
def api_state():
    if STATE_PATH.exists():
        with open(STATE_PATH,"r",encoding="utf-8") as f: return jsonify(json.load(f))
    return jsonify({"running":False,"grade":None,"attempt":0,"remaining":None,"attrs":[],"filter_status":"--"})

@app.route("/api/config",methods=["GET","POST"])
def api_config():
    if request.method=="POST":
        data=request.get_json()
        cfg={}
        if CONFIG_PATH.exists():
            with open(CONFIG_PATH,"r",encoding="utf-8") as f: cfg=yaml.safe_load(f.read()) or {}
        if "filter" in data: cfg["filter"]=data["filter"]
        if "target_grade" in data: cfg["target_grade"]=data["target_grade"]
        with open(CONFIG_PATH,"w",encoding="utf-8") as f:
            yaml.dump(cfg,f,allow_unicode=True,default_flow_style=False,sort_keys=False)
        return jsonify({"status":"ok"})
    if CONFIG_PATH.exists():
        with open(CONFIG_PATH,"r",encoding="utf-8") as f: return jsonify(yaml.safe_load(f.read()) or {})
    return jsonify({})

@app.route("/api/control",methods=["POST"])
def api_control():
    data=request.get_json()
    with open(CONTROL_PATH,"w") as f: f.write(data.get("action","")+"\n")
    return jsonify({"status":"ok"})

@app.route("/api/attr_names")
def api_attr_names():
    from tuner.attr_matcher import ATTRIBUTES
    grade = request.args.get("grade", "DG")

    # Attributes available per grade
    GRADE_ATTRS = {
        "N": {"攻擊力","魔法力","防禦力","攻擊速度","必殺技","命中率","迴避率",
              "移動速度","HP(值)","AP(值)","減少道具等級限制"},
        "G": {"每級+1力量","每級+1敏捷","每級+1智力","每級+1幸運","每級+1體力",
              "每級+1精神","HP(%)","AP(%)","經驗值獲得量增加","副本傷害增加"},
        "DG": {"增加傷害","減少傷害"},
        "XG": {"每級+2力量","每級+2敏捷","每級+2智力","每級+2幸運"},
        "SG": set(),
    }

    # Build cumulative set
    order = ["N","G","DG","XG","SG"]
    allowed = set()
    for g in order:
        allowed |= GRADE_ATTRS.get(g, set())
        if g == grade:
            break

    return jsonify([n for n,_,_ in ATTRIBUTES if n in allowed])

def start_panel(port=5000):
    import threading
    t=threading.Thread(target=lambda:app.run(host="127.0.0.1",port=port,debug=False,use_reloader=False),daemon=True)
    t.start()
    print(f"[Panel] http://127.0.0.1:{port}")
    return t
