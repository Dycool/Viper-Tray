[CmdletBinding()]
param(
    [string]$OutputDirectory = 'dist'
)

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
$stagePath = Join-Path ([System.IO.Path]::GetTempPath()) ("viper-tray-package-" + [guid]::NewGuid())
$portablePath = Join-Path $stagePath 'ViperTray'
New-Item -ItemType Directory -Path $portablePath -Force | Out-Null

Copy-Item -LiteralPath $exePath -Destination (Join-Path $outputPath 'viper-tray.exe')
Copy-Item -LiteralPath $exePath -Destination $portablePath
foreach ($name in @('README.md', 'LICENSE', 'THIRD_PARTY_NOTICES.md', 'VALIDATION.md')) {
    Copy-Item -LiteralPath (Join-Path $repoPath $name) -Destination $portablePath
}
foreach ($name in @('assets', 'docs')) {
    Copy-Item -LiteralPath (Join-Path $repoPath $name) -Destination $portablePath -Recurse
}

$sourceArchive = Join-Path $outputPath 'viper-tray-source.zip'
git -C $repoPath archive --format=zip "--output=$sourceArchive" HEAD
if ($LASTEXITCODE -ne 0) { throw 'Could not package the committed source' }
Copy-Item -LiteralPath $sourceArchive -Destination $portablePath

$portableArchive = Join-Path $outputPath 'ViperTray-windows-x64.zip'
Compress-Archive -LiteralPath $portablePath -DestinationPath $portableArchive -Force
$checksums = foreach ($name in @('viper-tray.exe', 'ViperTray-windows-x64.zip', 'viper-tray-source.zip')) {
    $hash = (Get-FileHash -LiteralPath (Join-Path $outputPath $name) -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash  $name"
}
$checksums | Set-Content -LiteralPath (Join-Path $outputPath 'SHA256SUMS.txt') -Encoding utf8
Write-Host "Portable downloads written to $outputPath"

# Only this invocation's unique temporary directory is eligible for cleanup.
$tempRoot = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath()).TrimEnd('\') + '\'
$resolvedStage = [System.IO.Path]::GetFullPath($stagePath)
if (-not $resolvedStage.StartsWith($tempRoot, [System.StringComparison]::OrdinalIgnoreCase) -or
    -not (Split-Path -Leaf $resolvedStage).StartsWith('viper-tray-package-')) {
    throw 'Refusing to clean an unexpected temporary directory'
}
Remove-Item -LiteralPath $resolvedStage -Recurse -Force
