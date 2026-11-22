
param([switch]$SignedUpdater)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$flags = @(
  "--remap-path-prefix=$root=veliora",
  "--remap-path-prefix=$($env:USERPROFILE)=~",
  '-C', 'link-arg=/PDBALTPATH:%_PDB%'
)
$env:CARGO_ENCODED_RUSTFLAGS = $flags -join [char]0x1f
Remove-Item Env:RUSTFLAGS -ErrorAction SilentlyContinue

Push-Location $root
$loadedKey = $false
$override = $null
try {
  $buildArgs = @('run', 'app:build', '--')
  if ($SignedUpdater) {
    if (!$env:TAURI_SIGNING_PRIVATE_KEY) {
      $signing = Join-Path $env:LOCALAPPDATA 'VELIORA-ReleaseSigning'
      $key = Join-Path $signing 'updater.key'
      $pass = Join-Path $signing 'updater.password.dpapi'
      if (!(Test-Path -LiteralPath $key) -or !(Test-Path -LiteralPath $pass)) {
        throw 'Set the Tauri signing secrets before building an updater release.'
      }
      $loadedKey = $true
      $env:TAURI_SIGNING_PRIVATE_KEY = Get-Content -LiteralPath $key -Raw
      $secure = Get-Content -LiteralPath $pass -Raw | ConvertTo-SecureString
      $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = [System.Net.NetworkCredential]::new('', $secure).Password
    }
    $override = Join-Path ([IO.Path]::GetTempPath()) ("veliora-sign-" + [guid]::NewGuid().ToString('N') + '.json')
    Set-Content -LiteralPath $override -Value '{"bundle":{"createUpdaterArtifacts":true}}' -Encoding ascii
    $buildArgs += @('--config', $override)
  }
  $buildArgs += @('--', '--locked')
  & npm.cmd @buildArgs
  if ($LASTEXITCODE -ne 0) { throw "Tauri build failed ($LASTEXITCODE)." }

  $target = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $root 'src-tauri/target' }
  $binary = Join-Path $target 'release/VELIORA.exe'
  $bytes = [IO.File]::ReadAllBytes($binary)
  $text = [Text.Encoding]::ASCII.GetString($bytes) + [Text.Encoding]::Unicode.GetString($bytes)
  $leaks = @($env:USERPROFILE, $root) | Where-Object { $_ -and $text.IndexOf($_,[StringComparison]::OrdinalIgnoreCase) -ge 0 }
  if ($leaks.Count) { throw 'VELIORA.exe still contains machine-local paths.' }

  $config = Get-Content 'src-tauri/tauri.conf.json' -Raw | ConvertFrom-Json
  $version = $config.version
  $setup = Get-Item -LiteralPath (Join-Path $target "release/bundle/nsis/$($config.productName)_$($version)_x64-setup.exe")
  $hash = (Get-FileHash -LiteralPath $setup.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
  "SHA-256 $hash"
  if ($SignedUpdater) {
    $artifactDir = Join-Path $target "release/veliora-v$version"
    New-Item -ItemType Directory -Path $artifactDir -Force | Out-Null
    $name = "VELIORA_App_$($version)_x64-setup.exe"
    $signature = (Get-Content -LiteralPath ($setup.FullName+'.sig') -Raw).Trim()
    $decoded = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($signature))
    if ($decoded -notmatch ('(?m)^trusted comment:[^\r\n]*\tversion:' + [regex]::Escape($version) + '(\s|$)')) {
      throw 'The updater signature is not bound to this release version.'
    }
    Copy-Item -LiteralPath $setup.FullName -Destination (Join-Path $artifactDir $name) -Force
    [IO.File]::WriteAllText((Join-Path $artifactDir "$name.sig"), $signature)
    [IO.File]::WriteAllText((Join-Path $artifactDir "$name.sha256"), "$hash  $name" + [char]10)
    $notes = Get-Content 'CHANGELOG.md' -Raw
    $match = [regex]::Match($notes, '(?ms)^## '+[regex]::Escape($version)+'(?=\s|$)[^\r\n]*\r?\n(.*?)(?=^## |\z)')
    if (!$match.Success) { throw 'This version has no changelog entry.' }
    $body = $match.Groups[1].Value.Trim()
    $updater = @{
      version=$version; notes=$body; pub_date=[DateTime]::UtcNow.ToString('yyyy-MM-ddTHH:mm:ssZ')
      platforms=@{'windows-x86_64'=@{
        signature=$signature
        url="https://github.com/gercikdaniil6-stack/VELIORA/releases/download/v$version/$name"
      }}
    }
    [IO.File]::WriteAllText((Join-Path $artifactDir 'latest.json'),($updater | ConvertTo-Json -Depth 5))
    [IO.File]::WriteAllText((Join-Path $artifactDir 'release-notes.md'),$body)
    "Release artifacts: $artifactDir"
  }
} finally {
  Pop-Location
  if ($override) { Remove-Item -LiteralPath $override -ErrorAction SilentlyContinue }
  if ($loadedKey) { Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY,Env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD -ErrorAction SilentlyContinue }
}
