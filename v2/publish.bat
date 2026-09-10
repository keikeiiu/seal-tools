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

set RELTAG=v2.3
set PUB=SealTools.Launcher\bin\Release\net8.0-windows\win-x64\publish
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

echo [1/5] Publishing self-contained single-file exe...
dotnet publish SealTools.Launcher -c Release -r win-x64 --self-contained ^
    -p:PublishSingleFile=true -p:DebugType=None -p:DebugSymbols=false
if errorlevel 1 (
  echo [!] Publish failed.
  pause
  exit /b 1
)

:: Clear any models/config left by a previous mode, so a public build can never
:: ship a local.yaml or calibration screenshot that a local build left behind.
echo [2/5] Clearing stale models + config...
if exist %PUB%\models rd /s /q %PUB%\models
if exist %PUB%\config rd /s /q %PUB%\config

echo [3/5] Copying models...
if not exist models (
  echo [!] models\ folder missing. Copy the PP-OCRv4 .onnx files there first:
  echo     ch_PP-OCRv4_det_infer.onnx
  echo     ch_PP-OCRv4_rec_infer.onnx
  echo     ch_ppocr_mobile_v2.0_cls_infer.onnx
  echo     ^(they come from the Python rapidocr-onnxruntime package models\ dir^)
) else (
  xcopy models %PUB%\models\ /E /I /Y >nul
)

echo [4/5] Copying config (%MODE%)...
if /i "%MODE%"=="local" (
  xcopy config %PUB%\config\ /E /I /Y >nul
  echo       full config - includes local.yaml + calibration screenshots
) else (
  mkdir %PUB%\config
  copy /Y config\attributes.yaml    %PUB%\config\ >nul
  copy /Y config\defaults.yaml      %PUB%\config\ >nul
  copy /Y config\local.yaml.example %PUB%\config\ >nul
  echo       template config only - local.yaml re-seeded on first run
)

echo [5/5] Zipping %ZIPNAME%...
if not exist dist mkdir dist
powershell -NoProfile -ExecutionPolicy Bypass -Command ^
  "Compress-Archive -Path '%PUB%\*' -DestinationPath '%ZIP%' -Force"
if errorlevel 1 (
  echo [!] Zip failed.
  pause
  exit /b 1
)

echo.
echo Distributable: %ZIP%
echo.
echo To run on the target PC (no Python/.NET needed):
echo   1. Unzip %ZIPNAME%.
echo   2. Run SealTools.Launcher.exe. In "public" mode config\local.yaml is
echo      created automatically from config\local.yaml.example on first run.
echo   3. Calibrate once per machine in the launcher's "Calibrate Tuner" and
echo      "Calibrate Gem" tabs (see docs\CALIBRATION.md), then start a tool.
pause
