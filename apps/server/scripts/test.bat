@echo off
setlocal
set ROOT=%~dp0..

pushd "%ROOT%"
mise exec -- cargo fmt --all -- --check
if errorlevel 1 goto :fail
mise exec -- cargo clippy --all-targets --all-features -- -D warnings
if errorlevel 1 goto :fail
mise exec -- cargo test --all-targets --all-features --locked
set EXIT=%errorlevel%
popd
exit /b %EXIT%

:fail
set EXIT=%errorlevel%
popd
exit /b %EXIT%
