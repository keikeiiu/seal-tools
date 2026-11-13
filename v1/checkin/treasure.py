"""
Seal Online 總長寶庫 (treasury) one-off draw
============================================
The 20261111 event page is the 極限突破愛羅武勇 treasury: an account that
completed 4 覺醒試煉 during the 三轉 event gets one draw per day (8 trials -> 2),
and 2026/11/13~11/15 is the make-up window for accounts whose eligibility was
misjudged. An account without the qualification gets nowhere at all — the page
ships

    alert('您尚未達成活動參加條件'); top.location.href = '/';

so the eligibility check reads the raw HTML through the request context; a real
browser has already bounced to the site root before anything can be scraped.

The event numbers its servers differently from the check-in event (20261111 has
1 = 拉麥爾/卡蕾兒/阿勒斯, while 20261104_1 had 2 for that same group), so the
領獎伺服器 is picked by the option *label*, never by its value.

Reuses checkin_config.yaml + sessions/. Headless; does NOT re-login (run
checkin.py first if a session expired). Usage: python treasure.py
"""
import sys
import re
import time
from playwright.sync_api import sync_playwright

from checkin import load_config, post_json, session_path

sys.stdout.reconfigure(encoding="utf-8")

EVENT_BASE = "https://security.sponline.com.tw/event/20261111/"
EVENT_URL = EVENT_BASE + "index.php"
INELIGIBLE = "尚未達成活動參加條件"
# 卡雷兒 is in the 拉麥爾/卡蕾兒/阿勒斯 group; match the text, not the value.
SERVER_GROUP_LABEL = "拉麥爾"
MAX_DRAWS = 10  # 2 per day is the cap; never trust a page loop forever
RET_MSG = {
    "-1": "請先登入（session 過期）",
    "-2": "活動期間外",
    "-3": "還沒選取伺服器",
    "-4": "今天已經領取過獎勵",
}


def decode_body(response):
    """The event pages are UTF-8 while the older site scripts are Big5."""
    raw = response.body()
    for encoding in ("utf-8", "big5", "cp950"):
        try:
            return raw.decode(encoding)
        except UnicodeDecodeError:
            continue
    return raw.decode("utf-8", errors="replace")


def pick_server(context):
    """One-time 領獎伺服器 pick for this event. Returns (ok, message)."""
    html = decode_body(context.request.get(EVENT_BASE + "select.php", timeout=30000))
    if INELIGIBLE in html:
        return False, "未達成活動參加條件"
    options = re.findall(r'<option value="([^"]+)">([^<]+)</option>', html)
    choice = next(
        ((value, label) for value, label in options if SERVER_GROUP_LABEL in label),
        None,
    )
    if not choice or choice[0] == "no":
        return False, f"select.php 找不到「{SERVER_GROUP_LABEL}」組（{options}）"
    value, label = choice
    context.request.post(
        EVENT_BASE + "select.php", form={"server": value}, timeout=30000
    )
    return True, f"已選擇伺服器 {value}（{label}）"


def process_account(p, acc):
    username = acc["username"]
    session = session_path(username)
    if not session.exists():
        return False, "冇 session（請先跑 checkin.py 登入）"

    browser = p.chromium.launch(headless=True)
    context = browser.new_context(
        storage_state=str(session), ignore_https_errors=True
    )
    try:
        response = context.request.get(EVENT_URL, timeout=30000)
        html = decode_body(response)
        if "login" in response.url:
            return False, "session 過期，要重新登入（請先跑 checkin.py）"
        if INELIGIBLE in html:
            return False, "未達成活動參加條件（冇抽獎資格）"
        if "select.php" in response.url or "請選擇伺服器" in html:
            ok, message = pick_server(context)
            print(f"  → {message}")
            if not ok:
                return False, message

        items = []
        for _ in range(MAX_DRAWS):
            data = post_json(context, EVENT_BASE + "get.php", {})
            ret = str(data.get("RetVal", ""))
            item = str(data.get("item_name", "") or "")
            chance = str(data.get("today_chance", "") or "")
            if ret == "Y":
                items.append(item or "?")
                print(f"  ✓ 抽到：{item}（今日剩餘 {chance} 次）")
                if chance in ("", "0"):
                    break
                time.sleep(0.6)
                continue
            if ret == "-12":
                print(f"  … 排隊中（{item}），10s 後重試")
                time.sleep(10)
                continue
            if ret == "-4":
                print("  … 今天已經抽完")
                return True, "、".join(items) or "今天已抽完"
            print(f"  ⛔ {RET_MSG.get(ret, f'未知回應 {data}')}")
            return False, RET_MSG.get(ret, f"未知回應 {data}")
        return True, "、".join(items) if items else "冇抽獎次數"
    finally:
        context.close()
        browser.close()


def main():
    cfg = load_config()
    accounts = cfg.get("accounts", [])
    if not accounts:
        print("[!] 無帳號設定（請編輯 checkin_config.yaml）")
        return

    print(f"共 {len(accounts)} 個帳號\n")
    results = []
    with sync_playwright() as p:
        for acc in accounts:
            username = acc.get("username", "?")
            print(f"=== {username} ===")
            try:
                ok, message = process_account(p, acc)
            except Exception as e:
                ok, message = False, f"錯誤: {e}"
            results.append((username, ok, message))

    print("\n=== 結果 ===")
    for username, ok, message in results:
        print(f"  {'✓' if ok else '✗'} {username}: {message}")


if __name__ == "__main__":
    main()
