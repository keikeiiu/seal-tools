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

set RELTAG=v2.16
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
  :: The shipped defaults go out under the TEMPLATE name. The app reads config\defaults.yaml.example
  :: as its BASE when it exists, and that name is what an update replaces - so extracting an update
  :: cannot cover the player's own config\defaults.yaml, which is where every setting they ever
  :: saved lives - the old save path wrote the whole portable projection into it.
  copy /Y config\defaults.yaml      %PUB%\config\defaults.yaml.example >nul
  copy /Y config\local.yaml.example %PUB%\config\ >nul
  :: The US dictionary TEMPLATE, and the only US dictionary that ships. The loader SEEDS a missing
  :: one from this file - the missing-file error's own instruction, automated - so shipping the
  :: empty dictionary as well would put a file the player grows in place into the zip.
  copy /Y config\attributes.us.yaml.example %PUB%\config\ >nul
  mkdir %PUB%\docs
  :: The pet feeding table, under docs\ beside the data file the repo tracks. It was in no zip
  :: until now, which is why an installed copy has never had a feeding estimate.
  copy /Y docs\pet-data.csv        %PUB%\docs\ >nul
  echo       template config only - local.yaml re-seeded on first run
)

:: The template that ships has to BE the released one. Until the launcher stops writing it (step 4
:: of PLAN-DEPLOY.md), a Save on any config tab rewrites the tracked config\defaults.yaml, and this
:: script would then package the developer's own values as every player's template. Compared
:: through git rather than bytes on purpose: the app's writer emits CRLF, so a byte comparison
:: against the blob would fail on line endings alone and the guard would be noise.
if /i "%MODE%"=="public" (
  git diff --quiet HEAD -- config/defaults.yaml
  if errorlevel 1 (
    echo [!] config\defaults.yaml differs from HEAD - your saved values would ship as the
    echo     template every player starts from. Put it back and run this script again:
    echo       git checkout -- config/defaults.yaml
    pause
    exit /b 1
  )
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

:: The rule this release rests on: NO archive member is a file the app writes, or a file the player
:: is told to grow in place. Checked as an ALLOW-list rather than a list of forbidden names, so a
:: future copy line for a new writer's file fails HERE instead of shipping quietly - and so does a
:: missing program file.
if /i "%MODE%"=="public" (
  powershell -NoProfile -ExecutionPolicy Bypass -Command ^
    "$root = (Resolve-Path '%PUB%').Path; $cfg = @('config/attributes.yaml','config/defaults.yaml.example','config/local.yaml.example','config/attributes.us.yaml.example'); $need = @('config/defaults.yaml.example','config/local.yaml.example','config/attributes.us.yaml.example','config/attributes.yaml','docs/pet-data.csv'); $bad = @(); foreach ($f in Get-ChildItem -Recurse -File '%PUB%') { $n = $f.FullName.Substring($root.Length + 1).Replace('\','/'); if ($n -like 'config/*') { if ($cfg -notcontains $n) { $bad += $n } } elseif ($n -eq 'SealTools.Launcher.exe') { } elseif ($n -match '\.dll$') { } elseif ($n -match '^models/.+\.onnx$') { } elseif ($n -eq 'docs/pet-data.csv') { } else { $bad += $n } }; foreach ($m in $need) { if (-not (Test-Path (Join-Path $root $m))) { $bad += 'MISSING: ' + $m } }; if ($bad.Count -gt 0) { $bad | ForEach-Object { Write-Host ('[!] ' + $_) }; exit 1 }"
  if errorlevel 1 (
    echo [!] The public zip carries a file a player owns, or is missing one it needs.
    echo     Nothing was published. The offending names are listed above.
    pause
    exit /b 1
  )
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
