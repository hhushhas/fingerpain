# FingerPain Uninstaller for Windows
# Run as Administrator

Write-Host "FingerPain Uninstaller for Windows" -ForegroundColor Cyan
Write-Host "===================================" -ForegroundColor Cyan

# Check for admin privileges
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole] "Administrator")
if (-not $isAdmin) {
    Write-Host "Please run this script as Administrator." -ForegroundColor Red
    exit 1
}

$installDir = "$env:ProgramFiles\FingerPain"
$dataDir = "$env:APPDATA\com.fingerpain.fingerpain"

# Stop the daemon
Write-Host "Stopping daemon..." -ForegroundColor Yellow
Stop-Process -Name "fingerpain-daemon" -Force -ErrorAction SilentlyContinue

# Remove from startup
Write-Host "Removing from startup..." -ForegroundColor Yellow
$regPath = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
Remove-ItemProperty -Path $regPath -Name "FingerPain" -ErrorAction SilentlyContinue

# Remove from PATH
Write-Host "Removing from PATH..." -ForegroundColor Yellow
$currentPath = [Environment]::GetEnvironmentVariable("Path", "Machine")
$newPath = ($currentPath.Split(';') | Where-Object { $_ -ne $installDir }) -join ';'
[Environment]::SetEnvironmentVariable("Path", $newPath, "Machine")

# Remove binaries
Write-Host "Removing binaries..." -ForegroundColor Yellow
Remove-Item -Path $installDir -Recurse -Force -ErrorAction SilentlyContinue

# Ask about data
$response = Read-Host "Do you want to remove all FingerPain data? (y/N)"
if ($response -eq 'y' -or $response -eq 'Y') {
    Write-Host "Removing data..." -ForegroundColor Yellow
    Remove-Item -Path $dataDir -Recurse -Force -ErrorAction SilentlyContinue
    Write-Host "Data removed."
} else {
    Write-Host "Data preserved at: $dataDir"
}

Write-Host ""
Write-Host "Uninstallation complete!" -ForegroundColor Green
