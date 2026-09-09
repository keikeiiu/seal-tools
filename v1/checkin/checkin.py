"""
Seal Online 每日簽到 Automation
================================
Login (manual reCAPTCHA) + daily 簽到 check-in for multiple accounts.

- Each account's session saved separately in sessions/<username>.json
- Session reused until expired; re-login (manual captcha) only when needed
- Check-in runs headless when session is valid (no browser window)
"""
import sys
import re
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


def get_day_numbers(page):
    """Return sorted list of unclaimed day numbers from get(N) links."""
    days = []
    for i in range(page.locator('a[href*="javascript:get("]').count()):
        href = page.locator('a[href*="javascript:get("]').nth(i).get_attribute("href") or ""
        m = re.search(r"get\((\d+)\)", href)
        if m:
            days.append(int(m.group(1)))
    return sorted(days)


def is_logged_in(page, checkin_url):
    """Navigate to check-in page; True if still logged in (not redirected to login).

    Only the login redirect is checked. A valid session can still show no claimable
    days (event ended / already claimed today), which is handled separately in
    do_checkin() — treating that as "session expired" here was a false alarm.
    """
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


def do_checkin(page, checkin_url):
    """Click earliest unclaimed day, read jQuery dialog result."""
    page.goto(checkin_url, wait_until="domcontentloaded")
    page.wait_for_timeout(2500)

    days = get_day_numbers(page)
    if not days:
        return False, "找不到可點擊日期（活動可能已結束）"

    day_n = days[0]
    print(f"  → 簽到第 {day_n} 天...")

    page.evaluate(f"get({day_n})")

    # Poll jQuery dialog (auto-closes after a few seconds)
    dialog_text = ""
    for _ in range(20):
        time.sleep(0.5)
        dialog_text = page.evaluate(
            "document.getElementById('dialog-message')?.innerText || ''"
        ).strip()
        if dialog_text:
            break

    if dialog_text:
        print(f"  [結果] {dialog_text}")

    if "恭喜獲得" in dialog_text:
        return True, f"第 {day_n} 天簽到完成"
    elif "已經領過" in dialog_text:
        return True, "今天已領過"
    elif dialog_text:
        return False, dialog_text
    else:
        return False, "無回應"


def do_lottery_join(page, lottery_url):
    """Navigate to the lottery page and submit the daily join (POST lottery_entry.php).

    Uses the same POST the 會員登入 button fires, so it works regardless of the
    button's visibility state (e.g. already-joined accounts).
    """
    page.goto(lottery_url, wait_until="domcontentloaded")
    page.wait_for_timeout(2000)

    try:
        data = page.evaluate(
            """
            async () => {
                const r = await fetch('lottery_entry.php', {
                    method: 'POST',
                    headers: {
                        'Content-Type': 'application/x-www-form-urlencoded',
                        'X-Requested-With': 'XMLHttpRequest',
                    },
                    body: 'ref='
                });
                return await r.json();
            }
            """
        )
    except Exception as e:
        return False, f"請求失敗: {e}"

    ret = str(data.get("RetVal", "")) if isinstance(data, dict) else ""

    if ret == "Y":
        return True, "登錄成功"
    elif ret == "-3":
        return True, "您已經登錄過了"
    elif ret == "-1":
        return False, "需要登入（session 過期）"
    elif ret == "-5":
        return False, "活動期間外"
    elif ret == "-14":
        return False, "請稍後再領取"
    else:
        return False, f"未知回應: {data}"


def process_account(p, acc, login_url, checkin_url, lottery_url=None):
    """Process one account: daily check-in + optional lottery join. Returns (ok, message)."""
    username = acc["username"]
    password = acc["password"]
    sp = session_path(username)

    page = None
    context = None

    # ── Try headless with saved session first ──
    if sp.exists():
        context = p.chromium.launch(headless=True).new_context(storage_state=str(sp))
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
        context = p.chromium.launch(headless=False).new_context()
        page = context.new_page()
        page.on("dialog", lambda d: d.accept())

        do_login(page, username, password, login_url)

        if not is_logged_in(page, checkin_url):
            context.close()
            return False, "登入失敗（檢查帳號密碼或 captcha）"

        context.storage_state(path=str(sp))
        print(f"[{username}] session 已儲存")

    # ── Daily actions ──
    ok, msg = do_checkin(page, checkin_url)
    if lottery_url:
        lok, lmsg = do_lottery_join(page, lottery_url)
        msg = f"{msg}；抽獎：{lmsg}"
        if not lok:
            ok = False

    context.close()
    return ok, msg


def main():
    cfg = load_config()
    accounts = cfg.get("accounts", [])
    if not accounts:
        print("[!] 無帳號設定（請編輯 checkin_config.yaml）")
        return

    login_url = cfg.get("login_url", "https://security.sponline.com.tw/login/login.php")
    checkin_url = cfg.get("checkin_url", "https://security.sponline.com.tw/event/20260806/")
    lottery_url = cfg.get("lottery_url")  # optional — daily lottery join

    print(f"共 {len(accounts)} 個帳號\n")

    results = []
    with sync_playwright() as p:
        for acc in accounts:
            username = acc.get("username", "?")
            print(f"\n=== {username} ===")
            try:
                ok, msg = process_account(p, acc, login_url, checkin_url, lottery_url)
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
