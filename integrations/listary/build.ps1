param(
    [string]$ListaryDirectory = 'C:\Program Files\Listary',
    [string]$OutputDirectory
)
$ErrorActionPreference = 'Stop'
$repoDirectory = Split-Path (Split-Path $PSScriptRoot)
if (-not $OutputDirectory) { $OutputDirectory = Join-Path $repoDirectory 'target\listary-plugin\FileFlow' }
$compiler = Join-Path $env:WINDIR 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'
$sdk = Join-Path $ListaryDirectory 'Listary.FileAppPlugin.dll'
if (-not (Test-Path -LiteralPath $compiler) -or -not (Test-Path -LiteralPath $sdk)) {
    throw 'Listary 6 and .NET Framework 4.8 are required to build its plugin.'
}
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$output = Join-Path $OutputDirectory 'Listary.FileAppPlugin.FileFlow.dll'
& $compiler /nologo /target:library /optimize+ "/out:$output" "/reference:$sdk" /reference:System.Web.Extensions.dll (Join-Path $PSScriptRoot 'FileFlowPlugin.cs')
if ($LASTEXITCODE -ne 0) { throw 'Listary plugin compilation failed' }
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'ListaryPluginManifest.json') -Destination $OutputDirectory -Force
Copy-Item -LiteralPath (Join-Path $repoDirectory 'Assets\FileFlow.png') -Destination (Join-Path $OutputDirectory 'Icon.png') -Force
Write-Output "Built plugin: $output"
