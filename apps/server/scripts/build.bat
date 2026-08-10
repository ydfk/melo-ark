@echo off
setlocal
set ROOT=%~dp0..

pushd "%ROOT%"
mise exec -- cargo build --release --locked
set EXIT=%errorlevel%
popd

if %EXIT% neq 0 exit /b %EXIT%
echo Build complete: %ROOT%\target\release\meloark-server.exe
