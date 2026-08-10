@echo off
setlocal
set ROOT=%~dp0..

pushd "%ROOT%"
mise exec -- cargo run --locked
set EXIT=%errorlevel%
popd
exit /b %EXIT%
