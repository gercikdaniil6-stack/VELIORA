// Fetches the network components VELIORA App ships with and verifies them.
//   npm run components            — Zapret bundle (default version below)
//   npm run components -- 1.10.5  — a specific Zapret release
//
// Zapret: official release archive of Flowseal/zapret-discord-youtube from
// GitHub. The archive's SHA-256 must match the digest GitHub publishes for the
// asset, otherwise nothing is written.
//
// TgWsProxy is NOT downloaded: the official Windows release is a tray GUI that
// ignores command-line options. It is rebuilt from Flowseal/tg-ws-proxy source
// by scripts/build-tgws.ps1 (see docs/COMPONENTS.md).
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { cpSync, existsSync, mkdirSync, mkdtempSync, readdirSync, rmSync, writeFileSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const COMPONENTS = join(ROOT, 'src-tauri', 'components');
const ZAPRET_REPO = 'Flowseal/zapret-discord-youtube';
const ZAPRET_VERSION = process.argv[2] || '1.10.4';

const gh = async (url) => {
  const res = await fetch(url, { headers: { 'User-Agent': 'VELIORA-App-build', Accept: 'application/vnd.github+json' } });
  if (!res.ok) throw new Error(`GitHub ${res.status} for ${url}`);
  return res;
};

async function fetchZapret() {
  const rel = await (await gh(`https://api.github.com/repos/${ZAPRET_REPO}/releases/tags/${ZAPRET_VERSION}`)).json();
  const asset = rel.assets.find((a) => /\.zip$/i.test(a.name));
  if (!asset) throw new Error(`no .zip asset in release ${ZAPRET_VERSION}`);
  const expected = String(asset.digest || '').replace(/^sha256:/, '');
  if (!/^[0-9a-f]{64}$/.test(expected)) throw new Error('GitHub did not publish a SHA-256 digest for the asset');

  const bytes = Buffer.from(await (await gh(asset.browser_download_url)).arrayBuffer());
  const actual = createHash('sha256').update(bytes).digest('hex');
  if (actual !== expected) throw new Error(`SHA-256 mismatch: expected ${expected}, got ${actual}`);

  const work = mkdtempSync(join(tmpdir(), 'veliora-zapret-'));
  try {
    const zip = join(work, asset.name);
    writeFileSync(zip, bytes);
    // bsdtar ships with Windows 10+ and reads .zip archives.
    // Use Windows' own bsdtar explicitly: a GNU tar from Git/MSYS earlier in PATH
    // would read "C:" as a remote host.
    const tar = process.platform === 'win32' ? join(process.env.SystemRoot || 'C:\\Windows', 'System32', 'tar.exe') : 'tar';
    execFileSync(tar, ['-xf', zip, '-C', work]);
    const root = readdirSync(work, { withFileTypes: true }).find((d) => d.isDirectory());
    const src = root ? join(work, root.name) : work;
    if (!existsSync(join(src, 'bin', 'winws.exe'))) throw new Error('archive has no bin/winws.exe');
    const dest = join(COMPONENTS, 'zapret');
    rmSync(dest, { recursive: true, force: true });
    mkdirSync(dest, { recursive: true });
    cpSync(src, dest, { recursive: true });
  } finally {
    rmSync(work, { recursive: true, force: true });
  }
  return { version: ZAPRET_VERSION, asset: asset.name, sha256: expected, source: asset.browser_download_url };
}

const manifestPath = join(COMPONENTS, 'components.json');
const manifest = existsSync(manifestPath) ? JSON.parse(readFileSync(manifestPath, 'utf8')) : {};
manifest.zapret = await fetchZapret();
writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
console.log(`Zapret ${manifest.zapret.version} verified (sha256 ${manifest.zapret.sha256.slice(0, 16)}…) → src-tauri/components/zapret`);
