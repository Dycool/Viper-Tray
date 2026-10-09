[CmdletBinding()]
param([string]$OutputDirectory = 'dist')

$ErrorActionPreference = 'Stop'
$repoPath = Split-Path -Parent $PSScriptRoot
$outputPath = if ([System.IO.Path]::IsPathRooted($OutputDirectory)) {
    [System.IO.Path]::GetFullPath($OutputDirectory)
} else {
    [System.IO.Path]::GetFullPath((Join-Path $repoPath $OutputDirectory))
}
$exePath = Join-Path $repoPath 'target\release\viper-tray.exe'
if (-not (Test-Path -LiteralPath $exePath -PathType Leaf)) {
    throw 'Build the executable first: cargo build --release --locked'
}
New-Item -ItemType Directory -Path $outputPath -Force | Out-Null
$packagedExePath = Join-Path $outputPath 'Viper-tray.exe'
if ($packagedExePath.Equals($exePath, [StringComparison]::OrdinalIgnoreCase)) {
    throw 'Package output must be different from the build directory'
}
if (Test-Path -LiteralPath $packagedExePath) { Remove-Item -LiteralPath $packagedExePath }
Copy-Item -LiteralPath $exePath -Destination $packagedExePath

# The public portable download contains exactly one file at the ZIP root.
$portableArchive = Join-Path $outputPath 'ViperTray-windows-x64.zip'
Add-Type -AssemblyName System.IO.Compression.FileSystem
if (Test-Path -LiteralPath $portableArchive) { Remove-Item -LiteralPath $portableArchive }
$zip = [System.IO.Compression.ZipFile]::Open($portableArchive, [System.IO.Compression.ZipArchiveMode]::Create)
try {
    [System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile(
        $zip, $packagedExePath, 'Viper-tray.exe', [System.IO.Compression.CompressionLevel]::Optimal
    ) | Out-Null
} finally {
    $zip.Dispose()
}
$archive = [System.IO.Compression.ZipFile]::OpenRead($portableArchive)
try {
    if ($archive.Entries.Count -ne 1 -or $archive.Entries[0].FullName -cne 'Viper-tray.exe') {
        throw 'Portable ZIP must contain only Viper-tray.exe at its root'
    }
} finally {
    $archive.Dispose()
}

# Separate source/checksum files are CI artifacts, never included in the portable ZIP.
$sourceArchive = Join-Path $outputPath 'viper-tray-source.zip'
git -C $repoPath archive --format=zip "--output=$sourceArchive" HEAD
if ($LASTEXITCODE -ne 0) { throw 'Could not package the committed source' }
$checksums = foreach ($name in @('Viper-tray.exe', 'ViperTray-windows-x64.zip', 'viper-tray-source.zip')) {
    $hash = (Get-FileHash -LiteralPath (Join-Path $outputPath $name) -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash  $name"
}
$checksums | Set-Content -LiteralPath (Join-Path $outputPath 'SHA256SUMS.txt') -Encoding utf8
Write-Host "Portable ZIP contains only Viper-tray.exe: $portableArchive"
