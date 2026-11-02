@echo off
rem Runs cargo with an in-workspace CARGO_HOME so the DSH file sandbox does not need to allow
rem writes to %USERPROFILE%\.cargo. See README.md for the one-time cache population and the gates.
rem Usage: cargo.cmd test --workspace --offline
rem A .cmd wrapper, not .ps1: Windows PowerShell blocks unsigned script files by policy.
rem Cargo is called by absolute path: a bare `cargo` would resolve to this very script, because
rem command lookup checks the current directory before PATH.

setlocal
set "ROOT=%~dp0"
set "CARGO_HOME=%ROOT%.scratch\cargo-home"
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"

"%USERPROFILE%\.cargo\bin\cargo.exe" %*
exit /b %ERRORLEVEL%
