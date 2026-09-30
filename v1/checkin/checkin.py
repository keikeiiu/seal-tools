"""
Seal Online 每日簽到 Automation
================================
Login (manual reCAPTCHA) + daily 簽到 check-in for multiple accounts.

2026/09/30 「初戀投心」event:
- 首次進入活動頁需先選伺服器 (select.php)，選擇後無法更改
- 每天可領一天；get.php 以 {index: N} 領取（必須依序，見 RetVal -11）
- 此活動沒有補簽機制（舊活動的 makeup() 已不存在）

- Each account's session saved separately in sessions/<username>.json
- Session reused until expired; re-login (manual captcha) only when needed
- Check-in runs headless when session is valid (no browser window)
"""
import sys
import ast
import json
import time
from pathlib import Path
import yaml
from playwright.sync_api import sync_playwright

sys.stdout.reconfigure(encoding="utf-8")

SCRIPT_DIR = Path(__file__).parent
CONFIG_PATH = SCRIPT_DIR / "checkin_config.yaml"
SESSION_DIR = SCRIPT_DIR / "sessions"
SESSION_DIR.mkdir(exist_ok=True)


def load_config():
    with open(CONFIG_PATH, "r", encoding="utf-8") as f:
        return yaml.safe_load(f.read()) or {}


def session_path(username):
    return SESSION_DIR / f"{username}.json"


def base_url(checkin_url):
    """Event directory, used to build get.php / select.php URLs."""
    return checkin_url if checkin_url.endswith("/") else checkin_url + "/"


def post_json(context, url, form):
    """POST a form and parse the JSON the PHP responds with.

    The event pages are UTF-8 but some helper scripts are still Big5, so the
    response encoding is sniffed rather than assumed.
    """
    r = context.request.post(url, form=form, timeout=30000)
    raw = r.body()
    try:
        text = raw.decode("utf-8")
    except UnicodeDecodeError:
        text = raw.decode("big5", errors="replace")
    try:
        return json.loads(text)
    except Exception:
        pass
    # The PHP hands back a JS array literal, e.g. {'RetVal':'Y','item_name':'...'}
    # — the page itself parses it with eval(). Python's literal_eval takes it as-is.
    try:
        parsed = ast.literal_eval(text.strip())
        if isinstance(parsed, dict):
            return parsed
    except Exception:
        pass
    return {"_raw": text}


def is_logged_in(page, checkin_url):
    """Navigate to the event page; True if still logged in (not sent to login)."""
    page.goto(checkin_url, wait_until="domcontentloaded")
    page.wait_for_timeout(2000)
    return "login" not in page.url


def do_login(page, username, password, login_url):
    """Fill credentials, wait for manual captcha, auto-submit when solved."""
    print(f"  → 登入 {username} ...")
    page.goto(login_url, wait_until="domcontentloaded")
    page.wait_for_timeout(1500)

    page.fill('#userID', username)
    page.fill('#userPW', password)

    print("  ⚠ 請在瀏覽器視窗中完成 reCAPTCHA 驗證...")

    solved = False
    for _ in range(120):  # up to 2 min
        val = page.evaluate(
            "document.getElementById('login_recaptcha')?.value || "
            "document.querySelector('.g-recaptcha-response')?.value || ''"
        )
        if val:
            solved = True
            break
        time.sleep(1)

    if not solved:
        print("  ⚠ 未偵測到 reCAPTCHA，仍嘗試提交...")

    try:
        page.evaluate("$('form').submit()")
    except Exception:
        page.click("a[href=\"javascript:$('form').submit();\"]")

    page.wait_for_timeout(3000)


