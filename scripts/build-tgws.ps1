# Rebuilds the headless Telegram WS proxy (TgWsProxy.exe) from Flowseal/tg-ws-proxy source.
#   powershell -ExecutionPolicy Bypass -File scripts\build-tgws.ps1 [-Tag v1.11.4]
#
# Why a rebuild: the official Windows release is a tray GUI that ignores --host/--port/--secret.
# The CLI entry point (proxy.tg_ws_proxy:main) is frozen with PyInstaller instead; the source is
# used unmodified. Everything happens in a temporary folder with its own virtual environment.
param([string]$Tag = 'v1.11.4')
$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$out = Join-Path $root 'src-tauri\components\tgws'
$work = Join-Path ([IO.Path]::GetTempPath()) ("veliora-tgws-" + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force $work | Out-Null

try {
  git clone --depth 1 --branch $Tag https://github.com/Flowseal/tg-ws-proxy.git (Join-Path $work 'src')
  python -m venv (Join-Path $work 'venv')
  $py = Join-Path $work 'venv\Scripts\python.exe'
  & $py -m pip install --disable-pip-version-check -q 'pyinstaller==6.16.0' 'cryptography==46.0.5' 'httpx[http2]==0.28.1' certifi
  Set-Content -Encoding ascii (Join-Path $work 'entry.py') "from proxy.tg_ws_proxy import main`nif __name__ == '__main__':`n    main()"
  & $py -m PyInstaller --noconfirm --onefile --console --name TgWsProxy `
      --distpath (Join-Path $work 'dist') --workpath (Join-Path $work 'build') --specpath (Join-Path $work 'build') `
      --paths (Join-Path $work 'src') --collect-submodules proxy --collect-submodules cryptography `
      --collect-submodules h2 --collect-submodules httpx --hidden-import certifi (Join-Path $work 'entry.py')
  New-Item -ItemType Directory -Force $out | Out-Null
  Copy-Item -Force (Join-Path $work 'dist\TgWsProxy.exe') (Join-Path $out 'TgWsProxy.exe')
  $hash = (Get-FileHash (Join-Path $out 'TgWsProxy.exe') -Algorithm SHA256).Hash.ToLower()
  $manifestPath = Join-Path $root 'src-tauri\components\components.json'
  $manifest = if (Test-Path $manifestPath) { Get-Content $manifestPath -Raw | ConvertFrom-Json } else { [pscustomobject]@{} }
  $tgws = [pscustomobject]@{ version = $Tag.TrimStart('v'); source = "https://github.com/Flowseal/tg-ws-proxy/tree/$Tag"; sha256 = $hash; build = 'PyInstaller 6.16.0, onefile, entry proxy.tg_ws_proxy:main' }
  $manifest | Add-Member -Force -NotePropertyName tgws -NotePropertyValue $tgws
  $manifest | ConvertTo-Json -Depth 4 | Set-Content -Encoding utf8 $manifestPath
  Write-Host "TgWsProxy $Tag built: $hash"
} finally {
  Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
}
