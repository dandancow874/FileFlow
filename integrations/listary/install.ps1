param(
    [string]$ListaryDirectory = 'C:\Program Files\Listary',
    [switch]$SkipRestart
)
$ErrorActionPreference = 'Stop'
& (Join-Path $PSScriptRoot 'build.ps1') -ListaryDirectory $ListaryDirectory
$repoDirectory = Split-Path (Split-Path $PSScriptRoot)
$builtDirectory = Join-Path $repoDirectory 'target\listary-plugin\FileFlow'
$pluginDirectory = Join-Path $ListaryDirectory 'FileAppPlugins\FileFlow'
$preferencesPath = Join-Path $env:APPDATA 'Listary\UserProfile\Settings\Preferences.json'
if (-not (Test-Path -LiteralPath $preferencesPath)) { throw 'Listary preferences were not found.' }
# Fail before stopping Listary if this Windows token cannot write its protected
# plugin directory. An elevated install can use -SkipRestart so the original,
# non-elevated caller restarts Listary at its normal integrity level afterwards.
$pluginsRoot = Join-Path $ListaryDirectory 'FileAppPlugins'
$writeProbe = Join-Path $pluginsRoot ('.fileflow-write-check-' + [Guid]::NewGuid().ToString('N'))
try {
    [IO.File]::WriteAllText($writeProbe, '', [Text.UTF8Encoding]::new($false))
} catch {
    throw "Windows denied write access to $pluginsRoot. Administrator authorization is required; Listary has been left running."
} finally {
    if (Test-Path -LiteralPath $writeProbe) { Remove-Item -LiteralPath $writeProbe }
}
$timestamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$preferencesBackup = Join-Path (Split-Path $preferencesPath) "Preferences.before-fileflow-$timestamp.json"
Copy-Item -LiteralPath $preferencesPath -Destination $preferencesBackup
if (Test-Path -LiteralPath $pluginDirectory) {
    Copy-Item -LiteralPath $pluginDirectory -Destination (Join-Path $repoDirectory "target\listary-plugin\previous-$timestamp") -Recurse
}
$listaryExe = Join-Path $ListaryDirectory 'Listary.exe'
$running = @(Get-Process Listary -ErrorAction SilentlyContinue | Where-Object Path -eq $listaryExe)
foreach ($process in $running) { Stop-Process -Id $process.Id -Force; $process.WaitForExit() }
try {
New-Item -ItemType Directory -Path $pluginDirectory -Force | Out-Null
foreach ($name in @('Listary.FileAppPlugin.FileFlow.dll','ListaryPluginManifest.json','Icon.png')) {
    Copy-Item -LiteralPath (Join-Path $builtDirectory $name) -Destination (Join-Path $pluginDirectory $name) -Force
    if ((Get-FileHash -LiteralPath (Join-Path $builtDirectory $name)).Hash -ne (Get-FileHash -LiteralPath (Join-Path $pluginDirectory $name)).Hash) { throw "Plugin hash mismatch: $name" }
}
$preferences = Get-Content -LiteralPath $preferencesPath -Raw | ConvertFrom-Json
$originalPreferences = $preferences | ConvertTo-Json -Depth 100 -Compress
$priorConfig = $preferences.AppConfigs.TypeConfigs.PSObject.Properties['FileFlow']
$priorValue = if ($priorConfig) { $priorConfig.Value } else { $null }
$preferences.AppConfigs.TypeConfigs | Add-Member -MemberType NoteProperty -Name FileFlow -Value ([pscustomobject]@{
    Enabled = $true
    EnableInstantTypeSearch = $false
    EnableInstantTypeSearchForGeneralLists = $false
    EnableMouseDoubleClick = $false
    EnableMouseMiddleClick = $false
}) -Force
$updatedJson = $preferences | ConvertTo-Json -Depth 100
# Confirm every other preference is preserved before writing the new integration.
if ($priorConfig) { $preferences.AppConfigs.TypeConfigs.FileFlow = $priorValue }
else { $preferences.AppConfigs.TypeConfigs.PSObject.Properties.Remove('FileFlow') }
if (($preferences | ConvertTo-Json -Depth 100 -Compress) -ne $originalPreferences) { throw 'Unrelated Listary preferences changed.' }
$temporaryPreferences = $preferencesPath + '.fileflow.tmp'
[IO.File]::WriteAllText($temporaryPreferences, $updatedJson, [Text.UTF8Encoding]::new($false))
[IO.File]::Move($temporaryPreferences, $preferencesPath, $true)
$installedConfig = (Get-Content -LiteralPath $preferencesPath -Raw | ConvertFrom-Json).AppConfigs.TypeConfigs.FileFlow
if (-not $installedConfig.Enabled -or $installedConfig.EnableInstantTypeSearch -or $installedConfig.EnableMouseDoubleClick) { throw 'Integration preferences verification failed.' }
Write-Output "Installed: $pluginDirectory"
Write-Output "Preferences backup: $preferencesBackup"
} finally {
    if ($running.Count -gt 0 -and -not $SkipRestart -and -not (Get-Process Listary -ErrorAction SilentlyContinue)) {
        Start-Process -FilePath $listaryExe -WorkingDirectory $ListaryDirectory -WindowStyle Hidden
    }
}
