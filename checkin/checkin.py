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
    """Navigate to check-in page; True if session valid (get(N) links present)."""
    page.goto(checkin_url, wait_until="domcontentloaded")
    page.wait_for_timeout(2000)
    if "login" in page.url:
        return False
    return len(get_day_numbers(page)) > 0


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
        return False, "找不到可點擊日期"

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


def process_account(p, acc, login_url, checkin_url):
    """Process one account. Returns (ok, message)."""
    username = acc["username"]
    password = acc["password"]
    sp = session_path(username)

    # ── Try headless with saved session first ──
    if sp.exists():
        context = p.chromium.launch(headless=True).new_context(storage_state=str(sp))
        page = context.new_page()
        page.on("dialog", lambda d: d.accept())
        if is_logged_in(page, checkin_url):
            print(f"[{username}] 使用已存 session")
            ok, msg = do_checkin(page, checkin_url)
            context.close()
            return ok, msg
        context.close()
        print(f"[{username}] session 過期，需重新登入")

    # ── Visible browser for login (manual captcha) ──
    context = p.chromium.launch(headless=False).new_context()
    page = context.new_page()
    page.on("dialog", lambda d: d.accept())

    do_login(page, username, password, login_url)

    # Verify login succeeded
    if not is_logged_in(page, checkin_url):
        context.close()
        return False, "登入失敗（檢查帳號密碼或 captcha）"

    # Save session for next time
    context.storage_state(path=str(sp))
    print(f"[{username}] session 已儲存")

    ok, msg = do_checkin(page, checkin_url)
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

    print(f"共 {len(accounts)} 個帳號\n")

    results = []
    with sync_playwright() as p:
        for acc in accounts:
            username = acc.get("username", "?")
            print(f"\n=== {username} ===")
            try:
                ok, msg = process_account(p, acc, login_url, checkin_url)
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
