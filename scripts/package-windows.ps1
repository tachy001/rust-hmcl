param([string]$TargetDir = 'target')
$ErrorActionPreference = 'Stop'
$taskRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$taskTarget = [IO.Path]::GetFullPath((Join-Path $taskRoot $TargetDir))
$taskRelease = Join-Path $taskTarget 'release'
$taskBinary = Join-Path $taskRelease 'rust-hmcl-desktop.exe'
if (-not (Test-Path -LiteralPath $taskBinary)) { throw 'Build the Tauri desktop application before packaging.' }
$taskOutput = [IO.Path]::GetFullPath((Join-Path $taskRoot 'dist/web-windows-x64'))
if (-not $taskOutput.StartsWith($taskRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'Output must stay inside the project.' }
New-Item -ItemType Directory -Path $taskOutput -Force | Out-Null
Copy-Item -LiteralPath $taskBinary -Destination (Join-Path $taskOutput 'rust-hmcl.exe') -Force
$taskNames = @('rust-hmcl.exe', 'LICENSE', 'ATTRIBUTION.md', 'README.md')
$taskLoader = Join-Path $taskRelease 'WebView2Loader.dll'
if (Test-Path -LiteralPath $taskLoader) {
  Copy-Item -LiteralPath $taskLoader -Destination $taskOutput -Force
  $taskNames += 'WebView2Loader.dll'
}
Copy-Item -LiteralPath (Join-Path $taskRoot 'LICENSE') -Destination $taskOutput -Force
Copy-Item -LiteralPath (Join-Path $taskRoot 'assets/hmcl/ATTRIBUTION.md') -Destination $taskOutput -Force
Copy-Item -LiteralPath (Join-Path $taskRoot 'docs/WEB-MIGRATION.md') -Destination (Join-Path $taskOutput 'README.md') -Force
$taskRevision = (& git -C $taskRoot rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0) { throw 'Cannot read source revision.' }
$taskDirty = [bool](& git -C $taskRoot status --porcelain)
$taskManifest = @{
  source_revision = $taskRevision
  source_dirty = $taskDirty
  frontend = 'Tauri + Vue + TypeScript'
  migration_complete = $false
  built_files = @($taskNames | ForEach-Object { @{name=$_;sha256=(Get-FileHash -LiteralPath (Join-Path $taskOutput $_)).Hash} })
}
$taskUtf8 = New-Object System.Text.UTF8Encoding($false)
[IO.File]::WriteAllText((Join-Path $taskOutput 'build-info.json'), ($taskManifest | ConvertTo-Json -Depth 5), $taskUtf8)
$taskFiles = @(($taskNames + 'build-info.json') | ForEach-Object { Join-Path $taskOutput $_ })
Compress-Archive -LiteralPath $taskFiles -DestinationPath (Join-Path $taskRoot 'dist/rust-hmcl-web-windows-x64.zip') -Force
Write-Output $taskOutput
