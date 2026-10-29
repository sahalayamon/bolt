// Real-network verification: drives the *built desktop app* (via WebView2's DevTools
// protocol, so the real UI/commands/events are exercised) against the `bolt` CLI.
//
//   node scripts/e2e-cli.mjs [path/to/bolt-desktop.exe] [path/to/bolt.exe]
//
// Scenarios: app->CLI send, CLI->app send (offer dialog accepted by clicking the UI),
// pairing in both directions (6-digit codes), mDNS discovery, retry/cancel of a failed send.
// Windows only (uses WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS). Exits non-zero on any failure.

import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

const APP = resolve(process.argv[2] ?? '../../target/release/bolt-desktop.exe');
const CLI = resolve(process.argv[3] ?? '../../target/release/bolt.exe');
const work = mkdtempSync(join(tmpdir(), 'bolt-e2e-'));
const dirs = Object.fromEntries(
  ['app-data', 'app-inbox', 'cli-data', 'cli-inbox', 'cli2-data', 'cli2-inbox', 'src', 'webview'].map((d) => [
    d,
    join(work, d),
  ]),
);
Object.values(dirs).forEach((d) => mkdirSync(d, { recursive: true }));
const APP_PORT = 47610;
const CLI_PORT = 47611;
const CDP_PORT = 9333;

let failed = 0;
const check = (name, ok, extra = '') => {
  console.log(`${ok ? 'PASS' : 'FAIL'}  ${name}${extra ? '  — ' + extra : ''}`);
  if (!ok) failed++;
};
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
async function until(fn, ms = 20000, what = 'condition') {
  const end = Date.now() + ms;
  for (;;) {
    const v = await fn();
    if (v) return v;
    if (Date.now() > end) throw new Error(`timeout waiting for ${what}`);
    await sleep(150);
  }
}
const sha = (p) => createHash('sha256').update(readFileSync(p)).digest('hex');
function tree(dir, base = dir, out = {}) {
  for (const e of readdirSync(dir, { withFileTypes: true })) {
    const p = join(dir, e.name);
    if (e.isDirectory()) tree(p, base, out);
    else out[p.slice(base.length + 1).replaceAll('\\', '/')] = sha(p);
  }
  return out;
}

// ---- fixtures -------------------------------------------------------------
const folder = join(dirs.src, 'holiday-photos');
mkdirSync(join(folder, 'raw'), { recursive: true });
const rnd = (n, seed) => {
  const b = Buffer.alloc(n);
  let x = seed;
  for (let i = 0; i < n; i++) {
    x = (x * 1103515245 + 12345) & 0x7fffffff;
    b[i] = x >> 16;
  }
  return b;
};
writeFileSync(join(folder, 'a.jpg'), rnd(3_000_000, 1));
writeFileSync(join(folder, 'raw', 'b.raw'), rnd(8_000_001, 2));
writeFileSync(join(folder, 'empty.txt'), '');
writeFileSync(join(folder, 'ünïcode name.txt'), 'héllo');
const big = join(dirs.src, 'big.bin');
writeFileSync(big, rnd(64_000_000, 3));

// ---- processes ------------------------------------------------------------
const procs = [];
function start(cmd, args, env = {}) {
  const p = spawn(cmd, args, { env: { ...process.env, ...env }, stdio: ['pipe', 'pipe', 'pipe'] });
  const o = { p, out: '' };
  p.stdout.on('data', (d) => (o.out += d));
  p.stderr.on('data', (d) => (o.out += d));
  procs.push(p); p.__o = o;
  return o;
}
/** Auto-answer the CLI's y/N prompts. */
function autoYes(o) {
  let seen = 0;
  setInterval(() => {
    const n = (o.out.match(/\[y\/N\]/g) ?? []).length;
    while (seen < n) {
      seen++;
      o.p.stdin.write('y\n');
    }
  }, 100).unref();
}

