@echo off
setlocal
chcp 65001 >nul

:: ── Mode ─────────────────────────────────────────────
::   public  (default)  template config only — safe to publish on GitHub
::   local              full config incl. your calibration — your own machine
set MODE=%1
if "%MODE%"=="" set MODE=public
if /i "%MODE%"=="public" goto :mode_ok
if /i "%MODE%"=="local"  goto :mode_ok
echo [!] Unknown mode "%MODE%". Usage: publish.bat [public^|local]
exit /b 1
:mode_ok

set RELTAG=v2.12
set PUB=SealTools.Launcher\bin\Release\net8.0-windows\win-x64\publish
:: The sketch lives at the repo root, beside v2\ - not under it. This script is run from v2\.
:: Arduino needs the .ino inside a folder of the same name, so the zip keeps the folder.
set SKETCH=..\arduino\seal_mouse
if /i "%MODE%"=="public" (set ZIPNAME=SealTools-%RELTAG%.zip) else (set ZIPNAME=SealTools-%RELTAG%-local.zip)
set ZIP=dist\%ZIPNAME%

echo ==============================================
echo   Seal Tools %RELTAG% - Build ^& Package (%MODE%)
echo ==============================================
echo.

where dotnet >nul 2>nul
if errorlevel 1 (
  echo [!] .NET 8 SDK not found.
  echo     Install: https://dotnet.microsoft.com/download/dotnet/8.0
  pause
  exit /b 1
)

echo [1/6] Publishing self-contained single-file exe (clean)...
:: Delete the whole publish folder first: an incremental publish skips the native
:: OCR DLLs, leaving an exe that cannot load onnxruntime / OpenCV at runtime.
if exist %PUB% rd /s /q %PUB%
dotnet publish SealTools.Launcher -c Release -r win-x64 --self-contained ^
    -p:PublishSingleFile=true -p:DebugType=None -p:DebugSymbols=false
if errorlevel 1 (
  echo [!] Publish failed.
  pause
  exit /b 1
)

:: NuGet content also ships an 89 MB libSkiaSharp.pdb and *.lib import libraries;
:: DebugType=None only stops our own symbols, so strip the leftovers here.
echo [2/6] Stripping debug symbols + import libraries...
del /q %PUB%\*.pdb 2>nul
del /q %PUB%\*.lib 2>nul

echo [3/6] Copying models...
if not exist models (
  echo [!] models\ folder missing. Copy the PP-OCRv4 .onnx files there first:
  echo     ch_PP-OCRv4_det_infer.onnx
  echo     ch_PP-OCRv4_rec_infer.onnx
  echo     ch_ppocr_mobile_v2.0_cls_infer.onnx
  echo     ^(they come from the Python rapidocr-onnxruntime package models\ dir^)
) else (
  xcopy models %PUB%\models\ /E /I /Y >nul
)

echo [4/6] Copying config (%MODE%)...
if /i "%MODE%"=="local" (
  xcopy config %PUB%\config\ /E /I /Y >nul
  :: Backups kept beside the live local.yaml are not part of a release. xcopy has no name-pattern
  :: exclude, so they are removed after the copy rather than filtered during it.
  del /q "%PUB%\config\local.yaml.backup-*" 2>nul
  del /q "%PUB%\config\local.yaml.corrupt-backup" 2>nul
  echo       full config - includes local.yaml + calibration screenshots
) else (
  mkdir %PUB%\config
  copy /Y config\attributes.yaml    %PUB%\config\ >nul
  copy /Y config\defaults.yaml      %PUB%\config\ >nul
  copy /Y config\local.yaml.example %PUB%\config\ >nul
  echo       template config only - local.yaml re-seeded on first run
)

echo [5/6] Zipping %ZIPNAME%...
if not exist dist mkdir dist
powershell -NoProfile -ExecutionPolicy Bypass -Command ^
  "Compress-Archive -Path '%PUB%\*' -DestinationPath '%ZIP%' -Force"
if errorlevel 1 (
  echo [!] Zip failed.
  pause
  exit /b 1
)

:: The firmware ships as its OWN zip, not inside the app zip: it is not part of the runtime install
:: (nothing extracts it), and a player who already has a flashed board never needs it. It was simply
:: absent from every release until now - publish.bat never mentioned it - so the sketch was only
:: reachable from the repo.
echo [6/6] Zipping the Arduino firmware...
set FWZIP=dist\SealTools-%RELTAG%-firmware.zip
if not exist "%SKETCH%\seal_mouse.ino" (
  echo [!] %SKETCH%\seal_mouse.ino not found - skipping the firmware zip.
  echo     Run this script from the v2\ folder, with the repo root one level up.
) else (
  powershell -NoProfile -ExecutionPolicy Bypass -Command ^
    "Compress-Archive -Path '%SKETCH%' -DestinationPath '%FWZIP%' -Force"
  if errorlevel 1 (
    echo [!] Firmware zip failed.
  ) else (
    echo       %FWZIP%
  )
)

echo.
echo Distributables:
echo   %ZIP%    (app)
echo   %FWZIP%    (Arduino firmware - flash with the Arduino IDE)
echo.
echo To run on the target PC (no Python/.NET needed):
echo   1. Unzip %ZIPNAME%.
echo   2. Run SealTools.Launcher.exe. In "public" mode config\local.yaml is
echo      created automatically from config\local.yaml.example on first run.
echo   3. Calibrate once per machine in the launcher's "Calibrate Tuner" and
echo      "Calibrate Gem" tabs (see docs\CALIBRATION.md), then start a tool.
pause
