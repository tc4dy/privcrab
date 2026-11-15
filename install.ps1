$ErrorActionPreference = "Stop"

$repo = "https://github.com/tc4dy/privcrab"
$work = Join-Path $env:TEMP "privcrab-install"
$dest = Join-Path $env:LOCALAPPDATA "Programs\privcrab"

function Clear-Work {
    if (Test-Path -LiteralPath $work) {
        Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
    }
}

function Stop-Install([string]$message) {
    Clear-Work
    Write-Error "[!] $message" -ErrorAction Continue
    exit 1
}

Write-Host "[*] privcrab installer"

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "[!] missing: cargo"
    Write-Host "[>] install the Rust toolchain: winget install Rustlang.Rustup"
    Write-Host "[>] open a new terminal and run this script again"
    exit 1
}

if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
    Write-Host "[!] missing: git"
    Write-Host "[>] install git: winget install Git.Git"
    Write-Host "[>] open a new terminal and run this script again"
    exit 1
}

try {
    Clear-Work

    Write-Host "[*] cloning $repo"
    & git clone --depth 1 $repo $work
    if ($LASTEXITCODE -ne 0) {
        throw "git clone exited with code $LASTEXITCODE"
    }
} catch {
    Stop-Install "clone failed: $($_.Exception.Message)"
}

try {
    Write-Host "[*] building"
    Push-Location $work
    try {
        & cargo build --release
        if ($LASTEXITCODE -ne 0) {
            throw "cargo exited with code $LASTEXITCODE"
        }
    } finally {
        Pop-Location
    }
} catch {
    Stop-Install "build failed: $($_.Exception.Message)"
}

$built = Join-Path $work "target\release\privcrab.exe"
if (-not (Test-Path -LiteralPath $built)) {
    Stop-Install "build finished but privcrab.exe was not produced"
}

try {
    New-Item -ItemType Directory -Path $dest -Force | Out-Null
    Copy-Item -LiteralPath $built -Destination (Join-Path $dest "privcrab.exe") -Force
} catch {
    Stop-Install "could not copy binary to ${dest}: $($_.Exception.Message)"
}

Clear-Work

$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
$entries = @()
if ($userPath) {
    $entries = $userPath -split ";"
}

if ($entries -notcontains $dest) {
    Write-Host "[!] $dest is not on PATH"
    $answer = "n"
    try {
        $answer = Read-Host "[>] add it to the user PATH? (y/N)"
    } catch {
        Write-Host "[!] no interactive prompt available"
    }
    if ($answer -match "^[yY]") {
        try {
            $joined = (($entries + $dest) | Where-Object { $_ }) -join ";"
            [Environment]::SetEnvironmentVariable("Path", $joined, "User")
            Write-Host "[+] added to user PATH - open a new terminal to use it"
        } catch {
            Write-Error "[!] could not update PATH: $($_.Exception.Message)" -ErrorAction Continue
            Write-Host "[>] add this directory to PATH manually: $dest"
        }
    } else {
        Write-Host "[>] add this directory to PATH manually: $dest"
    }
}

Write-Host "[!] note: MAC spoofing, probe blocking, and DNS modules require Linux. Only leak testing is functional on Windows."
Write-Host "[+] privcrab installed"