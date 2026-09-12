@echo off
setlocal
rem Windows helper: avoid `>python -m kagra` (cmd treats `>` as redirect).
if exist "%~dp0.venv\Scripts\python.exe" (
  "%~dp0.venv\Scripts\python.exe" -m kagra %*
  exit /b %ERRORLEVEL%
)
where py >nul 2>&1
if %ERRORLEVEL%==0 (
  py -3 -m kagra %*
  exit /b %ERRORLEVEL%
)
where python >nul 2>&1
if %ERRORLEVEL%==0 (
  python -m kagra %*
  exit /b %ERRORLEVEL%
)
echo [kagra] Python 3.10+ not found. Install from https://www.python.org/ and tick "Add python.exe to PATH".
echo [kagra] Then:  py -3 -m pip install kagra
echo [kagra]         py -3 -m kagra
exit /b 1
