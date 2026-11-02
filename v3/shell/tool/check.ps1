# The shell quality gate: every change runs this before it is called done.
#
# It is intentionally the same shape as the Rust side (fmt/clippy/test): formatting, the analyzer
# with infos fatal, the widget/unit suite, and a Windows debug build so a missing platform register
# does not wait for a release. Run it from anywhere:
#
#   pwsh -File v3/shell/tool/check.ps1
#   pwsh -File v3/shell/tool/check.ps1 -FlutterRoot 'D:\flutter'
#
# `FLUTTER_ROOT` is honoured when the SDK is not under the default `C:\src\flutter` (the README's
# recorded toolchain location; Flutter is not on PATH on this machine by design).
param(
    [string]$FlutterRoot = $(if ($env:FLUTTER_ROOT) { $env:FLUTTER_ROOT } else { 'C:\src\flutter' })
)

$ErrorActionPreference = 'Stop'
$shell = Split-Path $PSScriptRoot -Parent
$flutter = Join-Path $FlutterRoot 'bin\flutter.bat'
$dart = Join-Path $FlutterRoot 'bin\cache\dart-sdk\bin\dart.exe'

if (-not (Test-Path -LiteralPath $flutter)) {
    throw "Flutter not found at '$flutter'. Set FLUTTER_ROOT or pass -FlutterRoot."
}
if (-not (Test-Path -LiteralPath $dart)) {
    throw "Dart not found at '$dart' (expected beside the Flutter SDK)."
}

function Assert-LastExit([string]$Step) {
    if ($LASTEXITCODE -ne 0) {
        throw "$Step failed (exit $LASTEXITCODE)"
    }
}

Push-Location $shell
try {
    Write-Host '== dart format ==' -ForegroundColor Cyan
    & $dart format --output=none --set-exit-if-changed lib test
    Assert-LastExit 'dart format'

    Write-Host '== flutter analyze --fatal-infos ==' -ForegroundColor Cyan
    & $flutter analyze --fatal-infos
    Assert-LastExit 'flutter analyze'

    Write-Host '== flutter test ==' -ForegroundColor Cyan
    & $flutter test --no-pub --reporter compact
    Assert-LastExit 'flutter test'

    Write-Host '== flutter build windows --debug ==' -ForegroundColor Cyan
    & $flutter build windows --debug --no-pub
    Assert-LastExit 'flutter build windows'

    Write-Host 'Shell quality gate passed.' -ForegroundColor Green
}
finally {
    Pop-Location
}
