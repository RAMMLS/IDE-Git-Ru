@echo off
setlocal EnableExtensions EnableDelayedExpansion

set "SCRIPT_DIR=%~dp0"
if "%SCRIPT_DIR:~-1%"=="\" set "SCRIPT_DIR=%SCRIPT_DIR:~0,-1%"
set "VCS_CORE_DIR=%SCRIPT_DIR%\vcs-core"
set "CARGO_BIN=%USERPROFILE%\.cargo\bin"
set "AURA_EXE=%CARGO_BIN%\aura.exe"

echo.
echo ==========================================
echo   Aura installer
echo ==========================================
echo.

if not exist "%VCS_CORE_DIR%\Cargo.toml" (
    echo [ERROR] Не найден "%VCS_CORE_DIR%\Cargo.toml".
    echo Убедись, что install-aura.bat лежит в корне проекта IDE-Git-Ru.
    exit /b 1
)

where cargo >nul 2>nul
if errorlevel 1 (
    echo [INFO] cargo не найден. Пытаюсь установить Rust toolchain через winget...
    where winget >nul 2>nul
    if errorlevel 1 (
        echo [ERROR] winget не найден. Установи Rust вручную с https://rustup.rs/
        exit /b 1
    )

    winget install --id Rustlang.Rustup -e --accept-source-agreements --accept-package-agreements
    if errorlevel 1 (
        echo [ERROR] Не удалось установить rustup через winget.
        exit /b 1
    )
)

if not exist "%CARGO_BIN%" (
    mkdir "%CARGO_BIN%" >nul 2>nul
)

echo [INFO] Проверяю пользовательский PATH...
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
    echo [INFO] Добавлен "%CARGO_BIN%" в пользовательский PATH.
) else (
    echo [INFO] "%CARGO_BIN%" уже есть в пользовательском PATH.
)

set "PATH=%CARGO_BIN%;%PATH%"

where cargo >nul 2>nul
if errorlevel 1 (
    echo [ERROR] cargo все еще недоступен в текущей сессии.
    echo Закрой терминал, открой заново и повторно запусти install-aura.bat.
    exit /b 1
)

echo [INFO] Устанавливаю локальный бинарник aura...
cargo install --path "%VCS_CORE_DIR%" --bin aura --force
if errorlevel 1 (
    echo [ERROR] Не удалось установить aura.
    exit /b 1
)

if not exist "%AURA_EXE%" (
    echo [ERROR] Установка завершилась, но "%AURA_EXE%" не найден.
    exit /b 1
)

echo.
echo [OK] Aura успешно установлен.
echo [OK] Исполняемый файл: "%AURA_EXE%"
echo.
echo Если команда aura еще не находится в старых терминалах, закрой их и открой заново.
echo Прямой запуск без перезапуска уже доступен так:
echo   "%AURA_EXE%" help
echo.
"%AURA_EXE%" help

exit /b 0
