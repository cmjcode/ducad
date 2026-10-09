<#
.SYNOPSIS
    Build and packaging script for DUCAD Editor on Windows OS.

.DESCRIPTION
    Automates building, version syncing, packaging, and cleaning of the DUCAD CAD editor
    on Windows (x86_64 MSVC). Supports preserving precompiled OpenCASCADE (OCCT)
    artifacts to prevent long recompilations.

.PARAMETER Target
    Build target:
      - 'app'       : (Default) Build DUCAD GUI desktop application (ducad.exe)
      - 'bundle'    : Build and package binary + assets into dist/windows as a ZIP
      - 'cli'       : Build DUCAD command-line interface (ducad-cli.exe)
      - 'all'       : Build both DUCAD GUI app and DUCAD CLI
      - 'check'     : Run cargo check across the workspace
      - 'test'      : Run cargo test across workspace crates
      - 'run'       : Build (if needed) and run ducad.exe immediately
      - 'clean'     : Clean build artifacts while preserving target/OCCT
      - 'clean-all' : Deep clean entire target/ directory (including OCCT)

.PARAMETER Profile
    Cargo build profile: 'release' (default) or 'debug'.

.PARAMETER Clean
    Clean build artifacts before building (preserves target/OCCT).

.PARAMETER CleanAll
    Deep clean everything (including target/OCCT) before building.

.PARAMETER CleanNoOcct
    Explicit clean of target/ artifacts while strictly preserving target/OCCT.

.PARAMETER Zip
    Compress build output and assets into a ZIP file in dist/windows.

.PARAMETER Run
    Run the application after building.

.PARAMETER Deps
    Verify and install required Rust targets and build tools.

.PARAMETER SyncVersion
    Sync version across Cargo.toml, ducad-app metadata, etc., using the VERSION file.

.PARAMETER Features
    Optional extra features to pass to cargo build (e.g. "local-gguf").

.EXAMPLE
    .\build_windows.ps1
    # Builds DUCAD app in release mode.

.EXAMPLE
    .\build_windows.ps1 bundle
    # Builds and packages ducad.exe and assets into dist/windows/ducad-<version>-windows-x86_64.zip.

.EXAMPLE
    .\build_windows.ps1 -Profile debug -Run
    # Builds debug profile and immediately runs ducad.exe.

.EXAMPLE
    .\build_windows.ps1 clean
    # Safely cleans cargo target cache while preserving OpenCASCADE C++ compilation.
#>

[CmdletBinding()]
param(
    [Parameter(Position = 0)]
    [ValidateSet("app", "bundle", "cli", "all", "check", "test", "run", "clean", "clean-all")]
    [string]$Target = "app",

    [ValidateSet("release", "debug")]
    [string]$Profile = "release",

    [switch]$Clean,
    [switch]$CleanAll,
    [switch]$CleanNoOcct,
    [switch]$Zip,
    [switch]$Run,
    [switch]$Deps,
    [switch]$SyncVersion,
    [string]$Features = "",
    [Alias("h")]
    [switch]$Help
)

$ErrorActionPreference = "Stop"

# --- Output Helper Functions ---
function Write-Status([string]$Message) {
    Write-Host "[INFO] " -ForegroundColor Cyan -NoNewline
    Write-Host $Message
}

function Write-Success([string]$Message) {
    Write-Host "[SUCCESS] " -ForegroundColor Green -NoNewline
    Write-Host $Message
}

function Write-Warning([string]$Message) {
    Write-Host "[WARNING] " -ForegroundColor Yellow -NoNewline
    Write-Host $Message
}

function Write-ErrorMsg([string]$Message) {
    Write-Host "[ERROR] " -ForegroundColor Red -NoNewline
    Write-Host $Message
}

function Write-Guard([string]$Message) {
    Write-Host "[SAFEGUARD] " -ForegroundColor Magenta -NoNewline
    Write-Host $Message
}

# --- Help Display ---
if ($Help) {
    Get-Help $MyInvocation.MyCommand.Path -Full
    exit 0
}

