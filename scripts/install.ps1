# Install prebuilt Windows zed-cc and register global Zed tasks.
# Usage: powershell -ExecutionPolicy Bypass -File scripts/install.ps1 -Repository OWNER/REPOSITORY
param(
    [Parameter(Mandatory=$true)]
    [string]$Repository
)
$ErrorActionPreference = 'Stop'
if ($Repository -notmatch '^[a-zA-Z0-9_.-]+/[a-zA-Z0-9_.-]+$') {
    throw 'Repository must be in OWNER/REPOSITORY format'
}
$target = 'x86_64-pc-windows-msvc'
if ([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture -ne [System.Runtime.InteropServices.Architecture]::X64) {
    throw 'Only Windows x64 prebuilt package is available. Use cargo install --git on other architectures.'
}
$asset = "zed-cc-$target.zip"
$base = "https://github.com/$Repository/releases/latest/download"
$tempDir = Join-Path ([System.IO.Path]::GetTempPath()) ("zed-cc-" + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $tempDir -Force | Out-Null
try {
    $archive = Join-Path $tempDir $asset
    $checksums = Join-Path $tempDir 'SHA256SUMS.txt'
    Invoke-WebRequest -Uri "$base/$asset" -OutFile $archive
    Invoke-WebRequest -Uri "$base/SHA256SUMS.txt" -OutFile $checksums
    $checksumLine = @(Get-Content $checksums | Where-Object { $_.EndsWith("  $asset") })
    if ($checksumLine.Count -ne 1) { throw 'Checksum entry not found or ambiguous' }
    $expected = ($checksumLine[0] -split '\s+')[0].ToUpperInvariant()
    $actual = (Get-FileHash -Path $archive -Algorithm SHA256).Hash.ToUpperInvariant()
    if ($expected -ne $actual) { throw 'Checksum mismatch. Refusing to install.' }
    Expand-Archive -Path $archive -DestinationPath $tempDir -Force
    $installDir = Join-Path $HOME '.local\bin'
    New-Item -ItemType Directory -Path $installDir -Force | Out-Null
    $exe = Join-Path $installDir 'zed-cc.exe'
    Copy-Item (Join-Path $tempDir 'zed-cc.exe') $exe -Force
    & $exe init-zed --global --bin $exe
    if ($LASTEXITCODE -ne 0) { throw 'Zed task registration failed' }
    Write-Host "Installed: $exe"
    Write-Host 'Restart Zed, then open Command Palette -> task: spawn -> Conventional Commit (Rust)'
    Write-Host "To use zed-cc in a regular terminal, add $installDir to your user PATH."
}
finally {
    Remove-Item $tempDir -Recurse -Force -ErrorAction SilentlyContinue
}
