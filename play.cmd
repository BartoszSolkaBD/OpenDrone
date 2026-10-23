@echo off
rem Gets OpenDrone running from this checkout, with its textures: see docs/pilot-guide.md.
cd /d "%~dp0"
where cargo >nul 2>nul
if errorlevel 1 (
  echo Rust isn't installed here: get rustup from https://rustup.rs, open a new terminal, and run play again.
  exit /b 1
)
cargo xtask play %*
exit /b %errorlevel%
