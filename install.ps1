$ErrorActionPreference = "Stop"

$Repo = "MorseWayne/aisub2api"
$Target = "x86_64-pc-windows-msvc"
$InstallDir = if ($env:AISUB2API_INSTALL_DIR) {
    $env:AISUB2API_INSTALL_DIR
} else {
    Join-Path $env:LOCALAPPDATA "Programs\aisub2api"
}
$ServiceWasRunning = $false

function Test-Architecture {
    $arch = $env:PROCESSOR_ARCHITEW6432
    if ([string]::IsNullOrWhiteSpace($arch)) {
        $arch = $env:PROCESSOR_ARCHITECTURE
    }

    if ($arch -ne "AMD64") {
        throw "Unsupported Windows architecture: $arch. Download manually from GitHub Releases."
    }
}

function Add-ToUserPath {
    param(
        [Parameter(Mandatory = $true)]
        [string]$PathToAdd
    )

    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    $pathParts = @()
    if (-not [string]::IsNullOrWhiteSpace($userPath)) {
        $pathParts = $userPath -split ';' | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
    }

    $alreadyInPath = $pathParts | Where-Object { $_.TrimEnd('\') -ieq $PathToAdd.TrimEnd('\') }
    if (-not $alreadyInPath) {
        $newPath = if ([string]::IsNullOrWhiteSpace($userPath)) {
            $PathToAdd
        } else {
            "$userPath;$PathToAdd"
        }

        [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
        Write-Host "Added $PathToAdd to your user PATH."
        Write-Host "Restart your terminal to use aisub2api globally."
        Write-Host ""
    }
}

function Get-AiSub2ApiProcesses {
    @(Get-Process -Name "aisub2api", "claude-proxy" -ErrorAction SilentlyContinue)
}

function Confirm-YesNo {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Prompt
    )

    $answer = Read-Host "$Prompt [y/N]"
    return $answer -in @("y", "Y", "yes", "YES", "Yes")
}

function Prepare-ExistingService {
    if ((Get-AiSub2ApiProcesses).Count -gt 0) {
        $script:ServiceWasRunning = $true
        Write-Host "A running aisub2api or legacy process was detected."
        Write-Host "Continuing installation will stop it before replacing the binary."
    }

    if (-not (Confirm-YesNo -Prompt "Continue installing aisub2api?")) {
        Write-Host "Installation cancelled."
        exit 1
    }
}

function Stop-ExistingService {
    if (-not $script:ServiceWasRunning) {
        return
    }

    $processes = Get-AiSub2ApiProcesses
    if ($processes.Count -eq 0) {
        Write-Host "aisub2api is no longer running."
        return
    }

    Write-Host "Stopping existing aisub2api process..."
    $processes | Stop-Process -Force -ErrorAction Stop

    foreach ($process in $processes) {
        Wait-Process -Id $process.Id -Timeout 10 -ErrorAction SilentlyContinue
        if (Get-Process -Id $process.Id -ErrorAction SilentlyContinue) {
            throw "Failed to stop aisub2api process $($process.Id) within 10 seconds."
        }
    }
}

function Remove-LegacyInstall {
    param(
        [Parameter(Mandatory = $true)]
        [string]$BinaryPath
    )

    $legacyBinary = Join-Path $InstallDir "claude-proxy.exe"
    if (Test-Path $legacyBinary) {
        Remove-Item -Force $legacyBinary
        Write-Host "Removed legacy binary $legacyBinary"
    }

    # Trigger state migration during installation. Conflicting legacy state is
    # archived so credentials remain recoverable but cannot affect startup.
    & $BinaryPath config path | Out-Null
}

function Start-InstalledService {
    param(
        [Parameter(Mandatory = $true)]
        [string]$BinaryPath
    )

    if (-not (Confirm-YesNo -Prompt "Start aisub2api now?")) {
        return
    }

    if (Confirm-YesNo -Prompt "Run aisub2api in the background?") {
        Write-Host "Starting aisub2api in the background..."
        Start-Process -FilePath $BinaryPath -ArgumentList @("server", "start") -WindowStyle Hidden | Out-Null
    } else {
        Write-Host "Starting aisub2api in the foreground..."
        & $BinaryPath server start
    }
}

function Install-AiSub2Api {
    Test-Architecture

    $archive = "aisub2api-$Target.zip"
    $url = "https://github.com/$Repo/releases/latest/download/$archive"
    $tempDir = Join-Path ([System.IO.Path]::GetTempPath()) ([System.IO.Path]::GetRandomFileName())

    Prepare-ExistingService

    Write-Host "Downloading aisub2api for Windows..."
    Write-Host "  URL: $url"

    New-Item -ItemType Directory -Force -Path $tempDir | Out-Null

    try {
        $archivePath = Join-Path $tempDir $archive
        Invoke-WebRequest -UseBasicParsing -Uri $url -OutFile $archivePath
        Expand-Archive -Path $archivePath -DestinationPath $tempDir -Force

        Stop-ExistingService

        New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
        $binaryPath = Join-Path $InstallDir "aisub2api.exe"
        Move-Item -Force (Join-Path $tempDir "aisub2api.exe") $binaryPath
        Remove-LegacyInstall -BinaryPath $binaryPath

        Write-Host ""
        Write-Host "Installed to $binaryPath"
        Write-Host ""

        Add-ToUserPath -PathToAdd $InstallDir

        & $binaryPath --version

        Start-InstalledService -BinaryPath $binaryPath
    } finally {
        if (Test-Path $tempDir) {
            Remove-Item -Recurse -Force $tempDir
        }
    }
}

Install-AiSub2Api
