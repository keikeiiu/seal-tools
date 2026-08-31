@echo off
chcp 65001 >nul
echo ==============================================
echo   Seal Online Automation Tools — Setup
echo ==============================================
echo.

where python >nul 2>nul
if errorlevel 1 (
    echo [!] Python not found. Install Python 3.10+ from https://python.org
    echo     and tick "Add Python to PATH" during install.
    pause
    exit /b 1
)

echo [1/2] Installing Python dependencies...
python -m pip install --upgrade pip
python -m pip install -r requirements.txt
if errorlevel 1 (
    echo [!] Dependency install failed.
    pause
    exit /b 1
)

echo.
echo [2/2] Checking Arduino (COM port)...
python -c "import serial.tools.list_ports as p; ports=[x.device for x in p.comports() if x.vid==0x2341]; print('   Arduino:', ports[0] if ports else 'NOT FOUND — plug in the Pro Micro')"

echo.
echo ==============================================
echo   Done.
echo   Start the launcher with:  python launcher.py
echo   Then open:                 http://127.0.0.1:5002
echo ==============================================
pause
