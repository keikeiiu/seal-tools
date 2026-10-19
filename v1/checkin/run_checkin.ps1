# Daily check-in entry point for the "SealTools CheckIn" scheduled task (2026-10-20).
#
# Task Scheduler starts this hidden at 00:05 and it runs checkin.py once for every
# account in checkin_config.yaml. The wrapper exists only for the unattended run:
# a manual `python checkin.py` still shows its console and asks for the reCAPTCHA
# when a session has expired, which is exactly what you want when you run it by hand.
#
# One log pair per run (stdout / stderr) under logs/, which is gitignored so the
# account names and rewards in it never reach the repo.
#
# Keep this file ASCII-only: Windows PowerShell 5.1 reads a BOM-less .ps1 as ANSI
# and would garble any non-ASCII literal.
$ErrorActionPreference = 'Stop'
Set-Location -LiteralPath $PSScriptRoot

$logDir = Join-Path $PSScriptRoot 'logs'
New-Item -ItemType Directory -Force -Path $logDir | Out-Null
$stamp = Get-Date -Format 'yyyyMMdd-HHmm'
$outLog = Join-Path $logDir "checkin-$stamp.log"
$errLog = Join-Path $logDir "checkin-$stamp.err.log"

$started = Get-Date
# -u keeps the log readable while a run is still going (an expired session pauses
# 2 minutes per account waiting for the captcha, and that wait should be visible).
$proc = Start-Process -FilePath 'python' -ArgumentList '-u', 'checkin.py' `
    -WorkingDirectory $PSScriptRoot -NoNewWindow -Wait -PassThru `
    -RedirectStandardOutput $outLog -RedirectStandardError $errLog
$ended = Get-Date

Add-Content -LiteralPath $outLog -Encoding UTF8 -Value (
    "`n[scheduled] exit $($proc.ExitCode) | " +
    "$($started.ToString('yyyy-MM-dd HH:mm:ss')) -> $($ended.ToString('HH:mm:ss'))"
)
exit $proc.ExitCode