# --- Resolve Project Paths ---
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
if (Test-Path (Join-Path $ScriptDir "Cargo.toml")) {
    $EditorDir = $ScriptDir
    $RootDir = Split-Path -Parent $ScriptDir
} elseif (Test-Path (Join-Path $ScriptDir "ducad-editor\Cargo.toml")) {
    $EditorDir = Join-Path $ScriptDir "ducad-editor"
    $RootDir = $ScriptDir
} else {
    $EditorDir = (Get-Location).Path
    $RootDir = $EditorDir
}

Set-Location -Path $EditorDir

$DistDir = Join-Path $EditorDir "dist"
$WindowsDistDir = Join-Path $DistDir "windows"
$TargetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $EditorDir "target" }

# --- Load .env If Present ---
$EnvPaths = @(
    Join-Path $RootDir ".env",
    Join-Path $EditorDir ".env"
)
foreach ($envFile in $EnvPaths) {
    if (Test-Path $envFile) {
        Write-Status "Memuat konfigurasi environment dari $envFile"
        Get-Content $envFile | ForEach-Object {
            $line = $_.Trim()
            if ($line -and -not $line.StartsWith("#") -and ($line -match "^([^=]+)=(.*)$")) {
                $name = $matches[1].Trim()
                $val = $matches[2].Trim().Trim('"').Trim("'")
                [System.Environment]::SetEnvironmentVariable($name, $val, [System.EnvironmentVariableTarget]::Process)
            }
        }
        break
    }
}

# --- Read Application Version ---
$Version = "0.3.0"
$VersionPaths = @(
    Join-Path $RootDir "VERSION",
    Join-Path $EditorDir "VERSION"
)
foreach ($vFile in $VersionPaths) {
    if (Test-Path $vFile) {
        $raw = (Get-Content $vFile -Raw).Trim()
        if ($raw) {
            $Version = $raw
            break
        }
    }
}
$env:VERSION = $Version

Write-Host ""
Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "  DUCAD Editor Windows Build System (v$Version)" -ForegroundColor Cyan
Write-Host "==========================================================" -ForegroundColor Cyan
Write-Status "Direktori Kerja : $EditorDir"
Write-Status "Target          : $Target | Profil: $Profile"

# --- Setup Visual Studio / MSVC Build Environment ---
function Initialize-MsvcEnvironment {
    # Pastikan default CMAKE_GENERATOR kompatibel jika belum diset
    if (-not $env:CMAKE_GENERATOR) {
        $env:CMAKE_GENERATOR = "Visual Studio 17 2022"
    }

    # Sinkronkan cache CMake di target/OCCT/build jika ada perbedaan generator sebelumnya
    $occtCache = Join-Path $TargetDir "OCCT\build\CMakeCache.txt"
    $occtCMakeFiles = Join-Path $TargetDir "OCCT\build\CMakeFiles"
    if (Test-Path $occtCache) {
        $cacheContent = Get-Content $occtCache -Raw -ErrorAction SilentlyContinue
        if ($cacheContent -and ($cacheContent -match "CMAKE_GENERATOR:INTERNAL=(.*)")) {
            $prevGen = $matches[1].Trim()
            if ($prevGen -and ($prevGen -ne $env:CMAKE_GENERATOR)) {
                Write-Status "Menyelaraskan generator CMake ($prevGen -> $($env:CMAKE_GENERATOR))..."
                Remove-Item -Force $occtCache -ErrorAction SilentlyContinue
                if (Test-Path $occtCMakeFiles) {
                    Remove-Item -Recurse -Force $occtCMakeFiles -ErrorAction SilentlyContinue
                }
            }
        }
    }

    if (Get-Command cl.exe -ErrorAction SilentlyContinue) {
        return
    }

    Write-Status "Mencari Visual Studio / MSVC toolchain..."
    $vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
    if (Test-Path $vswhere) {
        $vsInstall = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
        if ($vsInstall -and (Test-Path "$vsInstall\Common7\Tools\Launch-VsDevShell.ps1")) {
            Write-Status "Menginisialisasi lingkungan pengembang MSVC dari $vsInstall..."
            try {
                & "$vsInstall\Common7\Tools\Launch-VsDevShell.ps1" -Arch amd64 -HostArch amd64 | Out-Null
                Write-Success "MSVC compiler (cl.exe) siap digunakan."
                return
            } catch {
                Write-Warning "Gagal memanggil Launch-VsDevShell: $_"
            }
        }
    }

    Write-Warning "cl.exe tidak ditemukan di PATH. Cargo akan mencoba mencari cl.exe otomatis via cc-rs."
}

