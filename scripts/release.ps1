# Builds the public release installer without local paths of the build machine.
#   powershell -ExecutionPolicy Bypass -File scripts\release.ps1
#
# Rust embeds source paths (panic locations of dependencies) and the PDB path into the exe.
# The remaps below replace the user profile and the project folder with neutral names, and
# /PDBALTPATH keeps only the PDB file name. The script then checks the result and prints SHA-256.
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$sep = [char]0x1f
$flags = @(
  "--remap-path-prefix=$($env:USERPROFILE)=~",
  "--remap-path-prefix=$root=veliora",
  '-C', 'link-arg=/PDBALTPATH:%_PDB%'
)
$env:CARGO_ENCODED_RUSTFLAGS = $flags -join $sep
Remove-Item Env:RUSTFLAGS -ErrorAction SilentlyContinue

Push-Location $root
try {
  npm run app:build
  if ($LASTEXITCODE -ne 0) { throw "tauri build failed ($LASTEXITCODE)" }
} finally { Pop-Location }

$exe = Join-Path $root 'src-tauri\target\release\VELIORA.exe'
$bytes = [IO.File]::ReadAllBytes($exe)
$text = [Text.Encoding]::ASCII.GetString($bytes) + [Text.Encoding]::Unicode.GetString($bytes)
$leaks = @($env:USERPROFILE, $env:USERNAME, $root) | Where-Object { $_ -and $text.IndexOf($_, [StringComparison]::OrdinalIgnoreCase) -ge 0 }
if ($leaks.Count) { throw "VELIORA.exe still contains local paths ($($leaks.Count) markers) - not publishing this build" }
Write-Host 'VELIORA.exe: no local paths found'

$setup = Get-ChildItem (Join-Path $root 'src-tauri\target\release\bundle\nsis') -Filter '*-setup.exe' | Sort-Object LastWriteTime -Descending | Select-Object -First 1
$hash = (Get-FileHash $setup.FullName -Algorithm SHA256).Hash.ToLower()
Write-Host "$($setup.Name)  $([math]::Round($setup.Length / 1MB, 1)) MB"
Write-Host "SHA-256  $hash"