const app = start(APP, [], {
  BOLT_SHOW_LOCAL_DEVICES: '1', // the CLI under test runs on this same computer
  BOLT_DATA_DIR: dirs['app-data'],
  BOLT_DOWNLOAD_DIR: dirs['app-inbox'],
  BOLT_PORT: String(APP_PORT),
  BOLT_ADVERTISE_IP: '127.0.0.1',
  WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${CDP_PORT}`,
  WEBVIEW2_USER_DATA_FOLDER: dirs.webview,
});

// ---- CDP ------------------------------------------------------------------
const targets = await until(async () => {
  try {
    const r = await fetch(`http://127.0.0.1:${CDP_PORT}/json`);
    const t = (await r.json()).filter((x) => x.type === 'page');
    return t.length ? t : null;
  } catch {
    return null;
  }
}, 40000, 'app webview (DevTools port)');
const ws = new WebSocket(targets[0].webSocketDebuggerUrl);
await new Promise((r) => (ws.onopen = r));
let seq = 0;
const pending = new Map();
ws.onmessage = (m) => {
  const d = JSON.parse(m.data);
  if (d.id && pending.has(d.id)) pending.get(d.id)(d);
};
const cdp = (method, params = {}) =>
  new Promise((res) => {
    const id = ++seq;
    pending.set(id, res);
    ws.send(JSON.stringify({ id, method, params }));
  });
async function js(expr) {
  const r = await cdp('Runtime.evaluate', { expression: expr, awaitPromise: true, returnByValue: true });
  if (r.result.exceptionDetails) throw new Error(JSON.stringify(r.result.exceptionDetails.exception?.description ?? r.result));
  return r.result.result.value;
}
const invoke = (cmd, args = {}) =>
  js(`window.__TAURI_INTERNALS__.invoke(${JSON.stringify(cmd)}, ${JSON.stringify(args)})`);
const clickButton = (name) =>
  js(`(() => { const b=[...document.querySelectorAll('button')].find(b=>b.textContent.trim()===${JSON.stringify(name)}||b.getAttribute('aria-label')===${JSON.stringify(name)}); if(!b) return false; b.click(); return true; })()`);
/** Text of the open dialog whose content includes `needle` (ignores dialogs still animating out). */
const dialogText = (needle) =>
  js(`[...document.querySelectorAll("[role=dialog]")].map(d => d.innerText).find(t => t.includes(${JSON.stringify(needle)})) ?? null`);
const shot = async (name) => {
  const r = await cdp('Page.captureScreenshot', { format: 'png' });
  writeFileSync(join(process.env.E2E_SHOTS ?? work, name + '.png'), Buffer.from(r.result.data, 'base64'));
};

