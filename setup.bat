@echo off
setlocal
chcp 65001 >nul
echo ==============================================
echo   Seal Online Automation Tools - Setup
echo ==============================================
echo.

where python >nul 2>nul
if errorlevel 1 (
    echo [!] Python not found. Install Python 3.12 64-bit from https://python.org
    echo     and tick "Add Python to PATH" during install.
    echo     NOTE: use 3.12 64-bit - newer 3.13+ is not supported by the OCR dependency.
    pause
    exit /b 1
)

rem ---- Step 1: upgrade pip (wrapped so a failure here doesn't abort the whole setup) ----
echo [1/4] Upgrading pip...
python -m pip install --upgrade pip
if errorlevel 1 echo     [warn] pip upgrade failed - continuing with current pip.

rem ---- Step 2: install dependencies ----
echo [2/4] Installing Python dependencies...
python -m pip install -r requirements.txt
if errorlevel 1 (
    echo.
    echo [!] Dependency install FAILED. Run this command manually to see the real error:
    echo     python -m pip install -r requirements.txt
    echo.
    echo     Common causes:
    echo       - Not on Python 3.12 64-bit. Run:  python --version
    echo       - Slow/unstable network - try again later.
    pause
    exit /b 1
)

rem ---- Step 3: browser for check-in ----
echo [3/4] Installing browser for check-in (one-time download)...
python -m playwright install chromium
if errorlevel 1 (
    echo     [warn] Browser download failed - check-in won't run, but other tools will.
    echo     Fix later with:  python -m playwright install chromium
)

rem ---- Step 4: check Arduino ----
echo [4/4] Checking Arduino...
python -c "import serial.tools.list_ports as p; ports=[x.device for x in p.comports() if x.vid==0x2341]; print('   Arduino:', ports[0] if ports else 'NOT FOUND (plug in the Pro Micro)')"

echo.
echo ==============================================
echo   Done.
echo   Launcher:  python launcher.py
echo   Open:      http://127.0.0.1:5002
echo   Check-in:  python checkin\checkin.py
echo ==============================================
pause
