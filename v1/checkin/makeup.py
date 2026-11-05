"""
Seal Online 補簽 (makeup check-in) one-off script
=================================================
Clicks the 補簽 (makeup) button repeatedly until it can no longer be
used (no chances left, no missed days, or today's reward not yet claimed).

Reuses the saved session + config from checkin.py. Headless when the
session is valid; does NOT re-login (run checkin.py first if a session
expired).
"""
import sys
import time
from playwright.sync_api import sync_playwright

sys.stdout.reconfigure(encoding="utf-8")

from checkin import load_config, session_path, is_logged_in

MAX_MAKEUPS = 30  # safety cap (event is 28 days max)


def do_makeup_once(page):
    """Fire one makeup request (mirrors the page's makeup() -> POST get.php).
    Returns the parsed response object (RetVal / item_name / makeup_chances)."""
    return page.evaluate(
        """
        async () => {
            const r = await fetch('get.php', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/x-www-form-urlencoded',
                    'X-Requested-With': 'XMLHttpRequest',
                },
                body: 'is_makeup=1'
            });
            const buf = await r.arrayBuffer();
            let text;
            try { text = new TextDecoder('big5').decode(buf); }
            catch (e) { text = new TextDecoder().decode(buf); }
            try { return JSON.parse(text); }
            catch (e) { return eval('(' + text + ')'); }
        }
        """
    )


def process_account(p, acc, checkin_url):
    username = acc["username"]
    sp = session_path(username)
    if not sp.exists():
        print(f"  [x] 無 session，跳過（請先跑 checkin.py 登入）")
        return

    context = p.chromium.launch(headless=True).new_context(storage_state=str(sp))
    page = context.new_page()
    page.on("dialog", lambda d: d.accept())

    if not is_logged_in(page, checkin_url):
        print(f"  [x] session 過期，需重新登入（請先跑 checkin.py）")
        context.close()
        return

    print(f"  [o] 使用已存 session，開始補簽...")
    total = 0
    for _ in range(MAX_MAKEUPS):
        data = do_makeup_once(page)
        if not isinstance(data, dict):
            print(f"  [x] 回應解析失敗: {data}")
            break

        ret = str(data.get("RetVal", ""))
        if ret == "Y":
            total += 1
            item = data.get("item_name", "")
            left = data.get("makeup_chances", "?")
            print(f"  ✓ 補簽成功 #{total}: {item}（剩餘 {left} 次）")
            time.sleep(0.5)
        elif ret == "-20":
            print("  ⛔ 目前沒有補簽次數了")
            break
        elif ret == "-21":
            print("  ⛔ 沒有可補的漏簽天數")
            break
        elif ret == "-22":
            print("  ⛔ 請先領取今天的獎勵再補簽")
            break
        else:
            print(f"  ⛔ 未知回應: {data}")
            break

    print(f"  → {username} 完成，共補簽 {total} 次")
    context.close()


def main():
    cfg = load_config()
    accounts = cfg.get("accounts", [])
    if not accounts:
        print("[!] 無帳號設定（請編輯 checkin_config.yaml）")
        return

    checkin_url = cfg.get(
        "checkin_url", "https://ntest.sponline.com.tw/intro/20260930/#event"
    )

    print(f"共 {len(accounts)} 個帳號\n")

    with sync_playwright() as p:
        for acc in accounts:
            print(f"=== {acc.get('username', '?')} ===")
            try:
                process_account(p, acc, checkin_url)
            except Exception as e:
                print(f"  [x] 錯誤: {e}")


if __name__ == "__main__":
    main()
