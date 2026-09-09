@echo off
setlocal
chcp 65001 >nul
echo ==============================================
echo   Seal Tools v2 - Build & Package (.exe)
echo ==============================================
echo.

where dotnet >nul 2>nul
if errorlevel 1 (
  echo [!] .NET 8 SDK not found.
  echo     Install: https://dotnet.microsoft.com/download/dotnet/8.0
  pause
  exit /b 1
)

echo [1/3] Publishing self-contained single-file exe...
dotnet publish SealTools.Launcher -c Release -r win-x64 --self-contained ^
    -p:PublishSingleFile=true -p:DebugType=None -p:DebugSymbols=false
if errorlevel 1 (
  echo [!] Publish failed.
  pause
  exit /b 1
)

set PUB=SealTools.Launcher\bin\Release\net8.0-windows\win-x64\publish

echo [2/3] Copying models + config...
if not exist models (
  echo [!] models\ folder missing. Copy the PP-OCRv4 .onnx files there first:
  echo     ch_PP-OCRv4_det_infer.onnx
  echo     ch_PP-OCRv4_rec_infer.onnx
  echo     ch_ppocr_mobile_v2.0_cls_infer.onnx
  echo     ^(they come from the Python rapidocr-onnxruntime package models\ dir^)
) else (
  xcopy models %PUB%\models\ /E /I /Y >nul
)
xcopy config %PUB%\config\ /E /I /Y >nul

echo [3/3] Done.
echo.
echo Distributable: %PUB%
echo.
echo To run on the target PC (no Python/.NET needed):
echo   1. Copy the whole publish folder.
echo   2. Run SealTools.Launcher.exe. config\local.yaml is created automatically
echo      from config\local.yaml.example on first run.
echo   3. Calibrate once per machine in the launcher's "Calibrate Tuner" and
echo      "Calibrate Gem" tabs (see docs\CALIBRATION.md), then start a tool.
pause
