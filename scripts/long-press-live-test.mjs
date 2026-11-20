/**
 * Long press on a session on a phone opens its menu as a bottom sheet, in a
 * browser page with a stand-in for Tauri (Pixel 7 emulation, real touch).
 *
 * What it proves:
 *   - Holding a finger on a session opens the same menu as a right click,
 *     with Connect via (other addresses) and Connect with (credentials).
 *   - Lifting the finger does not also connect.
 *   - A press that moves (a scroll) opens nothing; a short tap connects.
 *
 * Needs Playwright's Chromium; skips cleanly without it. SHOT_DIR=<dir>
 * saves screenshots.
 *
 * Run: node scripts/long-press-live-test.mjs
 */

import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { spawn } from 'node:child_process';
import net from 'node:net';
import path from 'node:path';

const ROOT = path.dirname(path.dirname(fileURLToPath(import.meta.url)));

let chromium;
try {
	({ chromium } = await import('playwright'));
} catch {
	try {
		({ chromium } = createRequire(import.meta.url)('playwright'));
	} catch {
		console.log('SKIP: playwright is not installed');
		process.exit(0);
	}
}

let failures = 0;
const check = (name, cond, extra = '') => {
	if (cond) console.log(`  PASS  ${name}`);
	else {
		failures++;
		console.log(`  FAIL  ${name}${extra ? '\n        ' + extra : ''}`);
	}
};
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
async function until(fn, ms = 15000, step = 50) {
	const end = Date.now() + ms;
	while (Date.now() < end) {
		if (await fn()) return true;
		await sleep(step);
	}
	return false;
}

const port = await new Promise((resolve) => {
	const s = net.createServer().listen(0, () => {
		const p = s.address().port;
		s.close(() => resolve(p));
	});
});
const vite = spawn('npx', ['vite', 'dev', '--port', String(port), '--strictPort'], { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] });
let viteOut = '';
vite.stdout.on('data', (d) => (viteOut += d));
vite.stderr.on('data', (d) => (viteOut += d));
const up = await until(async () => {
	try {
		return (await fetch(`http://localhost:${port}/`)).ok;
	} catch {
		return false;
	}
}, 60000, 250);
if (!up) {
	console.log('FAIL: vite did not start\n' + viteOut);
	vite.kill();
	process.exit(1);
}



const browser = await chromium.launch({ args: ['--disable-webgl', '--disable-3d-apis'] });
const { devices } = await import('playwright').catch(() => createRequire(import.meta.url)('playwright'));
const ctx = await browser.newContext({ ...devices['Pixel 7'], colorScheme: 'dark', ...(process.env.SHOT_DIR ? {} : { deviceScaleFactor: 1 }) });
const page = await ctx.newPage();
await page.addInitScript(() => {
	localStorage.setItem('reach-settings', JSON.stringify({ setupComplete: true, seenFeatures: ['devops', 'containers-k8s'], checkForUpdates: false, terminalBar: 'hidden', serverMonitoring: false, theme: 'dark' }));
});
const auth = { type: 'Password', password: 'x' };
const SESSIONS = [
	{ id: 's1', name: 'web-01', host: '10.0.0.10', port: 22, username: 'admin', auth_method: auth, folder_id: null, tags: [], alt_addresses: [{ label: 'External', host: 'web.example.com', port: 22 }] },
	{ id: 's2', name: 'db-01', host: '10.0.0.20', port: 22, username: 'admin', auth_method: auth, folder_id: null, tags: [] }
];
const CREDS = [
	{ id: 'c1', name: 'Ops', username: 'ops', auth_method: auth },
	{ id: 'c2', name: 'Backup', username: 'backup', auth_method: auth }
];
const calls = [];
page.on('pageerror', (e) => console.log(`  page error: ${e.message}`));
await page.exposeBinding('__bridge', async (_src, { cmd, args }) => {
	calls.push({ cmd, args });
	switch (cmd) {
		case 'plugin:event|listen':
			return args.handler;
		case 'vault_has_identity':
		case 'credential_has_master_password':
		case 'vault_auto_unlock':
			return true;
		case 'vault_is_locked':
		case 'vault_is_held':
			return false;
		case 'session_list':
			return SESSIONS;
		case 'saved_credential_list':
			return CREDS;
		case 'session_list_folders':
		case 'vault_list':
		case 'vault_duplicates':
		case 'ssh_list_connections':
		case 'snippet_list':
		case 'theme_list_installed':
		case 'theme_fetch_registry':
		case 'plugin:window|available_monitors':
			return [];
		case 'share_load':
		case 'share_get_config':
			return { enabled: false, iceServers: [] };
		case 'mcp_status':
			return { enabled: false, running: false, port: null, shared: [] };
		case 'history_start':
			return { recording: 'commands', paused: false, logFile: null };
		default:
			return null;
	}
});
await page.addInitScript(() => {
	const callbacks = new Map();
	let next = 1;
	window.__TAURI_INTERNALS__ = {
		metadata: { currentWindow: { label: 'main' }, currentWebview: { windowLabel: 'main', label: 'main' } },
		transformCallback(cb, once) {
			const id = next++;
			callbacks.set(id, (d) => {
				if (once) callbacks.delete(id);
				cb(d);
			});
			return id;
		},
		unregisterCallback(id) {
			callbacks.delete(id);
		},
		invoke: (cmd, args) => window.__bridge({ cmd, args: args ?? {} }),
		convertFileSrc: (p) => p
	};
	window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
});

