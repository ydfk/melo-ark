@echo off
setlocal
set ROOT=%~dp0..
set IMAGE_TAG=%~1
if "%IMAGE_TAG%"=="" set IMAGE_TAG=meloark:local

where docker >nul 2>nul
if errorlevel 1 goto :missing
docker info >nul 2>nul
if errorlevel 1 goto :stopped

pushd "%ROOT%"
docker build --tag "%IMAGE_TAG%" .
set EXIT=%errorlevel%
popd
exit /b %EXIT%

:missing
echo [ERROR] Docker was not found. Install and start Docker Desktop first.
exit /b 1

:stopped
echo [ERROR] Docker daemon is unavailable. Start Docker Desktop first.
exit /b 1
