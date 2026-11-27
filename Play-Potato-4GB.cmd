@echo off
setlocal EnableExtensions

REM potato-recomp: 4 GB VRAM + No-RT performance launcher (Series S profile)
REM Configure VICTORY_GAME_DIR (folder with your console build) and optionally VICTORY_GAME_EXE.

if not defined VICTORY_CACHE_DIR set "VICTORY_CACHE_DIR=%LOCALAPPDATA%\VIctory\cache"
if not defined VICTORY_DATA_DIR set "VICTORY_DATA_DIR=%LOCALAPPDATA%\VIctory\data"
if not defined VICTORY_LOG_FILE set "VICTORY_LOG_FILE=%LOCALAPPDATA%\VIctory\potato_series_s.log"
if not defined VICTORY_LOG set "VICTORY_LOG=warn"

if not exist "%VICTORY_CACHE_DIR%" mkdir "%VICTORY_CACHE_DIR%"
if not exist "%VICTORY_DATA_DIR%" mkdir "%VICTORY_DATA_DIR%"

if not defined VICTORY_GAME_DIR (
    if exist "%~dp0..\AAA.Game.Dump" (
        set "VICTORY_GAME_DIR=%~dp0..\AAA.Game.Dump"
    ) else (
        echo Set VICTORY_GAME_DIR to the folder containing your console build. 1>&2
        exit /b 1
    )
)

if not exist "%VICTORY_GAME_DIR%" (
    echo VICTORY_GAME_DIR does not exist: "%VICTORY_GAME_DIR%" 1>&2
    exit /b 1
)

if not defined VICTORY_GAME_EXE (
    for %%F in ("%VICTORY_GAME_DIR%\*.exe") do if not defined VICTORY_GAME_EXE set "VICTORY_GAME_EXE=%%~nxF"
)

if not defined VICTORY_GAME_EXE (
    echo No .exe found in "%VICTORY_GAME_DIR%". Set VICTORY_GAME_EXE explicitly. 1>&2
    exit /b 1
)

if not exist "%VICTORY_GAME_DIR%\%VICTORY_GAME_EXE%" (
    echo Executable not found: "%VICTORY_GAME_DIR%\%VICTORY_GAME_EXE%" 1>&2
    exit /b 1
)

if not defined VICTORY_RUNNER (
    if exist "%~dp0target\release\victory.exe" (
        set "VICTORY_RUNNER=%~dp0target\release\victory.exe"
    ) else if exist "%~dp0victory.exe" (
        set "VICTORY_RUNNER=%~dp0victory.exe"
    ) else (
        echo Missing victory.exe. Build first: cargo build --release --locked -p victory 1>&2
        exit /b 1
    )
)

if not defined VICTORY_DEFAULT_ADAPTER set "VICTORY_DEFAULT_ADAPTER="

REM --- No Ray Tracing (indoor ambient preserved via ambient.rs floor + clear_air cap + clock settle) ---
set "VICTORY_BVH_BUILDER="
set "VICTORY_RT_RESOLVE=0"
set "VICTORY_ACTIVATE_RAYS=0"
set "VICTORY_AMBIENT=0.8"
set "VICTORY_AMBIENT_MIN=0.025"
set "VICTORY_AMBIENT_AIR=1.5"
set "VICTORY_AMBIENT_SETTLE=250,3"
set "VICTORY_SHADER_FAST_BITS=24"
set "VICTORY_WAVE_WIDE="

REM --- 4 GB VRAM, Host Residency & Texture Streaming Fixes ---
set "VICTORY_RESIDENCY=1"
set "VICTORY_TEXTURE_POOL=0"
set "VICTORY_LEVELS_ON_DEMAND=1"
set "VICTORY_LEVELS_MIN_KB=512"
set "VICTORY_CARD_ROOM_FOR_TARGETS_MB=1280"
set "VICTORY_CARD_ROOM_FOR_BUFFERS=0"
set "VICTORY_CARD_BUFFERS_MB=96"
set "VICTORY_CARD_BUFFERS_MIN_MB=2"
set "VICTORY_TEXTURE_ROOM_PERCENT=90"
set "VICTORY_DYNAMIC_SLOTS=327680"

REM --- 720p Output (title upscaler avoids Series S FSR4 1080p table mismatch), Renderer Ports 1/2/5/6/7, Fast Copy & Async Submission ---
set "VICTORY_OUTPUT=1280x720"
set "VICTORY_UPSCALER=6"
set "VICTORY_FRAME_INTERVAL_MS=16.667"
set "VICTORY_FPS_LOG=1"
set "VICTORY_PORT_CONSTANTS=1"
set "VICTORY_PORT_CONSTANTS_ORDER=1"
set "VICTORY_PORT_DESCRIPTORS=1"
set "VICTORY_PORT_COPIES=1"
set "VICTORY_PORT_CBV=1"
set "VICTORY_PORT_GUARD=1"
set "VICTORY_PORT_JOURNAL=1"
set "VICTORY_COPY_PLAIN=1"
set "VICTORY_THIN=1"
set "VICTORY_ASYNC_SUBMIT=1"
set "VICTORY_JOIN_PARTS=8"
set "VICTORY_CARD_STEPS=2"
set "VICTORY_CARD_REHASH_RUNS=1"
set "VICTORY_JOIN_COPIES=1"
set "VICTORY_RELAY=2"
set "VICTORY_RELAY_COPY=1"
set "VICTORY_WIDE_BACK=1"
set "VICTORY_UPLOAD_AHEAD=1"
set "VICTORY_TEXTURE_NO_WAIT=1"
set "VICTORY_INDIRECT_WINDOW=1"
set "VICTORY_INDIRECT_BATCH=2"
set "VICTORY_SHOW_AFTER=1"
set "VICTORY_MAP_THREADS=1"
set "VICTORY_TITLE_PRIORITIES=0"
set "VICTORY_CARD_AWAKE=1"
set "VICTORY_INDIRECT_REST=0"
if not defined VICTORY_KBM_PC set "VICTORY_KBM_PC=1"

echo Starting AAA game ^(potato-recomp 4 GB No-RT profile^)...
echo Game:  %VICTORY_GAME_DIR%\%VICTORY_GAME_EXE%
echo Cache: %VICTORY_CACHE_DIR%
echo Data:  %VICTORY_DATA_DIR%
echo Log:   %VICTORY_LOG_FILE%

pushd "%~dp0" >nul
"%VICTORY_RUNNER%" run "%VICTORY_GAME_DIR%\%VICTORY_GAME_EXE%" --execute --console series-s %* > "%VICTORY_LOG_FILE%" 2>&1
set "EXIT_CODE=%ERRORLEVEL%"
popd >nul
if not "%EXIT_CODE%"=="0" (
    echo Exited with code %EXIT_CODE%. See "%VICTORY_LOG_FILE%" 1>&2
    exit /b %EXIT_CODE%
)
