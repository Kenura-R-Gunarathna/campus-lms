# Campus LMS Windows CLI Installer
$repo = "Kenura-R-Gunarathna/campus-lms"
$installDir = Join-Path $env:LOCALAPPDATA "campus-lms"
$binDir = Join-Path $installDir "bin"
$exePath = Join-Path $binDir "campus-lms.exe"

Write-Host "--- Campus LMS Installer ---" -ForegroundColor Cyan

# 1. Create directories
if (-not (Test-Path $binDir)) {
    New-Item -ItemType Directory -Force -Path $binDir | Out-Null
}

# 2. Get latest release metadata from GitHub
Write-Host "Checking for latest release..."
$url = "https://api.github.com/repos/$repo/releases/latest"
try {
    $release = Invoke-RestMethod -Uri $url
} catch {
    Write-Error "Failed to fetch latest release. Check your internet connection."
    exit
}

$version = $release.tag_name
$asset = $release.assets | Where-Object { $_.name -like "*windows*.exe" } | Select-Object -First 1

if (-not $asset) {
    Write-Error "Could not find a Windows binary for version $version."
    exit
}

Write-Host "Found version $version. Downloading..."
Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $exePath

# 3. Add to PATH if not present
$currentPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($currentPath -notlike "*$binDir*") {
    Write-Host "Adding $binDir to User PATH..."
    [Environment]::SetEnvironmentVariable("Path", $currentPath + ";" + $binDir, "User")
    $env:Path += ";" + $binDir
    Write-Host "Successfully added to PATH. You may need to restart your terminal." -ForegroundColor Yellow
}

Write-Host "`nInstallation Complete!" -ForegroundColor Green
Write-Host "You can now run 'campus-lms' from any terminal."
