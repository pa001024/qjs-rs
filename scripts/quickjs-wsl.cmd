@echo off
setlocal

rem Forward QuickJS CLI invocations to the Linux binary via WSL.
rem This keeps benchmark comparator integration working on Windows hosts.
set "FIRST=%~1"

if "%FIRST%"=="" (
  wsl /mnt/d/dev/QuickJS/qjs
  set "RC=%ERRORLEVEL%"
  endlocal & exit /b %RC%
)

rem Preserve option-style invocations like --version / -e.
if "%FIRST:~0,1%"=="-" (
  wsl /mnt/d/dev/QuickJS/qjs %*
  set "RC=%ERRORLEVEL%"
  endlocal & exit /b %RC%
)

rem If first arg is a Windows file path, translate it for WSL.
for /f "usebackq delims=" %%I in (`wsl wslpath -a "%FIRST%"`) do set "FIRST_WSL=%%I"
shift
wsl /mnt/d/dev/QuickJS/qjs "%FIRST_WSL%" %*
set "RC=%ERRORLEVEL%"
endlocal & exit /b %RC%
