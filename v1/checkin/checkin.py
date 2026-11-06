"""
Seal Online 每日簽到 Automation
================================
Login (manual reCAPTCHA) + daily 簽到 check-in for multiple accounts.

2026/11/04 「火雞的考驗」event:
- 首次進入活動頁需先選伺服器 (select.php)，選擇後無法更改
  （此期 select 的 value 同舊期相反：2 = 拉麥爾/卡蕾兒/阿勒斯, 1 = 哈比兔/勇者咕咕）
- 每天可領一天；get.php 以 {index: N} 領取（必須依序，見 RetVal -11）
- 已領的日子不再有 get(N) button（換成 collar_1 標記），剩下嘅 link 就係未領
- 活動有補簽 (get.php is_makeup=1)：今日領取後會用盡剩餘次數補漏簽（-20/-21/-22）

- Each account's session saved separately in sessions/<username>.json
- Session reused until expired; re-login (manual captcha) only when needed
- Check-in runs headless when session is valid (no browser window)
"""
import sys
import ast
import json
import time
from pathlib import Path
from urllib.parse import urlsplit
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
    """Event directory, used to build get.php / select.php URLs.

    checkin_url may be the directory or its index.php (the browser shows both),
    so drop a trailing page name before joining the endpoints.
    """
    u = urlsplit(checkin_url)
    path = u.path if u.path.endswith("/") else u.path.rsplit("/", 1)[0] + "/"
    return f"{u.scheme}://{u.netloc}{path}"


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


def ensure_server_selected(context, page, event_url, server_value):
    """First visit to a new event forces a one-time, irreversible server pick.

    Each event has its own select.php AND its own value numbering: for 20261104
    2 = 拉麥爾/卡蕾兒/阿勒斯 and 1 = 哈比兔/勇者咕咕, the reverse of 20260930,
    so `server` in the config has to change together with the event.

    Returns (ok, message_or_None).
    """
    page.goto(event_url, wait_until="domcontentloaded")
    page.wait_for_timeout(2000)

    if "select.php" not in page.url:
        return True, None

    if server_value in (None, "", "no"):
        return False, "需要選伺服器，但 config 未設定 server（2=拉麥爾/卡蕾兒/阿勒斯, 1=哈比兔/勇者咕咕）"

    print(f"  → 首次進入，選擇伺服器 {server_value} ...")
    context.request.post(base_url(event_url) + "select.php",
                         form={"server": str(server_value)}, timeout=30000)

    page.goto(event_url, wait_until="domcontentloaded")
    page.wait_for_timeout(2000)
    if "select.php" in page.url:
        return False, "選擇伺服器失敗"
    return True, "已選擇伺服器"


def get_claimable_days(page):
    """Day numbers still claimable, from the javascript:get(N) day buttons.

    A claimed day stops rendering its button entirely — the server puts a
    `collar_1` 已領取 marker in its place — so every get(N) link left on the page
    is a day that can still be claimed.
    """
    return page.evaluate(
        """() => [...document.querySelectorAll('a[href^="javascript:get("]')]
             .map(a => {
               const m = (a.getAttribute('href') || '').match(/(\\d+)/);
               return m ? parseInt(m[1], 10) : 0;
             })
             .filter(day => day > 0)"""
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


# get.php is_makeup=1 RetVal meanings (see gachapon.js)
MAKEUP_MSG = {
    "-20": "目前沒有補簽次數",
    "-21": "沒有可補的漏簽天數",
    "-22": "請先領取今天的獎勵再補簽",
}


def do_checkin(context, page, checkin_url):
    """Claim the next unclaimed day; the server allows one per calendar day."""
    page.goto(checkin_url, wait_until="domcontentloaded")
    page.wait_for_timeout(2500)

    days = get_claimable_days(page)
    if not days:
        return True, "沒有可領取的日期（已領完或活動未開始）"

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
            if ret == "-10":
                # One day per calendar day. A second run (or the scheduled task
                # catching up after a missed 00:05) must not be a failure.
                if claimed:
                    print("  ✓ 今天已領過，收工")
                    return True, "；".join(claimed) + "；今天已領過"
                print("  ✓ 今天已領過，不用再領")
                return True, RET_MSG["-10"]

            print(f"  ⛔ 第 {day} 天：{RET_MSG.get(ret, f'未知回應 {data}')}")
            return (bool(claimed), "；".join(claimed) or RET_MSG.get(ret, ret))
        else:
            print(f"  ⛔ 第 {day} 天：排隊重試多次仍失敗")
            return (bool(claimed), "；".join(claimed) + "；排隊逾時")

    return True, "；".join(claimed)


def do_makeup(context, checkin_url):
    """Spend every 補簽 (makeup) chance the event grants, after today's claim.

    Each call fills one missed day and answers with the remaining count. The
    server refuses with -20 (no chances), -21 (nothing to make up) or -22 (today
    must be claimed first). Capped at 10 calls so a bad response cannot spin.
    """
    base = base_url(checkin_url)
    filled = []
    for _ in range(10):
        data = post_json(context, base + "get.php", {"is_makeup": 1})
        ret = str(data.get("RetVal", ""))
        item = str(data.get("item_name", "") or "")
        if ret == "Y":
            filled.append(item or "補簽成功")
            print(f"  ✓ 補簽：{item}")
            if str(data.get("makeup_chances", "0")).strip() in ("", "0"):
                break
            time.sleep(0.6)
            continue
        if ret == "-22":
            print(f"  ⚠ 補簽：{MAKEUP_MSG[ret]}")
        elif ret in ("-20", "-21"):
            print(f"  … 補簽：{MAKEUP_MSG[ret]}")
        else:
            print(f"  ⛔ 補簽：未知回應 {data}")
        break
    return filled


def process_account(p, acc, login_url, checkin_url, server_value):
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
    ok, sel_msg = ensure_server_selected(context, page, checkin_url, server_value)
    if sel_msg:
        print(f"[{username}] {sel_msg}")
    if not ok:
        context.close()
        return False, sel_msg

    # ── Daily check-in ──
    ok, msg = do_checkin(context, page, checkin_url)

    # ── 補簽 (makeup) for missed days, only once today's claim is in ──
    if ok:
        filled = do_makeup(context, checkin_url)
        if filled:
            msg = f"{msg}；補簽 {'、'.join(filled)}"

    context.close()
    return ok, msg


def main():
    cfg = load_config()
    accounts = cfg.get("accounts", [])
    if not accounts:
        print("[!] 無帳號設定（請編輯 checkin_config.yaml）")
        return

    login_url = cfg.get("login_url", "https://security.sponline.com.tw/login/login.php")
    checkin_url = cfg.get("checkin_url", "https://security.sponline.com.tw/event/20261104_1/index.php")
    server_value = cfg.get("server")

    print(f"共 {len(accounts)} 個帳號\n")

    results = []
    with sync_playwright() as p:
        for acc in accounts:
            username = acc.get("username", "?")
            print(f"\n=== {username} ===")
            try:
                ok, msg = process_account(p, acc, login_url, checkin_url, server_value)
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
