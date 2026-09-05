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

echo [2/3] Copying models + config + panel...
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
copy launcher.html %PUB%\ >nul

echo [3/3] Done.
echo.
echo Distributable: %PUB%
echo.
echo To run on the target PC (no Python/.NET needed):
echo   1. Copy the whole publish folder.
echo   2. First run: copy config\local.yaml.example  ->  config\local.yaml
echo   3. SealTools.Launcher.exe --autoanchor   (scales coords to the window size)
echo   4. SealTools.Launcher.exe                (launcher panel on http://127.0.0.1:5003)
echo      Use the in-browser "Calibrate" tool to fine-tune, or --diagnose to verify.
pause
