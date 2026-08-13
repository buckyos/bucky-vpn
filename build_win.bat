@echo off
setlocal
cd /d "%~dp0"

set "APP_VERSION="
for /f "usebackq delims=" %%V in (`powershell -NoProfile -Command "$json = cargo metadata --no-deps --format-version 1; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }; $metadata = ConvertFrom-Json -InputObject ($json -join [Environment]::NewLine); $packages = @(@($metadata.packages).Where({ $_.name -eq 'bucky-vpn' })); if ($packages.Count -ne 1) { Write-Error 'expected exactly one bucky-vpn package in cargo metadata'; exit 1 }; $version = $packages[0].version; if ($version -notmatch '^\d+\.\d+\.\d+$') { Write-Error ('unsupported installer version: ' + $version); exit 1 }; Write-Output $version"`) do set "APP_VERSION=%%V"
if not defined APP_VERSION (
    echo Failed to read the bucky-vpn package version from vpn-client/Cargo.toml. 1>&2
    exit /b 1
)

cargo build -p bucky-vpn --release
if errorlevel 1 exit /b %errorlevel%

where ISCC.exe >nul 2>&1
if errorlevel 1 (
    echo ISCC.exe was not found on PATH. 1>&2
    exit /b 1
)

ISCC.exe "/DAppVersion=%APP_VERSION%" "install.iss"
if errorlevel 1 exit /b %errorlevel%

if not exist "dist\BuckyVPN_%APP_VERSION%_amd64_Setup.exe" (
    echo Expected Windows installer was not produced. 1>&2
    exit /b 1
)