# --- Check Prerequisites ---
function Check-Prerequisites {
    Write-Status "Memeriksa dependensi build..."

    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        Write-ErrorMsg "Cargo/Rust tidak ditemukan. Harap install Rust dari https://rustup.rs"
        exit 1
    }

    if (-not (Get-Command cmake -ErrorAction SilentlyContinue)) {
        Write-Warning "CMake tidak ditemukan di PATH. Crate native seperti OpenCASCADE membutuhkan CMake."
    }

    Initialize-MsvcEnvironment
    Write-Success "Dependensi dasar siap."
}

# --- Install Build Dependencies and Targets ---
function Install-Dependencies {
    Write-Status "Memeriksa & menginstall target Rust x86_64-pc-windows-msvc..."
    rustup target add x86_64-pc-windows-msvc
    Write-Success "Target Rust siap."
}

# --- Synchronize Versions Across Project Files ---
function Sync-Versions {
    Write-Status "Menyelaraskan seluruh versi terkait dengan VERSION ($Version)..."

    # 1. Update VERSION file
    Set-Content -Path (Join-Path $EditorDir "VERSION") -Value $Version -NoNewline

    # 2. Update workspace Cargo.toml
    $cargoToml = Join-Path $EditorDir "Cargo.toml"
    if (Test-Path $cargoToml) {
        $content = Get-Content $cargoToml -Raw
        $pattern = '(?ms)(\[workspace\.package\][\s\S]*?version\s*=\s*")[^"]+(")'
        $newContent = [regex]::Replace($content, $pattern, "${1}$Version${2}", 1)
        if ($newContent -ne $content) {
            Set-Content -Path $cargoToml -Value $newContent -NoNewline
            Write-Status "  - Mengupdate Cargo.toml [workspace.package] version -> $Version"
        }
    }

    # 3. Update crates/ducad-app/Cargo.toml
    $appCargo = Join-Path $EditorDir "crates\ducad-app\Cargo.toml"
    if (Test-Path $appCargo) {
        $content = Get-Content $appCargo -Raw
        $pattern = '(?ms)(\[package\.metadata\.bundle\][\s\S]*?version\s*=\s*")[^"]+(")'
        $newContent = [regex]::Replace($content, $pattern, "${1}$Version${2}", 1)
        if ($newContent -ne $content) {
            Set-Content -Path $appCargo -Value $newContent -NoNewline
            Write-Status "  - Mengupdate crates/ducad-app/Cargo.toml [package.metadata.bundle] version -> $Version"
        }
    }

    Write-Success "Sinkronisasi versi selesai (v$Version)."
}

# --- Safe Clean Functions ---
function Clean-Build([bool]$deepClean) {
    if (-not (Test-Path $TargetDir)) {
        Write-Status "Direktori target tidak ditemukan ($TargetDir). Tidak ada yang perlu dibersihkan."
        return
    }

    if ($deepClean) {
        Write-Warning "PERINGATAN: Membersihkan SELURUH target/ termasuk OpenCASCADE C++ kernel!"
        Write-Warning "Kompilasi ulang OpenCASCADE dapat memakan waktu 10-30 menit."
        Remove-Item -Recurse -Force $TargetDir -ErrorAction SilentlyContinue
        if (Test-Path $DistDir) {
            Remove-Item -Recurse -Force $DistDir -ErrorAction SilentlyContinue
        }
        Write-Success "Pembersihan total selesai."
        return
    }

    Write-Guard "Menjalankan pembersihan aman (melindungi target/OCCT)..."
    
    # 1. Bersihkan dist/windows
    if (Test-Path $WindowsDistDir) {
        Remove-Item -Recurse -Force $WindowsDistDir -ErrorAction SilentlyContinue
    }

    # 2. Bersihkan target profil tertentu atau workspace
    $occtBackup = Join-Path ([System.IO.Path]::GetTempPath()) ("ducad_occt_backup_" + [System.IO.Path]::GetRandomFileName())
    $occtPath = Join-Path $TargetDir "OCCT"
    $hasOcct = Test-Path $occtPath

    if ($hasOcct) {
        Write-Status "Mengamankan target/OCCT ke $occtBackup..."
        Move-Item -Path $occtPath -Destination $occtBackup -Force
    }

    try {
        Write-Status "Membersihkan cache build Rust via cargo clean..."
        cargo clean --manifest-path (Join-Path $EditorDir "Cargo.toml")
    } finally {
        if ($hasOcct -and (Test-Path $occtBackup)) {
            if (-not (Test-Path $TargetDir)) {
                New-Item -ItemType Directory -Path $TargetDir -Force | Out-Null
            }
            Write-Status "Mengembalikan target/OCCT..."
            Move-Item -Path $occtBackup -Destination $occtPath -Force
            Write-Success "target/OCCT berhasil dipertahankan!"
        }
    }

    Write-Success "Pembersihan aman selesai."
}

