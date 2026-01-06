# FingerPain Installer for Windows
# Run as Administrator

Write-Host "FingerPain Installer for Windows" -ForegroundColor Cyan
Write-Host "=================================" -ForegroundColor Cyan

# Check for admin privileges
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole] "Administrator")
if (-not $isAdmin) {
    Write-Host "Please run this script as Administrator." -ForegroundColor Red
    exit 1
}

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$projectRoot = Join-Path $scriptDir "..\..\"
$targetDir = Join-Path $projectRoot "target\release"
$installDir = "$env:ProgramFiles\FingerPain"

# Check if binaries exist, if not build
if (-not (Test-Path (Join-Path $targetDir "fingerpain-daemon.exe"))) {
    Write-Host "Building FingerPain..." -ForegroundColor Yellow
    Push-Location $projectRoot
    cargo build --release
    Pop-Location
}

# Create installation directory
Write-Host "Creating installation directory..." -ForegroundColor Yellow
New-Item -ItemType Directory -Path $installDir -Force | Out-Null

# Copy binaries
Write-Host "Installing binaries to $installDir..." -ForegroundColor Yellow
Copy-Item (Join-Path $targetDir "fingerpain-daemon.exe") $installDir -Force
Copy-Item (Join-Path $targetDir "fingerpain.exe") $installDir -Force

# Add to PATH
$currentPath = [Environment]::GetEnvironmentVariable("Path", "Machine")
if ($currentPath -notlike "*$installDir*") {
    Write-Host "Adding to PATH..." -ForegroundColor Yellow
    [Environment]::SetEnvironmentVariable("Path", "$currentPath;$installDir", "Machine")
}

# Create startup entry (Registry)
Write-Host "Adding to startup..." -ForegroundColor Yellow
$regPath = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
Set-ItemProperty -Path $regPath -Name "FingerPain" -Value "$installDir\fingerpain-daemon.exe"

# Start the daemon
Write-Host "Starting daemon..." -ForegroundColor Yellow
Start-Process -FilePath "$installDir\fingerpain-daemon.exe" -WindowStyle Hidden

Write-Host ""
Write-Host "Installation complete!" -ForegroundColor Green
Write-Host ""
Write-Host "The daemon is now running and will start automatically on login."
Write-Host ""
Write-Host "Commands:" -ForegroundColor Cyan
Write-Host "  fingerpain status  - Check daemon status"
Write-Host "  fingerpain today   - View today's stats"
Write-Host "  fingerpain week    - View this week's stats"
Write-Host ""