def ensure_server_selected(context, page, event_url, field, server_value):
    """First visit to a new event forces a one-time, irreversible server pick.

    Each event has its own select.php. Note the receive page's form names the
    field `sever` (sic) while the check-in page's is `server` — hence `field`.

    Returns (ok, message_or_None).
    """
    page.goto(event_url, wait_until="domcontentloaded")
    page.wait_for_timeout(2000)

    if "select.php" not in page.url:
        return True, None

    if server_value in (None, "", "no"):
        return False, "需要選伺服器，但 config 未設定 server（1=拉麥爾, 2=哈比兔）"

    print(f"  → 首次進入，選擇伺服器 {server_value} ...")
    context.request.post(base_url(event_url) + "select.php",
                         form={field: str(server_value)}, timeout=30000)

    page.goto(event_url, wait_until="domcontentloaded")
    page.wait_for_timeout(2000)
    if "select.php" in page.url:
        return False, "選擇伺服器失敗"
    return True, "已選擇伺服器"


def get_claimable_days(page):
    """Day numbers of 第N天 buttons that are not already used/disabled."""
    return page.evaluate(
        """() => [...document.querySelectorAll('.heart-btn')]
             .map(b => {
               const m = (b.innerText || '').match(/(\\d+)/);
               return {
                 day: m ? parseInt(m[1], 10) : 0,
                 used: b.classList.contains('used') || b.disabled,
               };
             })
             .filter(x => x.day && !x.used)
             .map(x => x.day)"""
    )


# get.php RetVal meanings (see gachapon.js)
RET_MSG = {
    "-1": "請先登入（session 過期）",
    "-2": "活動期間外",
    "-3": "天數錯誤",
    "-10": "今天已經領過囉",
    "-11": "請依序領取獎勵",
    "-12": "人數過多，請稍後再領取",
}


def do_checkin(context, page, checkin_url):
    """Claim every not-yet-used day, in order, until the server refuses."""
    page.goto(checkin_url, wait_until="domcontentloaded")
    page.wait_for_timeout(2500)

    days = get_claimable_days(page)
    if not days:
        return False, "沒有可領取的日期（今天已領完或活動未開始）"

    base = base_url(checkin_url)
    claimed = []
    for day in sorted(days):
        # -12 means a queue formed; retry a few times before giving up
        for attempt in range(4):
            data = post_json(context, base + "get.php", {"index": day})
            ret = str(data.get("RetVal", ""))
            item = str(data.get("item_name", "") or "")

            if ret == "Y":
                claimed.append(f"第 {day} 天 → {item}")
                print(f"  ✓ 領取第 {day} 天：{item}")
                time.sleep(0.6)
                break
            if ret == "-12":
                wait = 3 * (attempt + 1)
                print(f"  … 第 {day} 天排隊中（{item}），{wait}s 後重試")
                time.sleep(wait)
                continue

            print(f"  ⛔ 第 {day} 天：{RET_MSG.get(ret, f'未知回應 {data}')}")
            return (bool(claimed), "；".join(claimed) or RET_MSG.get(ret, ret))
        else:
            print(f"  ⛔ 第 {day} 天：排隊重試多次仍失敗")
            return (bool(claimed), "；".join(claimed) + "；排隊逾時")

    return True, "；".join(claimed)


# get.php (20260930_receive) RetVal meanings (see that page's gachapon.js)
RECV_TIERS = [("1", "集會包"), ("2", "義氣包"), ("3", "大哥包")]
RECV_MSG = {
    "-1": "請先登入（session 過期）",
    "-2": "請選擇領取項目",
    "-4": "活動期間才可領取",
    "-10": "資格不符",
    "-11": "已領取過了",
    "-13": "請先選擇伺服器",
    "-14": "請稍後再領取",
}