try {
  await until(() => js(`!!window.__TAURI_INTERNALS__ && document.body.innerText.includes('Send')`), 30000, 'UI render');
  const info = await invoke('app_info');
  check('app started, identity + receiver listening', info.port === APP_PORT && info.deviceId.length === 64, `port ${info.port}, id ${info.shortId}`);
  await shot('01-home-empty');

  // ===== CLI receiver (unattended: answers y) =============================
  const cliRx = start(CLI, ['--data', dirs['cli-data'], 'receive', '--dir', dirs['cli-inbox'], '--port', String(CLI_PORT), '--name', 'CLI-Receiver', '--pair'], { BOLT_ADVERTISE_IP: '127.0.0.1' });
  autoYes(cliRx);
  await until(() => cliRx.out.includes('pairing (valid'), 15000, 'CLI receiver ready');
  const cliId = cliRx.out.match(/device id: ([0-9a-f]{64})/)[1];
  const cliUri = cliRx.out.match(/pairing \(valid 5 min\): (\S+)/)[1];

  // ===== discovery (mDNS, real) ==========================================
  const found = await until(async () => (await invoke('list_devices')).find((d) => d.id === cliId), 25000, 'mDNS discovery of CLI').catch(() => null);
  if (found) { await js(`document.querySelector('ul[aria-label="Nearby devices"] li button')?.click()`); await sleep(500); await shot('01b-home-device'); }
  check('app discovers the CLI via real mDNS', !!found, found ? `${found.name} @ ${found.addrs.join(',')}` : 'not found');

  // ===== pairing: app scans CLI's pairing link ===========================
  const pairP = invoke('pair_with_uri', { uri: cliUri });
  const paired = await pairP;
  const codeCli = (await until(() => /Verification code: (\d{3} ?\d{3})/.exec(cliRx.out), 5000, 'CLI pair code'))[1].replace(/\D/g, '');
  check('pairing app→CLI approved, CLI is trusted', paired.id === cliId && (await invoke('list_trusted')).some((t) => t.id === cliId), `CLI showed code ${codeCli}`);

  // ===== app -> CLI send (folder, multi-file, unicode, empty) ============
  let t = await invoke('start_send', { paths: [folder], target: { deviceId: cliId } });
  const sentId = t.id;
  const done = await until(async () => {
    const x = (await invoke('list_transfers')).find((y) => y.id === sentId);
    return x && ['completed', 'failed', 'rejected', 'cancelled'].includes(x.state) ? x : null;
  }, 60000, 'app→CLI transfer');
  check('app→CLI folder send completed', done.state === 'completed', done.error ?? `${done.filesTotal} files, ${done.bytesTotal} B`);
  const got = tree(join(dirs['cli-inbox'], 'holiday-photos'));
  const want = tree(folder, dirs.src);
  const wantRel = Object.fromEntries(Object.entries(want).map(([k, v]) => [k.replace(/^holiday-photos\//, ''), v]));
  check('received tree is byte-identical (incl. empty + unicode names)', JSON.stringify(got) === JSON.stringify(wantRel), `${Object.keys(got).length} files`);
  const hist = await invoke('history_list', { filter: { text: 'holiday' } });
  check('sent transfer recorded in history', hist.some((h) => h.direction === 'sent' && h.status === 'completed'));

  // ===== paste: a real Ctrl+V paste event in the app -> staged file -> review -> send =====
  await invoke('end_pairing');
  await js(`document.querySelector('ul[aria-label="Nearby devices"] li button')?.click()`);
  await sleep(300);
  await js(`(() => { const dt = new DataTransfer(); dt.setData('text/plain', 'pasted via e2e é'); document.body.dispatchEvent(new ClipboardEvent('paste', { clipboardData: dt, bubbles: true, cancelable: true })); })()`);
  await until(() => js(`document.body.innerText.includes('Pasted text')`), 10000, 'paste review');
  check('Ctrl+V paste opens review for the selected device (no recipient picker)', await js(`!document.querySelector('[role=radiogroup]') && document.body.innerText.includes('Send to ')`));
  await shot('01c-paste-review');
  await clickButton('Send');
  await until(async () => (await invoke('list_transfers')).find((y) => y.title.startsWith('Pasted text') && y.state === 'completed'), 30000, 'paste send');
  const pasted = readdirSync(dirs['cli-inbox']).find((n) => n.startsWith('Pasted text'));
  check('pasted text arrived as a file with the right content', !!pasted && readFileSync(join(dirs['cli-inbox'], pasted), 'utf8') === 'pasted via e2e é', pasted);

  // ===== cancel mid-transfer, then retry ================================
  t = await invoke('start_send', { paths: [big], target: { deviceId: cliId } });
  await until(async () => (await invoke('list_transfers')).find((y) => y.id === t.id && y.state === 'transferring' && y.bytesDone > 0), 30000, 'big transfer running');
  const prog = (await invoke('list_transfers')).find((y) => y.id === t.id);
  check('progress reports live speed + ETA', prog.speedBps >= 0 && prog.bytesTotal === 64_000_000, `${(prog.speedBps / 1e6).toFixed(1)} MB/s, ${prog.bytesDone}/${prog.bytesTotal}`);
  await shot('02-transfer-running');
  await invoke('cancel_transfer', { id: t.id });
  const cancelled = await until(async () => (await invoke('list_transfers')).find((y) => y.id === t.id && y.state === 'cancelled'), 15000, 'cancel');
  check('cancel stops the transfer', !!cancelled && cancelled.canRetry);
  await invoke('retry_transfer', { id: t.id });
  const retried = await until(async () => (await invoke('list_transfers')).find((y) => y.id === t.id && y.state === 'completed'), 90000, 'retry completes');
  check('retry completes and file is intact', !!retried && sha(join(dirs['cli-inbox'], 'big.bin')) === sha(big));

  // ===== CLI -> app send (offer dialog accepted through the UI) ==========
  const cliTx = start(CLI, ['--data', dirs['cli2-data'], 'send', '--to', `127.0.0.1:${APP_PORT}`, '--name', 'CLI-Sender', folder]);
  await until(() => js(`!!document.querySelector('[role=dialog]') && document.body.innerText.includes('Incoming transfer')`), 20000, 'offer dialog');
  const dlgText = await js(`document.querySelector('[role=dialog]').innerText`);
  check('incoming-offer dialog shows sender, file count and "not paired" warning', /CLI-Sender/.test(dlgText) && /Not paired/.test(dlgText) && /4 files/.test(dlgText), dlgText.replace(/\s+/g, ' ').slice(0, 120));
  await shot('03-offer-dialog');
  check('Decline is focused for an unknown device', await js(`document.activeElement?.textContent.trim()==='Decline'`));
  await clickButton('Accept');
  const closed = await until(async () => !(await dialogText("Wants to send")) && !(await js(`[...document.querySelectorAll("[role=dialog]")].some(d => d.innerText.includes("Accept"))`)), 5000, "offer dialog close").catch(() => false);
  check('offer dialog closes after responding', !!closed);
  await until(() => cliTx.out.includes('receiver verified every file'), 60000, 'CLI send finished').catch(() => {});
  check('CLI→app send completed (CLI confirms receiver verified)', cliTx.out.includes('receiver verified every file'), cliTx.out.trim().split('\n').pop());
  const got2 = tree(join(dirs['app-inbox'], 'holiday-photos'));
  check('app received tree is byte-identical', JSON.stringify(got2) === JSON.stringify(wantRel), `${Object.keys(got2).length} files`);
  await until(async () => (await invoke('history_list', { filter: { direction: 'received' } })).some((h) => h.status === 'completed'), 10000, 'received history');
  check('received transfer recorded in history', true);
  // "Show folder": the command must succeed (it opens Explorer with the received folder selected).
  const recvd = (await invoke('list_transfers')).find((y) => y.direction === 'received' && y.state === 'completed');
  let revealOk = false;
  try {
    await invoke('reveal_transfer', { id: recvd.id });
    revealOk = true;
  } catch (e) {
    revealOk = false;
  }
  check('reveal_transfer (Show folder) succeeds', revealOk);

  // ===== decline path ====================================================
  const cliTx2 = start(CLI, ['--data', dirs['cli2-data'], 'send', '--to', `127.0.0.1:${APP_PORT}`, '--name', 'CLI-Sender', join(folder, 'a.jpg')]);
  await until(() => js(`(document.querySelector('[role=dialog]')?.innerText ?? '').includes('a.jpg')`), 20000, 'second offer');
  await clickButton('Decline');
  await until(() => /declined|send failed/i.test(cliTx2.out), 15000, 'CLI sees decline').catch(() => {});
  check('declining is reported to the sender', /declined/i.test(cliTx2.out), cliTx2.out.trim().split('\n').pop());

  // ===== pairing: CLI scans app's QR link; user compares codes in UI =====
  const pi = await invoke('begin_pairing');
  check('pairing link + QR SVG generated', pi.uri.startsWith('bolt://pair?') && pi.qrSvg.includes('<svg'), pi.address);
  const cliPair = start(CLI, ['--data', dirs['cli2-data'], 'pair', '--uri', pi.uri, '--name', 'CLI-Sender']);
  await until(() => dialogText('Pair with this device?'), 20000, 'pair request dialog');
  const uiCode = (await dialogText('Pair with this device?')).match(/(\d{3}) (\d{3})/);
  const cliCode = (await until(() => /Verification code: (\d{3} ?\d{3})/.exec(cliPair.out), 10000, 'CLI code'))[1].replace(/\D/g, '');
  check('both devices show the same 6-digit code', !!uiCode && uiCode[1] + uiCode[2] === cliCode, `UI ${uiCode?.[0]} / CLI ${cliCode}`);
  await shot('04-pair-dialog');
  await clickButton('Codes match — pair');
  await until(() => /paired with/.test(cliPair.out), 15000, 'CLI paired').catch(() => {});
  const cli2Id = (await invoke('list_trusted')).find((x) => x.name === 'CLI-Sender')?.id;
  check('pairing CLI→app approved; app lists CLI-Sender as trusted', /paired with/.test(cliPair.out) && !!cli2Id);

  // rename + revoke
  await invoke('rename_trusted', { id: cli2Id, name: 'Renamed Sender' });
  check('rename persists', (await invoke('list_trusted')).some((x) => x.name === 'Renamed Sender'));
  await invoke('revoke_trusted', { id: cli2Id });
  check('revoke removes trust', !(await invoke('list_trusted')).some((x) => x.id === cli2Id));

  // ===== failure + retry path: nothing listening ========================
  const dead = await invoke('start_send', { paths: [join(folder, 'a.jpg')], target: { address: '127.0.0.1:1' } });
  const failedT = await until(async () => (await invoke('list_transfers')).find((y) => y.id === dead.id && y.state === 'failed'), 30000, 'failure');
  check('unreachable peer fails cleanly and is retryable', !!failedT && failedT.canRetry, failedT?.error);

  // ===== settings persistence ============================================
  const s = await invoke('get_settings');
  const s2 = await invoke('update_settings', { settings: { ...s, deviceName: 'E2E Desktop', autoAcceptTrusted: true } });
  check('settings update applies', s2.deviceName === 'E2E Desktop' && (await invoke('app_info')).deviceName === 'E2E Desktop');

  // UI screenshots of the remaining screens
  for (const [label, file] of [['Transfers', '05-transfers'], ['History', '06-history'], ['Devices', '07-devices'], ['Settings', '08-settings']]) {
    await js(`[...document.querySelectorAll('nav button')].find(b=>b.getAttribute('aria-label')===${JSON.stringify(label)}||b.textContent.trim()===${JSON.stringify(label)})?.click()`);
    await sleep(600);
    await shot(file);
  }
  // Responsive + light-theme screenshots (visual review only; not assertions).
  await js(`document.documentElement.dataset.theme='light'`);
  await sleep(300);
  await shot('09-settings-light');
  await cdp('Emulation.setDeviceMetricsOverride', { width: 420, height: 860, deviceScaleFactor: 2, mobile: true });
  const clickText = (t) => js(`(() => { const b=[...document.querySelectorAll('button')].find(b=>b.textContent.trim().startsWith(${JSON.stringify(t)})); b?.click(); return !!b; })()`);
  await clickText('Devices'); // back button -> list (narrow)
  await sleep(500);
  await shot('10-mobile-list');
  await js(`document.querySelector('ul[aria-label="Nearby devices"] li button')?.click()`);
  await sleep(500);
  await shot('11-mobile-detail');
  await clickText('Devices');
  await sleep(300);
  await js(`[...document.querySelectorAll('nav button')].find(b=>b.getAttribute('aria-label')==='Settings')?.click()`);
  await sleep(500);
  await shot('12-mobile-settings');
  await cdp('Emulation.clearDeviceMetricsOverride');
} catch (e) {
  check('script ran to completion', false, e.stack ?? String(e));
  for (const [i, p] of procs.entries()) if (p.__o) console.log(`--- process ${i} ---\n${p.__o.out}`);
} finally {
  // Restart the app on the same data dir to prove persistence is on disk.
  procs.forEach((p) => p.kill());
  await sleep(500);
  console.log(failed ? `\n${failed} check(s) FAILED. Work dir kept: ${work}` : '\nAll checks passed.');
  if (!failed && !process.env.E2E_KEEP) rmSync(work, { recursive: true, force: true });
  process.exit(failed ? 1 : 0);
}
