param([Parameter(Mandatory=$true)][string]$Installer)
$ErrorActionPreference = 'Stop'
$testRoot = Join-Path $env:RUNNER_TEMP ('mz-uninstall-test-' + [guid]::NewGuid())
$appDir = Join-Path $testRoot 'app'
$sharedDir = Join-Path $testRoot 'shared-cache-with-personal-files'
New-Item -ItemType Directory -Path $appDir,$sharedDir | Out-Null
$installerPath = (Resolve-Path -LiteralPath $Installer).ProviderPath
$install = Start-Process -FilePath $installerPath -ArgumentList @('/S','/NS',('/D=' + $appDir)) -Wait -PassThru
if ($install.ExitCode -ne 0) { throw "Test installation failed: $($install.ExitCode)" }
if (-not (Test-Path -LiteralPath (Join-Path $appDir 'mz-browser.exe'))) { throw 'Test program missing' }

$nestedDir = Join-Path $appDir 'personal-files'
New-Item -ItemType Directory -Path $nestedDir | Out-Null
$sentinels = @((Join-Path $appDir 'keep-personal.txt'), (Join-Path $nestedDir 'keep.txt'), (Join-Path $sharedDir 'keep.txt'))
$content = 'Uninstall must preserve this unrelated file.'
foreach ($sentinel in $sentinels) { Set-Content -LiteralPath $sentinel -Value $content }
$runtimeKey = 'HKCU:\Software\MZBrowser\RuntimePaths'
# Exercise both the actual application key and the old uninstaller's mismatched key.
foreach ($key in @($runtimeKey, 'HKCU:\Software\MZ指纹浏览器\RuntimePaths')) {
  New-Item -Path $key -Force | Out-Null
  foreach ($name in @('RootDir','CacheDir','LogsDir','DataDir','ConfigDir','KernelsDir')) {
    New-ItemProperty -LiteralPath $key -Name $name -Value $sharedDir -PropertyType String -Force | Out-Null
  }
}
$uninstall = Start-Process -FilePath (Join-Path $appDir 'uninstall.exe') -ArgumentList '/S' -Wait -PassThru
if ($uninstall.ExitCode -ne 0) { throw "Uninstaller failed: $($uninstall.ExitCode)" }
$deadline = (Get-Date).AddSeconds(45)
do {
  Start-Sleep -Milliseconds 500
  $stillInstalled = Test-Path -LiteralPath (Join-Path $appDir 'mz-browser.exe')
} while ($stillInstalled -and (Get-Date) -lt $deadline)
if ($stillInstalled) { throw 'Package-owned program was not removed' }
Start-Sleep -Seconds 2
foreach ($sentinel in $sentinels) {
  if (-not (Test-Path -LiteralPath $sentinel)) { throw "Uninstall deleted unrelated file: $sentinel" }
  if ((Get-Content -LiteralPath $sentinel -Raw).Trim() -ne $content) { throw 'Unrelated file contents changed' }
}
if ((Get-ItemProperty -LiteralPath $runtimeKey).CacheDir -ne $sharedDir) { throw 'Preserved runtime path was lost' }
Write-Host 'Real installer/uninstaller test passed: packaged program removed, unrelated files and shared data preserved.'
