# Seal Online 每日簽到 Automation

Automates the daily **簽到 (check-in)** on the current Seal Online monthly check-in event page for multiple accounts.

Each account logs in once (you solve the reCAPTCHA by hand), the session is saved, and reused every day until it expires — so daily runs are fully headless with no manual captcha.

## What It Does

1. Loads each account's saved login session (Playwright `storage_state`)
2. Navigates to the event check-in page and confirms the session is still valid
3. Claims the next unclaimed day through `get.php` (each account progresses independently)
4. Reads the reward result from the response and reports it
5. Spends any remaining 補簽 (makeup) chances on missed days
6. If a session has expired → opens a visible browser, fills credentials, waits for you to solve the reCAPTCHA, saves the fresh session

## Requirements

```bash
pip install playwright pyyaml
python -m playwright install chromium
```

## Files

```
checkin/
├── checkin.py               ← Main script (run this)
├── run_checkin.ps1          ← Unattended entry point for the 00:05 scheduled task
├── checkin_config.yaml      ← Accounts + URLs (edit this)
├── README.md
├── logs/                    ← One stdout/stderr pair per scheduled run (gitignored)
└── sessions/                ← Saved login sessions (gitignored — 勿提交)
    ├── account1.json
    ├── account2.json
    └── ...
```

## Configuration (`checkin_config.yaml`)

```yaml
# Login page + event check-in page
login_url: "https://security.sponline.com.tw/login/login.php"
checkin_url: "https://security.sponline.com.tw/event/20261104_1/index.php"

# One-time pick of the 領獎伺服器 (irreversible). The value numbering is
# per-event — 20261104 has 2 = 拉麥爾/卡蕾兒/阿勒斯, 1 = 哈比兔/勇者咕咕;
# the 20260930 event had them the other way round.
server: "2"

accounts:
  - username: "your_account_1"
    password: "your_password_1"
  - username: "your_account_2"
    password: "your_password_2"
  # ... one entry per account
```

> **Note:** the event `checkin_url` and the `server` numbering change with each new activity (each event runs ~28 days). Update both when a new event starts — the script does not auto-detect them. Read the new event's `select.php` for its option values before the first run; the pick is one-time and irreversible.

## Usage

```bash
cd "SEALONLINE SCRIPTS/checkin"
python checkin.py
```

### First run (per account)

- The script opens a **visible browser**, fills the username/password, and pauses.
- **You must solve the reCAPTCHA in that browser window** (up to 2 minutes).
- Once solved, the script submits, saves the session to `sessions/<username>.json`, and checks in.
- The first event-page visit also picks the 領獎伺服器 from the config's `server` value — one-time and irreversible.

### Subsequent runs

- Sessions are loaded headless (no browser window, no captcha).
- Each account checks in its next unclaimed day and prints the reward.

### Example output

```
=== keisign4 ===
[keisign4] 使用已存 session
  → 首次進入，選擇伺服器 2 ...
  ✓ 領取第 1 天：[活動]進階擺攤-1天*1
  ✓ 今天已領過，收工

=== 結果 ===
  ✓ keisign4: 第 1 天 → [活動]進階擺攤-1天*1；今天已領過
  ... one block per account ...

成功 7/7。session 已存，下次免重新登入（直到過期）。
```

## Scheduled runs (00:05 and 21:00, all accounts)

A Windows Task Scheduler task named **SealTools CheckIn** starts `run_checkin.ps1`
hidden **twice a day — 00:05 and 21:00** — and it runs `checkin.py` once, so it walks
every account in `checkin_config.yaml`, writing the run's stdout + stderr to
`logs/checkin-<yyyyMMdd-HHmm>.log` and `.err.log` beside it.

Two triggers mean the PC does not have to stay on past midnight: the 21:00 run claims
the current calendar day's reward before you shut down. Only one day can be claimed per
calendar day, and a run that finds today already claimed reports success (`-10`), so
whichever trigger fires first claims and the other becomes a cheap no-op.

```powershell
Get-ScheduledTaskInfo -TaskName 'SealTools CheckIn'                      # last result + next run
Start-ScheduledTask    -TaskName 'SealTools CheckIn'                      # run now, out of schedule
Unregister-ScheduledTask -TaskName 'SealTools CheckIn' -Confirm:$false    # stop the schedule
```

- The task only runs while you are logged on; if the PC is off at both times it starts
  as soon as Windows can (`StartWhenAvailable`), and that catch-up still claims that
  day's reward.
- An expired session still triggers the manual-captcha flow: the unattended run opens a
  visible browser and waits up to 2 minutes per account. Run `python checkin.py` once
  while you are at the PC, and the saved session makes the scheduled runs headless again.
- The schedule is machine state, not repo state. If it goes missing, recreate it with:

```powershell
$action = New-ScheduledTaskAction -Execute "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" `
    -Argument '-NoProfile -NonInteractive -WindowStyle Hidden -ExecutionPolicy Bypass -File "C:\Git_repo\seal-tools\v1\checkin\run_checkin.ps1"'
$triggers = @(
    (New-ScheduledTaskTrigger -Daily -At '00:05'),
    (New-ScheduledTaskTrigger -Daily -At '21:00')
)
Register-ScheduledTask -TaskName 'SealTools CheckIn' -Action $action -Trigger $triggers `
    -Settings (New-ScheduledTaskSettingsSet -StartWhenAvailable -MultipleInstances IgnoreNew)
```

## How It Works (technical)

| Piece | Detail |
|-------|--------|
| **Login form** | `#userID` (帳號), `#userPW` (密碼), submitted via `$('form').submit()` |
| **reCAPTCHA** | Polls the hidden `login_recaptcha` / `.g-recaptcha-response` field until you solve it, then auto-submits |
| **Day buttons** | Each unclaimed day is `<a href="javascript:get(N)">` (N = 1–28). A claimed day loses that link — the server replaces it with a `collar_1` 已領取 marker — so the remaining `get(N)` hrefs are the claimable days |
| **Claim** | `POST get.php {index: N}`; the PHP answers a JS array literal (`RetVal`, `item_name`) parsed with `literal_eval`. `-10` = already claimed today, `-11` = claim in order, `-12` = queue (retried) |
| **Makeup** | The event also exposes 補簽 through `POST get.php {is_makeup: 1}`. After today's claim the run spends every remaining chance (capped at 10 calls) and reports what it filled; `-20`/`-21`/`-22` mean no chances / nothing missed / claim today first |
| **Session** | Saved via `context.storage_state(path=...)`; loaded via `browser.new_context(storage_state=...)` |

## Notes

- **Do not commit `sessions/`** — it contains login tokens. It's listed in the parent `.gitignore`.
- **Passwords** are stored in plaintext in `checkin_config.yaml` — keep this file private too.
- Sessions expire server-side after some period; when that happens the script falls back to the manual-captcha login flow automatically.
- Each account's check-in progress is independent — a later-created account lags behind on day number.
- A run that finds today already claimed reports success (`-10`). The scheduled task may legitimately run twice in one day (e.g. `StartWhenAvailable` catching up), and that must not look like a failure.
