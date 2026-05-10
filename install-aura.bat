@echo off
setlocal EnableExtensions EnableDelayedExpansion

set "SCRIPT_DIR=%~dp0"
if "%SCRIPT_DIR:~-1%"=="\" set "SCRIPT_DIR=%SCRIPT_DIR:~0,-1%"
set "VCS_CORE_DIR=%SCRIPT_DIR%\vcs-core"
set "CARGO_BIN=%USERPROFILE%\.cargo\bin"
set "AURA_EXE=%CARGO_BIN%\aura.exe"
set "SHELL_SETUP_PS1=%SCRIPT_DIR%\install-aura-shell.ps1"
set "VERIFY_DIR=%TEMP%\aura-global-check"

echo.
echo ==========================================
echo   Aura installer
echo ==========================================
echo.

if not exist "%VCS_CORE_DIR%\Cargo.toml" (
    echo [ERROR] "%VCS_CORE_DIR%\Cargo.toml" was not found.
    echo Make sure install-aura.bat is placed in the IDE-Git-Ru project root.
    exit /b 1
)

if not exist "%SHELL_SETUP_PS1%" (
    echo [ERROR] "%SHELL_SETUP_PS1%" was not found.
    echo The PowerShell integration helper must be located next to install-aura.bat.
    exit /b 1
)

where cargo >nul 2>nul
if errorlevel 1 (
    echo [INFO] cargo was not found. Trying to install Rust toolchain via winget...
    where winget >nul 2>nul
    if errorlevel 1 (
        echo [ERROR] winget was not found. Install Rust manually from https://rustup.rs/
        exit /b 1
    )

    winget install --id Rustlang.Rustup -e --accept-source-agreements --accept-package-agreements
    if errorlevel 1 (
        echo [ERROR] Failed to install rustup via winget.
        exit /b 1
    )
)

if not exist "%CARGO_BIN%" (
    mkdir "%CARGO_BIN%" >nul 2>nul
)

echo [INFO] Checking user PATH...
set "USER_PATH="
for /f "tokens=2,*" %%A in ('reg query "HKCU\Environment" /v Path 2^>nul ^| findstr /i "Path"') do set "USER_PATH=%%B"

echo ;!USER_PATH!; | find /i ";%CARGO_BIN%;" >nul
if errorlevel 1 (
    if defined USER_PATH (
        set "NEW_USER_PATH=!USER_PATH!;%CARGO_BIN%"
    ) else (
        set "NEW_USER_PATH=%CARGO_BIN%"
    )
    setx Path "!NEW_USER_PATH!" >nul
    echo [INFO] Added "%CARGO_BIN%" to the user PATH.
) else (
    echo [INFO] "%CARGO_BIN%" is already present in the user PATH.
)

set "PATH=%CARGO_BIN%;%PATH%"

where cargo >nul 2>nul
if errorlevel 1 (
    echo [ERROR] cargo is still unavailable in the current session.
    echo Close the terminal, open it again, and rerun install-aura.bat.
    exit /b 1
)

echo [INFO] Installing local aura binary...
cargo install --path "%VCS_CORE_DIR%" --bin aura --force
if errorlevel 1 (
    echo [ERROR] Failed to install aura.
    exit /b 1
)

if not exist "%AURA_EXE%" (
    echo [ERROR] Installation finished, but "%AURA_EXE%" was not found.
    exit /b 1
)

echo [INFO] Installing PowerShell profile integration and command shim...
powershell -NoLogo -NoProfile -ExecutionPolicy Bypass -File "%SHELL_SETUP_PS1%"
if errorlevel 1 (
    echo [ERROR] Failed to install PowerShell integration for aura.
    exit /b 1
)

where aura >nul 2>nul
if errorlevel 1 (
    echo [WARN] aura is not visible through the current session PATH yet.
    echo [WARN] A PowerShell profile shim was installed to make `aura` available in new terminals.
)
if not errorlevel 1 (
    echo [INFO] aura is now visible through PATH in this session.
)

if not exist "%VERIFY_DIR%" (
    mkdir "%VERIFY_DIR%" >nul 2>nul
)

echo [INFO] Verifying aura launch from a fresh PowerShell session...
powershell -NoLogo -ExecutionPolicy Bypass -Command "Set-Location -LiteralPath '%VERIFY_DIR%'; aura help > $null"
if errorlevel 1 (
    echo [ERROR] aura is still unavailable in a fresh PowerShell session.
    echo Check your PowerShell profile settings and make sure scripts are allowed for your user.
    exit /b 1
)

echo [INFO] Verifying aura launch from an arbitrary directory...
pushd "%VERIFY_DIR%" >nul
if exist "%APPDATA%\npm\aura.cmd" (
    call "%APPDATA%\npm\aura.cmd" help >nul 2>nul
    if errorlevel 1 (
        popd >nul
        echo [ERROR] The installed aura.cmd shim did not launch correctly.
        exit /b 1
    )
)
popd >nul

echo.
echo [OK] Aura was installed successfully.
echo [OK] Executable file: "%AURA_EXE%"
echo [OK] Global launch was verified from "%VERIFY_DIR%".
echo.
echo If the aura command is still unavailable in older terminals, reopen the IDE window or start a new PowerShell.
echo Direct launch without restarting is already available like this:
echo   "%AURA_EXE%" help
echo.
"%AURA_EXE%" help

exit /b 0
