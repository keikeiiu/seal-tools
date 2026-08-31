# Seal Online 每日簽到 Automation

Automates the daily **簽到 (check-in)** on the Seal Online 七夕簽到簿 event page for multiple accounts.

Each account logs in once (you solve the reCAPTCHA by hand), the session is saved, and reused every day until it expires — so daily runs are fully headless with no manual captcha.

## What It Does

1. Loads each account's saved login session (Playwright `storage_state`)
2. Navigates to the event check-in page and confirms the session is still valid
3. Clicks the earliest unclaimed day button (each account progresses independently)
4. Reads the reward result from the on-page dialog and reports it
5. If a session has expired → opens a visible browser, fills credentials, waits for you to solve the reCAPTCHA, saves the fresh session

## Requirements

```bash
pip install playwright pyyaml
python -m playwright install chromium
```

## Files

```
checkin/
├── checkin.py               ← Main script (run this)
├── checkin_config.yaml      ← Accounts + URLs (edit this)
├── README.md
└── sessions/                ← Saved login sessions (gitignored — 勿提交)
    ├── account1.json
    ├── account2.json
    └── ...
```

## Configuration (`checkin_config.yaml`)

```yaml
# Login page + event check-in page
login_url: "https://security.sponline.com.tw/login/login.php"
checkin_url: "https://security.sponline.com.tw/event/20260806/"

accounts:
  - username: "account4"
    password: "REDACTED"
  - username: "account3"
    password: "REDACTED"
  # ... one entry per account
```

> **Note:** the event `checkin_url` changes with each new activity (each event runs ~28 days). Update it in the config when a new event starts — the script does not auto-detect it.

## Usage

```bash
cd "SEALONLINE SCRIPTS/checkin"
python checkin.py
```

### First run (per account)

- The script opens a **visible browser**, fills the username/password, and pauses.
- **You must solve the reCAPTCHA in that browser window** (up to 2 minutes).
- Once solved, the script submits, saves the session to `sessions/<username>.json`, and checks in.

### Subsequent runs

- Sessions are loaded headless (no browser window, no captcha).
- Each account checks in its next unclaimed day and prints the reward.

### Example output

```
=== account4 ===
[account4] 使用已存 session
  → 簽到第 16 天...
  [結果] 恭喜獲得
[活動]寵物便當盒-1天*1

=== 結果 ===
  ✓ account4: 第 16 天簽到完成
  ✓ account3: 第 18 天簽到完成
  ...

成功 7/7。session 已存，下次免重新登入（直到過期）。
```

## How It Works (technical)

| Piece | Detail |
|-------|--------|
| **Login form** | `#userID` (帳號), `#userPW` (密碼), submitted via `$('form').submit()` |
| **reCAPTCHA** | Polls the hidden `login_recaptcha` / `.g-recaptcha-response` field until you solve it, then auto-submits |
| **Day buttons** | Each unclaimed day is `<a href="javascript:get(N)">` (N = 1–28); the script clicks the smallest N still present |
| **Result dialog** | jQuery modal `#dialog-message` (auto-closes) — polled for text like `恭喜獲得` / `你今天已經領過囉` |
| **Session** | Saved via `context.storage_state(path=...)`; loaded via `browser.new_context(storage_state=...)` |

## Notes

- **Do not commit `sessions/`** — it contains login tokens. It's listed in the parent `.gitignore`.
- **Passwords** are stored in plaintext in `checkin_config.yaml` — keep this file private too.
- Sessions expire server-side after some period; when that happens the script falls back to the manual-captcha login flow automatically.
- Each account's check-in progress is independent — a later-created account lags behind on day number.
