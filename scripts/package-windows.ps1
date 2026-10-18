param([string]$TargetDir = 'target', [string]$OutputName = 'aster-windows-x64', [string]$BinaryRevision = '')
$ErrorActionPreference = 'Stop'
$taskRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$taskTarget = [IO.Path]::GetFullPath((Join-Path $taskRoot $TargetDir))
$taskRelease = Join-Path $taskTarget 'release'
$taskBinary = Join-Path $taskRelease 'rust-hmcl-desktop.exe'
if (-not (Test-Path -LiteralPath $taskBinary)) { throw 'Build the Tauri desktop application before packaging.' }
& node (Join-Path $PSScriptRoot 'verify-windows-exe.mjs') $taskBinary
if ($LASTEXITCODE -ne 0) { throw 'Single executable packaging requires a statically linked WebView2 loader.' }
if ($OutputName -notmatch '^aster-windows-x64(?:-[a-z0-9-]+)?$') { throw 'Invalid package output name.' }
$taskOutput = [IO.Path]::GetFullPath((Join-Path $taskRoot ('dist/' + $OutputName)))
if (-not $taskOutput.StartsWith($taskRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'Output must stay inside the project.' }
New-Item -ItemType Directory -Path $taskOutput -Force | Out-Null
if ((Get-Item -LiteralPath $taskOutput).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Package output must not be a link.' }
Copy-Item -LiteralPath $taskBinary -Destination (Join-Path $taskOutput 'AsterLauncher.exe') -Force
$taskNames = @('AsterLauncher.exe', 'LICENSE', 'ATTRIBUTION.md', 'ASTER-ATTRIBUTION.md', 'README.md')
$taskLoader = Join-Path $taskOutput 'WebView2Loader.dll'
if (Test-Path -LiteralPath $taskLoader) {
  if ((Get-Item -LiteralPath $taskLoader).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Old loader must not be a link.' }
  Remove-Item -LiteralPath $taskLoader -Force
}
Copy-Item -LiteralPath (Join-Path $taskRoot 'LICENSE') -Destination $taskOutput -Force
Copy-Item -LiteralPath (Join-Path $taskRoot 'assets/hmcl/ATTRIBUTION.md') -Destination $taskOutput -Force
Copy-Item -LiteralPath (Join-Path $taskRoot 'assets/aster/ATTRIBUTION.md') -Destination (Join-Path $taskOutput 'ASTER-ATTRIBUTION.md') -Force
Copy-Item -LiteralPath (Join-Path $taskRoot 'docs/ASTER-README.md') -Destination (Join-Path $taskOutput 'README.md') -Force
$taskRevision = (& git -C $taskRoot rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0) { throw 'Cannot read source revision.' }
$taskBinaryRevision = $taskRevision
if ($BinaryRevision) {
  if ($BinaryRevision -notmatch '^[a-f0-9]{7,40}$') { throw 'Invalid binary source revision.' }
  $taskBinaryRevision = (& git -C $taskRoot rev-parse --verify ($BinaryRevision + '^{commit}')).Trim()
  if ($LASTEXITCODE -ne 0) { throw 'Cannot resolve binary source revision.' }
}
$taskDirty = [bool](& git -C $taskRoot status --porcelain)
$taskManifest = @{
  product = 'Aster Launcher'
  preview = $true
  source_revision = $taskBinaryRevision
  packaging_revision = $taskRevision
  source_dirty = $taskDirty
  frontend = 'Tauri + Vue + TypeScript'
  webview2_loader = 'statically linked'
  migration_complete = $false
  built_files = @($taskNames | ForEach-Object { @{name=$_;sha256=(Get-FileHash -LiteralPath (Join-Path $taskOutput $_)).Hash} })
}
$taskUtf8 = New-Object System.Text.UTF8Encoding($false)
[IO.File]::WriteAllText((Join-Path $taskOutput 'build-info.json'), ($taskManifest | ConvertTo-Json -Depth 5), $taskUtf8)
$taskFiles = @(($taskNames + 'build-info.json') | ForEach-Object { Join-Path $taskOutput $_ })
$taskArchive = Join-Path $taskRoot ('dist/aster-launcher-windows-x64-' + $taskRevision.Substring(0, 7) + '.zip')
Compress-Archive -LiteralPath $taskFiles -DestinationPath $taskArchive -Force
Write-Output $taskOutput
Write-Output $taskArchive
