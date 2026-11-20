/**
 * The quick command picker (Ctrl+Shift+R) in the real app, in a browser
 * page with a stand-in for Tauri: a local shell tab, a session history that
 * answers searches, and the keys of the picker.
 *
 * What it proves:
 *   - The key opens the picker over a terminal, listing each command once,
 *     newest first, with how often it ran.
 *   - Typing searches again; the arrows move; Enter pastes the chosen
 *     command into the tab (bracketed when the shell asks, no Enter).
 *   - Ctrl+Enter pastes and runs it; Esc closes without typing anything.
 *
 * Needs Playwright's Chromium; skips cleanly without it. Starts `vite dev`
 * on a free port.
 *
 * Run: node scripts/command-picker-live-test.mjs
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


const now = Date.now();
const HISTORY = [
	{ text: 'systemctl status nginx', at: now - 60_000, exit: 3 },
	{ text: 'git pull --rebase', at: now - 5 * 60_000, exit: 0 },
	{ text: 'systemctl status nginx', at: now - 2 * 3600_000, exit: 0 },
	{ text: 'journalctl -u nginx -f', at: now - 26 * 3600_000, exit: 0 },
	{ text: 'git pull --rebase', at: now - 3 * 86400_000, exit: 0 },
	{ text: 'df -h', at: now - 4 * 86400_000, exit: 0 }
].map((h, i) => ({
	kind: 'command', id: i + 1, run: i < 3 ? 1 : 2, at: h.at, text: h.text, cwd: '~', exit: h.exit, redacted: false,
	sessionId: null, name: 'web-01', host: 'web-01.example.com', protocol: 'ssh', device: 'laptop'
}));

const browser = await chromium.launch({ args: ['--disable-webgl', '--disable-3d-apis'] });
const page = await browser.newPage({ viewport: { width: 1200, height: 760 }, deviceScaleFactor: process.env.SHOT_DIR ? 2 : 1 });
await page.addInitScript(() => {
	localStorage.setItem('reach-settings', JSON.stringify({ setupComplete: true, seenFeatures: [], checkForUpdates: false, terminalBar: 'hidden', serverMonitoring: false }));
});
const calls = [];
page.on('pageerror', (e) => console.log(`  page error: ${e.message}`));
await page.exposeBinding('__bridge', async (_src, { cmd, args }) => {
	calls.push({ cmd, args });
	switch (cmd) {
		case 'plugin:event|listen':
			return args.handler;
		case 'pty_spawn':
			return args.id ?? null;
		case 'vault_has_identity':
		case 'credential_has_master_password':
		case 'vault_auto_unlock':
			return true;
		case 'vault_is_locked':
		case 'vault_is_held':
			return false;
		case 'history_search': {
			const q = (args.query.text ?? '').trim().toLowerCase();
			return HISTORY.filter((h) => !q || h.text.toLowerCase().includes(q));
		}
		case 'session_list':
		case 'session_list_folders':
		case 'vault_list':
		case 'vault_duplicates':
		case 'ssh_list_connections':
		case 'snippet_list':
		case 'theme_list_installed':
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

const written = () => calls.filter((c) => c.cmd === 'pty_write').map((c) => new TextDecoder().decode(new Uint8Array(c.args.data))).join('');
const rows = () => page.locator('.picker .row .cmd').allInnerTexts();
async function shot(name) {
	if (!process.env.SHOT_DIR) return;
	await sleep(300);
	await page.screenshot({ path: `${process.env.SHOT_DIR}/${name}.png` }).catch(() => {});
}

try {
	await page.goto(`http://localhost:${port}/`);
	await page.waitForSelector('.tabbar', { timeout: 30000 });
	await page.locator('.new-tab-btn').last().click();
	check('a local shell starts', await until(() => calls.some((c) => c.cmd === 'pty_spawn')));
	await page.locator('.terminal-wrapper.active .xterm').click();

	console.log('\nopen');
	await page.keyboard.press('Control+Shift+R');
	check('Ctrl+Shift+R opens the picker', await until(async () => (await rows()).length > 0, 5000));
	check('each command once, newest first', JSON.stringify(await rows()) === JSON.stringify(['systemctl status nginx', 'git pull --rebase', 'journalctl -u nginx -f', 'df -h']), JSON.stringify(await rows()));
	check('repeats are counted', (await page.locator('.picker .row').first().locator('.count').innerText()) === '×2');
	check('the search box has the keys', await page.locator('.picker input').evaluate((el) => el === document.activeElement));
	await shot('picker');

	console.log('\nfilter and paste');
	await page.keyboard.type('nginx');
	check('typing searches again', await until(async () => (await rows()).length === 2));
	await page.keyboard.press('ArrowDown');
	await shot('picker-filtered');
	await page.keyboard.press('Enter');
	check('Enter pastes the chosen command without Enter', await until(() => written() === 'journalctl -u nginx -f'), JSON.stringify(written()));
	check('the picker closes', (await page.locator('.picker').count()) === 0);

	console.log('\npaste and run');
	calls.length = 0;
	await page.keyboard.press('Control+Shift+R');
	await until(async () => (await rows()).length > 0, 5000);
	await page.keyboard.press('Control+Enter');
	check('Ctrl+Enter pastes and runs', await until(() => written() === 'systemctl status nginx\r'), JSON.stringify(written()));

	console.log('\nescape');
	calls.length = 0;
	await page.keyboard.press('Control+Shift+R');
	await until(async () => (await rows()).length > 0, 5000);
	await page.keyboard.press('Escape');
	await sleep(200);
	check('Esc closes and types nothing', (await page.locator('.picker').count()) === 0 && written() === '');

	console.log('\nthe terminal menu has it');
	await page.locator('.terminal-wrapper.active .xterm').click({ button: 'right' });
	await page.getByText('Command history…', { exact: true }).first().click();
	check('the right-click menu opens the picker', await until(async () => (await rows()).length > 0, 5000));
} finally {
	await browser.close();
	vite.kill();
}

console.log(failures ? `\n${failures} failed` : '\nall passed');
process.exit(failures ? 1 : 0);
