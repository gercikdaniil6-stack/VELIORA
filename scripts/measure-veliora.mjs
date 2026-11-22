
import { spawn, execFileSync } from 'node:child_process';
import { existsSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { setTimeout as wait } from 'node:timers/promises';

const exe = process.argv[2] && resolve(process.argv[2]);
const port = Number(process.argv[3] ?? 9463);
const idleSeconds = Number(process.argv[4] ?? 60);
if (!exe || !existsSync(exe) || !Number.isInteger(port) || port < 1024 || port > 65535 ||
    !Number.isFinite(idleSeconds) || idleSeconds < 1 || idleSeconds > 3600) {
  throw new Error('node scripts/measure-veliora.mjs <isolated-QA.exe> [CDP-port] [idle-seconds]');
}
const endpoint = 'http://127.0.0.1:' + port + '/json/list';
if (await fetch(endpoint).then(() => true).catch(() => false)) throw new Error('CDP port is already in use');

const profile = mkdtempSync(join(tmpdir(), 'veliora-metrics-'));
const born = performance.now();
const child = spawn(exe, ['--no-elevate'], {
  cwd: dirname(exe), windowsHide: true,
  env: { ...process.env, WEBVIEW2_USER_DATA_FOLDER: profile,
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=' + port },
});
let ws;
let sequence = 0;
const pending = new Map();
const snapshots = [];
const errors = [];
function sample() {
  const command = [
    '$all=Get-CimInstance Win32_Process',
    '$ids=@(' + child.pid + ')',
    '$layer=$ids',
    'while ($layer.Count) { $layer=@($all | Where-Object { $_.ParentProcessId -in $layer -and $_.ProcessId -notin $ids } | ForEach-Object ProcessId); $ids+=$layer }',
    'Get-Process -Id $ids -ErrorAction SilentlyContinue | Select-Object Id, ProcessName, PrivateMemorySize64, CPU | ConvertTo-Json -Compress',
  ].join('; ');
  const response = JSON.parse(execFileSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', command], { encoding: 'utf8' }));
  return { at: performance.now(), processes: Array.isArray(response) ? response : [response] };
}
function send(method, params) {
  const id = ++sequence;
  return new Promise((resolve, reject) => {
    pending.set(id, { resolve, reject });
    ws.send(JSON.stringify({ id, method, params }));
  });
}
async function evaluate(expression) {
  const r = await send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
  if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description ?? r.exceptionDetails.text);
  return r.result.value;
}
async function until(test, timeout = 15000) {
  const end = performance.now() + timeout;
  while (performance.now() < end) {
    if (await test()) return;
    await wait(100);
  }
  throw new Error('Timed out waiting for VELIORA');
}
async function nav(id, title) {
  await evaluate('document.querySelector(' + JSON.stringify('[data-route="' + id + '"]') + ').click()');
  await until(() => evaluate('document.querySelector("main h1")?.textContent===' + JSON.stringify(title)));
}
function record(label, first, last) {
  const groups = {};
  for (const group of ['core', 'webview', 'zapret', 'other']) {
    const match = (p) => {
      if (p.Id === child.pid) return group === 'core';
      if (p.ProcessName === 'msedgewebview2') return group === 'webview';
      if (/^winws2?$/i.test(p.ProcessName)) return group === 'zapret';
      return group === 'other';
    };
    const processes = last.processes.filter(match);
    const prior = new Map(first.processes.filter(match).map((p) => [p.Id, p.CPU]));
    groups[group] = {
      processes: processes.length,
      privateMiB: +(processes.reduce((total, p) => total + p.PrivateMemorySize64, 0) / 1048576).toFixed(1),
      cpuPercentOfOneCore: +(processes.reduce((total, p) => total + Math.max(0, (p.CPU ?? 0) - (prior.get(p.Id) ?? 0)), 0) * 100000 / Math.max(1, last.at - first.at)).toFixed(2),
    };
  }
  snapshots.push({ label, groups });
}
async function measure(label) {
  const first = sample();
  await wait(3000);
  record(label, first, sample());
}
try {
  let page;
  await until(async () => {
    if (child.exitCode !== null) throw new Error('QA instance exited');
    const pages = await fetch(endpoint).then(r => r.json()).catch(() => []);
    page = pages.find((item) => item.type === 'page' && item.url.startsWith('http://tauri.localhost'));
    return page;
  });
  ws = new WebSocket(page.webSocketDebuggerUrl);
  await new Promise((resolve, reject) => { ws.addEventListener('open', resolve); ws.addEventListener('error', reject); });
  ws.addEventListener('message', ({ data }) => {
    const message = JSON.parse(data);
    const callbacks = pending.get(message.id);
    if (callbacks) {
      pending.delete(message.id);
      if (message.error) callbacks.reject(new Error(message.error.message));
      else callbacks.resolve(message.result);
    }
    if (message.method === 'Runtime.exceptionThrown') errors.push(message.params.exceptionDetails.exception?.description ?? message.params.exceptionDetails.text);
  });
  await send('Runtime.enable');
  await until(() => evaluate('document.querySelector("main h1")?.textContent==="Главная"'));
  const startupMs = Math.round(performance.now() - born);
  const startup = sample();
  record('startup', { at: born, processes: [] }, startup);
  const info = await evaluate('window.__TAURI_INTERNALS__.invoke("app_info").then(({ version, admin })=>({version,admin}))');
  if (process.env.VELIORA_QA_RESTART === '1') {
    const config = await evaluate('window.__TAURI_INTERNALS__.invoke("config_get").then(c=>c.zapret)');
    if (!config.managedSites.some((site) => site.domain === 'example.net' && site.name === 'Проверочный сайт' && !site.enabled)) {
      throw new Error('Named site did not survive a native restart');
    }
    await evaluate('window.__TAURI_INTERNALS__.invoke("managed_sites_save",' + JSON.stringify({
      revision: config.managedRevision, sites: [], services: [],
    }) + ')');
    console.log('Native restart persistence verified');
  } else {
    await wait(5000);
    await measure('idle');
    await nav('catalog', 'Каталог обходов');
    await measure('catalog');
    const beforeLoop = sample();
    for (let i = 0; i < 12; i++) {
      await nav('home', 'Главная');
      await nav('catalog', 'Каталог обходов');
      await nav('settings', 'Настройки');
    }
    record('36 navigations', beforeLoop, sample());
    await nav('home', 'Главная');
    await wait(idleSeconds * 1000);
    await measure('after ' + idleSeconds + ' s');

    if (process.env.VELIORA_QA_VERIFY === '1') {
      const screenshot = async (name) => {
        if (!process.env.VELIORA_QA_SCREENSHOTS) return;
        await wait(440);
        const { data } = await send('Page.captureScreenshot', { format: 'png' });
        writeFileSync(join(process.env.VELIORA_QA_SCREENSHOTS, name + '.png'), Buffer.from(data, 'base64'));
      };
      const assert = (condition, message) => { if (!condition) throw new Error(message); };
      const click = async (label) => {
        const clicked = await evaluate('(()=>{const b=[...document.querySelectorAll("button")].find(e=>e.textContent.trim()==='
          + JSON.stringify(label) + '&&!e.disabled); if (!b) return false; b.click(); return true})()');
        assert(clicked, 'Button not found: ' + label);
      };
      assert(await evaluate('!document.querySelector(".manager-metric,.manager-events,.manager-chart,[data-route=sites],[data-route=stats]")'), 'Unused statistics or old navigation');
      assert(!(await evaluate('document.querySelectorAll(".splash-cursor__canvas").length')), 'WebGL on idle startup');
      await screenshot('veliora-228-home');
      await nav('catalog', 'Каталог обходов');
      await screenshot('veliora-228-catalog');
      const initial = await evaluate('window.__TAURI_INTERNALS__.invoke("config_get").then(c=>c.zapret)');
      assert(!(initial.managedSites?.length || initial.managedServices?.length), 'Use an empty isolated QA configuration');
      const fixtures = Array.from({ length: 120 }, (_, index) => ({ domain: 'site' + index + '.example.org', enabled: true, strategy: null }));
      fixtures.push({ domain: 'github.com', enabled: true, strategy: null });
      let result = await evaluate('window.__TAURI_INTERNALS__.invoke("managed_sites_save",' + JSON.stringify({
        revision: initial.managedRevision, sites: fixtures, services: [],
      }) + ')');
      await until(() => evaluate('document.querySelectorAll(".manager-site").length===48'));
      await click('Показать ещё');
      await until(() => evaluate('document.querySelectorAll(".manager-site").length===96'));
      await click('Готовые обходы');
      await until(() => evaluate('document.querySelectorAll(".manager-service").length===9&&!document.querySelector(".manager-site")'));
      await click('Общение');
      await until(() => evaluate('document.querySelectorAll(".manager-service").length===1&&document.querySelector(".manager-service h2").textContent==="Telegram"'));
      await click('Мои сайты');
      await until(() => evaluate('document.querySelectorAll(".manager-site").length===48'));
      await click('Добавить сайт');
      const setInput = (selector, value, type) => evaluate('(()=>{const el=document.querySelector(' + JSON.stringify(selector)
        + ');Object.getOwnPropertyDescriptor(' + type + '.prototype,"value").set.call(el,' + JSON.stringify(value)
        + ');el.dispatchEvent(new Event("input",{bubbles:true}))})()');
      await setInput('.manager-textarea', 'ftp://bad.example.org\nhttps://EXAMPLE.net/path?a=b\nexample.net', 'HTMLTextAreaElement');
      await until(() => evaluate('document.querySelector(".managed-preview")?.textContent.includes("Исправьте адреса: 1")'));
      assert(await evaluate('[...document.querySelectorAll("button")].find(b=>b.textContent.trim()==="Добавить в каталог").disabled'), 'Invalid domain was accepted');
      await setInput('.manager-textarea', 'https://EXAMPLE.net/path?a=b\nexample.net', 'HTMLTextAreaElement');
      await setInput('input[placeholder="Мой сайт"]', 'Проверочный сайт', 'HTMLInputElement');
      await until(() => evaluate('document.querySelector(".managed-preview")?.textContent.includes("Повторов в списке: 1")'));
      assert(await evaluate('!document.querySelector("[aria-label=\\"Стратегия сайта\\"]")'), 'Untested strategy was offered');
      await click('Добавить в каталог');
      await until(() => evaluate('!document.querySelector(".sheet textarea")'));
      await setInput('input[type=search]', 'Проверочный', 'HTMLInputElement');
      await until(() => evaluate('document.querySelector(".manager-site strong")?.textContent==="Проверочный сайт"'));
      await evaluate('document.querySelector("[aria-label=\\"Включить example.net\\"]").click()');
      await until(() => evaluate('document.querySelector("[aria-label=\\"Включить example.net\\"]")?.getAttribute("aria-checked")==="false"'));
      const saved = await evaluate('window.__TAURI_INTERNALS__.invoke("config_get").then(c=>c.zapret)');
      assert(saved.managedSites.some(s=>s.domain==='example.net'&&!s.enabled&&s.name==='Проверочный сайт'), 'Named site was not persisted');
      if (process.env.VELIORA_QA_KEEP !== '1') {
        result = await evaluate('window.__TAURI_INTERNALS__.invoke("managed_sites_save",' + JSON.stringify({
          revision: saved.managedRevision, sites: [], services: [],
        }) + ')');
        assert(!result.restarted, 'Inactive Zapret unexpectedly restarted');
      }
      await nav('settings', 'Настройки');
      await click('Журнал');
      await until(() => evaluate('document.querySelector("main h1")?.textContent==="Журнал" && !!document.querySelector(".log")'));
      if (process.env.VELIORA_QA_EFFECTS === '1') {
        await send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: 540, y: 160 });
        await until(() => evaluate('!!document.querySelector(".splash-cursor__canvas")&&document.querySelector(".backdrop").dataset.active==="true"'));
        await until(() => evaluate('!document.querySelector(".splash-cursor__canvas")&&document.querySelector(".backdrop").dataset.active==="false"'), 30000);
      }
      console.log(JSON.stringify({ verification: 'native catalog, 121 legacy entries, paging, validation, labels, switch, journal' }));
    }

    console.log(JSON.stringify({ info, startupMs, snapshots, errors }, null, 2));
  }
} finally {
  if (ws?.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({ id: ++sequence, method: 'Runtime.evaluate',
      params: { expression: 'void window.__TAURI_INTERNALS__.invoke("quit_app")' } }));
    await wait(500);
    ws.close();
  }
  if (child.exitCode === null) child.kill();
}