# --- Format File Size ---
function Format-FileSize([long]$bytes) {
    if ($bytes -ge 1GB) { return "{0:N2} GB" -f ($bytes / 1GB) }
    if ($bytes -ge 1MB) { return "{0:N2} MB" -f ($bytes / 1MB) }
    if ($bytes -ge 1KB) { return "{0:N2} KB" -f ($bytes / 1KB) }
    return "$bytes B"
}

# --- Build Execution ---
function Invoke-DucadBuild([string]$packageName, [string]$binaryName) {
    $cargoArgs = @("build")
    if ($Profile -eq "release") {
        $cargoArgs += "--release"
    }
    $cargoArgs += @("-p", $packageName)

    if ($Features) {
        $cargoArgs += @("--features", $Features)
    }

    Write-Status "Menjalankan: cargo $($cargoArgs -join ' ')"
    $process = Start-Process -FilePath "cargo" -ArgumentList $cargoArgs -NoNewWindow -PassThru -Wait
    if ($process.ExitCode -ne 0) {
        Write-ErrorMsg "Build $packageName gagal dengan kode keluar $($process.ExitCode)!"
        exit $process.ExitCode
    }

    # Lokasi binary hasil build
    $binSubDir = if ($Profile -eq "release") { "release" } else { "debug" }
    $builtExe = Join-Path $TargetDir "$binSubDir\$binaryName.exe"

    if (-not (Test-Path $builtExe)) {
        Write-ErrorMsg "Binary hasil kompilasi tidak ditemukan di $builtExe"
        exit 1
    }

    Write-Success "Berhasil membangun $binaryName.exe di $builtExe"
    return $builtExe
}

# --- Packaging Function ---
function Package-Windows([string]$exePath, [bool]$createZip) {
    Write-Status "Menyiapkan direktori distribusi: $WindowsDistDir"
    if (-not (Test-Path $WindowsDistDir)) {
        New-Item -ItemType Directory -Path $WindowsDistDir -Force | Out-Null
    }

    $destExe = Join-Path $WindowsDistDir (Split-Path -Leaf $exePath)
    Copy-Item -Path $exePath -Destination $destExe -Force
    Write-Success "Disalin: $destExe ($(Format-FileSize (Get-Item $destExe).Length))"

    # Salin aset icon dan dokumen jika ada
    $assetsSrc = Join-Path $EditorDir "assets"
    if (Test-Path $assetsSrc) {
        $assetsDst = Join-Path $WindowsDistDir "assets"
        if (-not (Test-Path $assetsDst)) {
            New-Item -ItemType Directory -Path $assetsDst -Force | Out-Null
        }
        Copy-Item -Path "$assetsSrc\*" -Destination $assetsDst -Recurse -Force
    }

    $copyFiles = @("README.md", "LICENSE", "VERSION")
    foreach ($f in $copyFiles) {
        $src = Join-Path $RootDir $f
        if (-not (Test-Path $src)) { $src = Join-Path $EditorDir $f }
        if (Test-Path $src) {
            Copy-Item -Path $src -Destination $WindowsDistDir -Force
        }
    }

    if ($createZip) {
        $zipName = "ducad-$Version-windows-x86_64.zip"
        $zipPath = Join-Path $WindowsDistDir $zipName
        if (Test-Path $zipPath) {
            Remove-Item -Force $zipPath
        }

        Write-Status "Membuat paket ZIP distribusi: $zipPath..."
        $stagingDir = Join-Path ([System.IO.Path]::GetTempPath()) ("ducad_pkg_" + [System.IO.Path]::GetRandomFileName())
        New-Item -ItemType Directory -Path $stagingDir -Force | Out-Null

        try {
            $bundleFolder = Join-Path $stagingDir "DUCAD"
            New-Item -ItemType Directory -Path $bundleFolder -Force | Out-Null
            
            Copy-Item -Path "$WindowsDistDir\*" -Exclude "*.zip" -Destination $bundleFolder -Recurse -Force
            Compress-Archive -Path "$stagingDir\*" -DestinationPath $zipPath -CompressionLevel Optimal
            Write-Success "Paket ZIP selesai: $zipPath ($(Format-FileSize (Get-Item $zipPath).Length))"
        } finally {
            if (Test-Path $stagingDir) {
                Remove-Item -Recurse -Force $stagingDir -ErrorAction SilentlyContinue
            }
        }
    }
}

