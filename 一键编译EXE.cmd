@echo off
chcp 65001 >nul
setlocal
cd /d "%~dp0"
echo ============================================
echo   岛仔（内网版）一键编译 EXE
echo ============================================
echo.

where cargo >nul 2>nul
if errorlevel 1 (
  echo [错误] 没找到 Rust 工具链。
  echo 请先安装：https://rustup.rs
  echo 安装时选择 "Desktop development with C++" 之外的默认选项即可，
  echo 另外需要 Visual Studio Build Tools（C++ 生成工具）。
  echo.
  pause
  exit /b 1
)

echo [1/2] 编译中（首次编译约 5-15 分钟）...
cargo +stable build --locked --release --bin daozai --target x86_64-pc-windows-msvc
if errorlevel 1 (
  echo.
  echo [错误] 编译失败，请把上面的报错发出来。
  pause
  exit /b 1
)

echo.
echo [2/2] 复制到当前目录...
if not exist dist mkdir dist
copy /y "target\x86_64-pc-windows-msvc\release\daozai.exe" "dist\Daozai-Intranet.exe" >nul
echo.
echo 完成！EXE 位置：
echo   %cd%\dist\Daozai-Intranet.exe
echo.
pause
