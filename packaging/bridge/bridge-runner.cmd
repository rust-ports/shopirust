@echo off
setlocal
set "SCRIPT_DIR=%~dp0"
if exist "%SCRIPT_DIR%node\node.exe" (
  "%SCRIPT_DIR%node\node.exe" "%SCRIPT_DIR%bridge-runner.mjs" %*
  exit /b %ERRORLEVEL%
)
where node >nul 2>nul
if %ERRORLEVEL% EQU 0 (
  node "%SCRIPT_DIR%bridge-runner.mjs" %*
  exit /b %ERRORLEVEL%
)
echo Shopify CLI bridge requires its bundled Node runtime. Reinstall the release artifact. 1>&2
exit /b 1