# --- Main Dispatcher ---
Check-Prerequisites

if ($Deps) {
    Install-Dependencies
}

if ($SyncVersion) {
    Sync-Versions
}

# Handle Clean Options
if ($CleanAll -or ($Target -eq "clean-all")) {
    Clean-Build -deepClean $true
    if ($Target -eq "clean" -or $Target -eq "clean-all") { exit 0 }
} elseif ($Clean -or $CleanNoOcct -or ($Target -eq "clean")) {
    Clean-Build -deepClean $false
    if ($Target -eq "clean") { exit 0 }
}

$builtExes = @()

switch ($Target) {
    "check" {
        Write-Status "Menjalankan cargo check untuk workspace..."
        cargo check --workspace
        Write-Success "Pemeriksaan kode berhasil."
        exit 0
    }

    "test" {
        Write-Status "Menjalankan cargo test..."
        cargo test --workspace
        Write-Success "Seluruh pengujian lulus."
        exit 0
    }

    "cli" {
        $exe = Invoke-DucadBuild -packageName "ducad-cli" -binaryName "ducad-cli"
        Package-Windows -exePath $exe -createZip $Zip
        $builtExes += $exe
    }

    "app" {
        $exe = Invoke-DucadBuild -packageName "ducad-app" -binaryName "ducad"
        Package-Windows -exePath $exe -createZip $Zip
        $builtExes += $exe
    }

    "bundle" {
        $exe = Invoke-DucadBuild -packageName "ducad-app" -binaryName "ducad"
        Package-Windows -exePath $exe -createZip $true
        $builtExes += $exe
    }

    "all" {
        $exeApp = Invoke-DucadBuild -packageName "ducad-app" -binaryName "ducad"
        $exeCli = Invoke-DucadBuild -packageName "ducad-cli" -binaryName "ducad-cli"
        Package-Windows -exePath $exeApp -createZip $false
        Package-Windows -exePath $exeCli -createZip ($Zip -or $true)
        $builtExes += $exeApp
        $builtExes += $exeCli
    }

    "run" {
        $exe = Invoke-DucadBuild -packageName "ducad-app" -binaryName "ducad"
        $builtExes += $exe
        $Run = $true
    }
}

# --- Summary Output ---
Write-Host ""
Write-Success "Proses build selesai dengan sukses!"
Write-Host ""
Write-Status "Berkas hasil di ${WindowsDistDir}:"
if (Test-Path $WindowsDistDir) {
    Get-ChildItem -Path $WindowsDistDir -File | ForEach-Object {
        $size = Format-FileSize $_.Length
        Write-Host "  * $($_.Name) ($size)" -ForegroundColor Green
    }
}

if ($Run -and $builtExes.Count -gt 0) {
    $mainExe = $builtExes[0]
    Write-Host ""
    Write-Status "Menjalankan $mainExe..."
    Start-Process -FilePath $mainExe
}