const cdp = await ctx.newCDPSession(page);
async function press(locator, ms, moveBy = 0) {
	const b = await locator.boundingBox();
	const x = b.x + b.width / 2;
	const y = b.y + b.height / 2;
	await cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x, y }] });
	if (moveBy) {
		await sleep(100);
		await cdp.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: [{ x, y: y + moveBy }] });
	}
	await sleep(ms);
	await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
	await sleep(250);
}
const connects = () => calls.filter((c) => /^(ssh_connect|pty_spawn|ssh_probe_addresses)/.test(c.cmd)).length;
const menuOpen = () => page.locator('.context-menu.context-sheet').count();
async function shot(name) {
	if (!process.env.SHOT_DIR) return;
	await sleep(300);
	await page.screenshot({ path: `${process.env.SHOT_DIR}/${name}.png` }).catch(() => {});
}

try {
	await page.goto(`http://localhost:${port}/`);
	await page.waitForSelector('.tabbar', { timeout: 30000 });
	// The phone layout opens the sessions list from the sidebar button.
	if ((await page.locator('.session-card').count()) === 0) {
		const btn = page.locator('[aria-label="Sessions"], .sessions-btn, button[title="Sessions"]').first();
		if (await btn.count()) await btn.click();
	}
	const card = page.locator('.session-card', { hasText: 'web-01' }).locator('.card-main');
	check('the session list shows', await until(async () => (await card.count()) === 1), String(await page.locator('.session-card').count()));
	await shot('list');

	console.log('\nlong press');
	const before = connects();
	await press(card, 800);
	check('opens the session menu as a bottom sheet', (await menuOpen()) === 1);
	const text = (await page.locator('.context-menu').innerText()).replace(/\s+/g, ' ');
	check('titled with the session', (await page.locator('.context-sheet-title').innerText()) === 'web-01');
	check('offers Connect via its addresses', /web\.example\.com|External/.test(text), text);
	check('offers Connect with a credential', text.includes('Ops') && text.includes('Backup'), text);
	check('lifting the finger did not connect', connects() === before, JSON.stringify(calls.slice(-5).map((c) => c.cmd)));
	await shot('sheet');
	const sheet = await page.locator('.context-menu').boundingBox();
	const vp = page.viewportSize();
	check('sits along the bottom edge', Math.abs(sheet.y + sheet.height - vp.height) < 2 && sheet.width >= vp.width - 1, JSON.stringify(sheet));

	console.log('\nclosing');
	await page.mouse.click(vp.width / 2, 40);
	check('a tap outside closes it', await until(async () => (await menuOpen()) === 0, 2000));

	console.log('\nshort tap and scroll');
	await press(card, 120, 30);
	check('a press that moves (a scroll) opens nothing', (await menuOpen()) === 0);
	await sleep(500);
	const beforeTap = calls.length;
	await card.tap();
	await sleep(400);
	check('a short tap still connects, no menu', (await menuOpen()) === 0 && calls.length > beforeTap, JSON.stringify(calls.slice(beforeTap).map((c) => c.cmd)));
} finally {
	await browser.close();
	vite.kill();
}
console.log(failures ? `\n${failures} failed` : '\nall passed');
process.exit(failures ? 1 : 0);
