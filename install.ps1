$ErrorActionPreference = "Stop"

Write-Host "Installing Nyx Programming Language..."

# Detect Architecture
$Architecture = $env:PROCESSOR_ARCHITECTURE
$URL = ""
$FileName = ""

if ($Architecture -eq "AMD64") {
    $URL = "https://github.com/nyx-lang/nyx/releases/latest/download/nyx-windows-x64.exe.zip"
    $FileName = "nyx-windows-x64.exe"
} elseif ($Architecture -eq "x86") {
    $URL = "https://github.com/nyx-lang/nyx/releases/latest/download/nyx-windows-x86.exe.zip"
    $FileName = "nyx-windows-x86.exe"
} else {
    Write-Error "Unsupported architecture: $Architecture. Nyx currently supports x64 and x86 Windows."
    exit 1
}

$InstallDir = "$env:USERPROFILE\.nyx\bin"
if (!(Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
}

$ZipFile = "$InstallDir\nyx.zip"
Write-Host "Downloading from $URL..."
Invoke-WebRequest -Uri $URL -OutFile $ZipFile

Write-Host "Extracting..."
Expand-Archive -Path $ZipFile -DestinationPath $InstallDir -Force
Remove-Item $ZipFile

# Rename the extracted binary to nyx.exe
Rename-Item -Path "$InstallDir\$FileName" -NewName "nyx.exe" -Force

# Add to PATH if not present
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($UserPath -notmatch [regex]::Escape($InstallDir)) {
    Write-Host "Adding $InstallDir to your PATH..."
    [Environment]::SetEnvironmentVariable("Path", "$UserPath;$InstallDir", "User")
    $env:PATH = "$env:PATH;$InstallDir"
}

Write-Host ""
Write-Host "Nyx installed successfully to $InstallDir!" -ForegroundColor Green
Write-Host "Test it by running: nyx --version"
Write-Host "Note: You may need to restart your terminal for PATH changes to take full effect."