def do_receive(context, page, receive_url):
    """Claim the 【集會包】/【義氣包】/【大哥包】 tier this account qualifies for.

    All three tiers share one endpoint; the server answers 資格不符 (-10) for the
    tiers the account isn't eligible for, so trying all three is safe.
    """
    page.goto(receive_url, wait_until="domcontentloaded")
    page.wait_for_timeout(2500)

    base = base_url(receive_url)
    results = []
    got_any = False
    for tier, name in RECV_TIERS:
        for attempt in range(4):
            data = post_json(context, base + "get.php", {"sel_button": tier})
            ret = str(data.get("RetVal", ""))
            item = str(data.get("item_name", "") or "")

            if ret == "Y":
                print(f"  ✓ {name}：領取成功 {item}")
                results.append(f"{name} {item}")
                got_any = True
                break
            if ret == "-12":
                wait = 3 * (attempt + 1)
                print(f"  … {name} 排隊中（{item}），{wait}s 後重試")
                time.sleep(wait)
                continue

            note = RECV_MSG.get(ret, f"未知回應 {data}")
            if ret == "-11":  # 已領取過 — a previous run already got it
                print(f"  ✓ {name}：{note}")
                results.append(f"{name} {note}")
                got_any = True
            elif ret not in ("-10",):  # 資格不符 is expected for the other two tiers
                print(f"  ⛔ {name}：{note}")
                results.append(f"{name} {note}")
            break

    return got_any, results


def process_account(p, acc, login_url, checkin_url, server_value, receive_url=None):
    """Process one account: server select (first time) + daily check-in."""
    username = acc["username"]
    password = acc["password"]
    sp = session_path(username)

    page = None
    context = None

    # ── Try headless with saved session first ──
    if sp.exists():
        context = p.chromium.launch(headless=True).new_context(
            storage_state=str(sp), ignore_https_errors=True
        )
        page = context.new_page()
        page.on("dialog", lambda d: d.accept())
        if is_logged_in(page, checkin_url):
            print(f"[{username}] 使用已存 session")
        else:
            context.close()
            page = None
            print(f"[{username}] session 過期，需重新登入")

    # ── Visible browser for login (manual captcha) if still needed ──
    if page is None:
        context = p.chromium.launch(headless=False).new_context(ignore_https_errors=True)
        page = context.new_page()
        page.on("dialog", lambda d: d.accept())

        do_login(page, username, password, login_url)

        if not is_logged_in(page, checkin_url):
            context.close()
            return False, "登入失敗（檢查帳號密碼或 captcha）"

        context.storage_state(path=str(sp))
        print(f"[{username}] session 已儲存")

    # ── Server selection (one-time, irreversible) ──
    ok, sel_msg = ensure_server_selected(context, page, checkin_url, "server", server_value)
    if sel_msg:
        print(f"[{username}] {sel_msg}")
    if not ok:
        context.close()
        return False, sel_msg

    # ── Daily check-in ──
    ok, msg = do_checkin(context, page, checkin_url)

    # ── One-time 【集會包】/【義氣包】/【大哥包】 claim ──
    if receive_url:
        rok, recv_msg = ensure_server_selected(context, page, receive_url, "sever", server_value)
        if recv_msg:
            print(f"[{username}] 領取頁 {recv_msg}")
        if rok:
            got, parts = do_receive(context, page, receive_url)
            if parts:
                msg = f"{msg}；{'；'.join(parts)}"
            ok = ok or got

    context.close()
    return ok, msg


def main():
    cfg = load_config()
    accounts = cfg.get("accounts", [])
    if not accounts:
        print("[!] 無帳號設定（請編輯 checkin_config.yaml）")
        return

    login_url = cfg.get("login_url", "https://security.sponline.com.tw/login/login.php")
    checkin_url = cfg.get("checkin_url", "https://security.sponline.com.tw/event/20260930/")
    receive_url = cfg.get("receive_url")
    server_value = cfg.get("server")

    print(f"共 {len(accounts)} 個帳號\n")

    results = []
    with sync_playwright() as p:
        for acc in accounts:
            username = acc.get("username", "?")
            print(f"\n=== {username} ===")
            try:
                ok, msg = process_account(p, acc, login_url, checkin_url, server_value, receive_url)
            except Exception as e:
                ok, msg = False, f"錯誤: {e}"
            results.append((username, ok, msg))

    print("\n=== 結果 ===")
    success = 0
    for username, ok, msg in results:
        status = "✓" if ok else "✗"
        print(f"  {status} {username}: {msg}")
        if ok:
            success += 1

    print(f"\n成功 {success}/{len(results)}。session 已存，下次免重新登入（直到過期）。")


if __name__ == "__main__":
    main()
