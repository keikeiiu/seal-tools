@echo off
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

echo [1/3] Installing Python dependencies...
python -m pip install --upgrade pip
python -m pip install -r requirements.txt
if errorlevel 1 (
    echo [!] Dependency install failed.
    pause
    exit /b 1
)

echo.
echo [2/3] Installing browser for check-in (one-time download)...
python -m playwright install chromium

echo.
echo [3/3] Checking Arduino...
python -c "import serial.tools.list_ports as p; ports=[x.device for x in p.comports() if x.vid==0x2341]; print('   Arduino:', ports[0] if ports else 'NOT FOUND (plug in the Pro Micro)')"

echo.
echo ==============================================
echo   Done.
echo   Launcher:  python launcher.py
echo   Open:      http://127.0.0.1:5002
echo   Check-in:  python checkin\checkin.py
echo ==============================================
pause
