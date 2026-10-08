[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$repoPath = Split-Path -Parent $PSScriptRoot
$runKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$entry = (Get-ItemProperty -LiteralPath $runKey).PSObject.Properties['ViperTray']
$originalValue = if ($entry) { [string]$entry.Value } else { $null }
$originalKind = if ($entry) { (Get-Item -LiteralPath $runKey).GetValueKind('ViperTray') } else { $null }
$runningApps = @(Get-Process viper-tray -ErrorAction SilentlyContinue)
$restartPaths = @($runningApps | ForEach-Object Path | Sort-Object -Unique)
Push-Location $repoPath
try {
    $runningApps | Stop-Process
    Remove-ItemProperty -LiteralPath $runKey -Name ViperTray -ErrorAction SilentlyContinue
    foreach ($test in @('wireless_every_menu_control', 'native_tray_lifecycle', 'wireless_unchanged_polling_latency')) {
        cargo test --locked $test -- --ignored --nocapture --test-threads=1
        if ($LASTEXITCODE -ne 0) { throw "Hardware test failed: $test" }
    }
} finally {
    if ($null -ne $originalValue) {
        Set-ItemProperty -LiteralPath $runKey -Name ViperTray -Value $originalValue -Type $originalKind
    } else {
        Remove-ItemProperty -LiteralPath $runKey -Name ViperTray -ErrorAction SilentlyContinue
    }
    foreach ($appPath in $restartPaths) {
        if (Test-Path -LiteralPath $appPath) { Start-Process -FilePath $appPath -WindowStyle Hidden }
    }
    Pop-Location
}
