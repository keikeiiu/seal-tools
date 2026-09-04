@echo off
setlocal
chcp 65001 >nul
echo ==============================================
echo   Seal Online Automation Tools - Setup
echo ==============================================
echo.

rem ---- Pick the Python command: prefer "python", fall back to "py" ----
set PY=python
where python >nul 2>nul
if errorlevel 1 (
    set PY=py
    where py >nul 2>nul
    if errorlevel 1 (
        echo [!] Neither "python" nor "py" found.
        echo     Install Python 3.12 64-bit from https://python.org
        echo     and tick "Add Python to PATH" during install.
        echo     NOTE: use 3.12 64-bit - newer 3.13+ is not supported by the OCR dependency.
        echo     Hint: if "python" does nothing but "py" works, just use "py" for everything.
        pause
        exit /b 1
    )
    echo [i] "python" not on PATH - using the "py" launcher instead.
)
echo [i] Using Python command: %PY%

rem ---- Step 1: upgrade pip (wrapped so a failure here doesn't abort the whole setup) ----
echo [1/5] Upgrading pip...
%PY% -m pip install --upgrade pip
if errorlevel 1 echo     [warn] pip upgrade failed - continuing with current pip.

rem ---- Step 2: install dependencies ----
echo [2/5] Installing Python dependencies...
%PY% -m pip install -r requirements.txt
if errorlevel 1 (
    echo.
    echo [!] Dependency install FAILED. Run this command manually to see the real error:
    echo     %PY% -m pip install -r requirements.txt
    echo.
    echo     Common causes:
    echo       - Not on Python 3.12 64-bit. Run:  %PY% --version
    echo       - Slow/unstable network - try again later.
    pause
    exit /b 1
)

rem ---- Step 3: verify the OCR engine can actually load (needs VC++ runtime) ----
echo [3/5] Verifying OCR engine...
%PY% -c "import onnxruntime, rapidocr_onnxruntime; print('   OCR OK')" >nul 2>nul
if errorlevel 1 (
    echo [!] OCR engine failed to load - most likely the Microsoft Visual C++ Redistributable
    echo     is missing on a fresh PC. Installing it automatically...
    where curl >nul 2>nul
    if not errorlevel 1 (
        curl -L -o "%TEMP%\vc_redist.x64.exe" https://aka.ms/vs/17/release/vc_redist.x64.exe
    ) else (
        powershell -Command "Invoke-WebRequest -OutFile \"%TEMP%\vc_redist.x64.exe\" https://aka.ms/vs/17/release/vc_redist.x64.exe"
    )
    "%TEMP%\vc_redist.x64.exe" /install /quiet /norestart
    echo     Re-checking OCR...
    %PY% -c "import onnxruntime, rapidocr_onnxruntime; print('   OCR OK')" >nul 2>nul
    if errorlevel 1 (
        echo.
        echo [!] Still failing. Install the Redistributable manually (it needs Administrator):
        echo     https://aka.ms/vs/17/release/vc_redist.x64.exe
        echo     Then retry:  %PY% -m pip install --force-reinstall rapidocr-onnxruntime
        pause
        exit /b 1
    )
    echo     OCR OK after installing VC++ runtime.
)

rem ---- Step 4: browser for check-in ----
echo [4/5] Installing browser for check-in (one-time download)...
%PY% -m playwright install chromium
if errorlevel 1 (
    echo     [warn] Browser download failed - check-in won't run, but other tools will.
    echo     Fix later with:  %PY% -m playwright install chromium
)

rem ---- Step 5: check Arduino ----
echo [5/5] Checking Arduino...
%PY% -c "import serial.tools.list_ports as p; ports=[x.device for x in p.comports() if x.vid==0x2341]; print('   Arduino:', ports[0] if ports else 'NOT FOUND (plug in the Pro Micro)')"

echo.
echo ==============================================
echo   Done.
echo   Launcher:  %PY% launcher.py
echo   Open:      http://127.0.0.1:5002
echo   Check-in:  %PY% checkin\checkin.py
echo ==============================================
pause
