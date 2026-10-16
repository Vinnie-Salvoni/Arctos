# =============================================================================
# Arctos OS — Git & GitHub Repository Setup Script
# =============================================================================
#
# This script guides you through installing Git (if missing), initializing a
# local Git repository, committing the clean Arctos codebase, and pushing it
# to a private GitHub repository for secure version control.
# =============================================================================

Write-Host "==================================================" -ForegroundColor Cyan
Write-Host "         Arctos OS Git Setup Assistant            " -ForegroundColor Cyan
Write-Host "==================================================" -ForegroundColor Cyan
Write-Host

# 1. Check if Git is installed
$gitCheck = Get-Command git -ErrorAction SilentlyContinue

if (-not $gitCheck) {
    Write-Host "[!] Git was not detected on your system." -ForegroundColor Yellow
    Write-Host "    We can install it automatically using Windows Package Manager (winget)." -ForegroundColor Yellow
    Write-Host
    
    $confirm = Read-Host "Would you like to install Git now? (Y/N)"
    if ($confirm -eq "Y" -or $confirm -eq "y") {
        Write-Host "[*] Installing Git via winget..." -ForegroundColor Cyan
        winget install --id Git.Git -e --source winget
        
        Write-Host
        Write-Host "[ok] Installation initiated. You may need to restart your terminal/IDE" -ForegroundColor Green
        Write-Host "     to refresh the System PATH and run this script again." -ForegroundColor Green
        Exit
    } else {
        Write-Host "[-] Git installation declined. Please install Git manually to continue." -ForegroundColor Red
        Exit
    }
}

Write-Host "[ok] Git is installed." -ForegroundColor Green
Write-Host

# 2. Check if already initialized
if (Test-Path .git) {
    Write-Host "[*] Git repository is already initialized here." -ForegroundColor Cyan
} else {
    Write-Host "[*] Initializing local Git repository..." -ForegroundColor Cyan
    git init
}

# 3. Stage and commit files
Write-Host "[*] Staging files..." -ForegroundColor Cyan
git add .

Write-Host "[*] Creating initial commit..." -ForegroundColor Cyan
git commit -m "Initial commit: Arctos OS bare-metal codebase with actual ATA/IDE disk integration"

Write-Host "[ok] Initial commit created successfully!" -ForegroundColor Green
Write-Host

# 4. Prompt for GitHub repository configuration
Write-Host "To push to a private GitHub repository:" -ForegroundColor Cyan
Write-Host "1. Go to github.com and log in."
Write-Host "2. Click 'New repository'."
Write-Host "3. Name it 'Arctos' (or any name you choose)."
Write-Host "4. Choose 'Private'."
Write-Host "5. Click 'Create repository' (Do NOT initialize with README or gitignore)."
Write-Host

$repoUrl = Read-Host "Enter your private GitHub repository URL (e.g. https://github.com/username/repo.git)"

if ($repoUrl) {
    # Remove existing remote if present
    git remote remove origin 2>$null
    
    Write-Host "[*] Configuring remote origin..." -ForegroundColor Cyan
    git remote add origin $repoUrl
    
    Write-Host "[*] Setting main branch name..." -ForegroundColor Cyan
    git branch -M main
    
    Write-Host
    Write-Host "==================================================" -ForegroundColor Green
    Write-Host "                 Ready to Push!                  " -ForegroundColor Green
    Write-Host "==================================================" -ForegroundColor Green
    Write-Host "To upload your project, run the following command:"
    Write-Host "   git push -u origin main" -ForegroundColor Yellow
    Write-Host
} else {
    Write-Host "[-] Skipped remote remote repository configuration." -ForegroundColor Yellow
    Write-Host "    You can add it later manually using: git remote add origin <URL>"
}
