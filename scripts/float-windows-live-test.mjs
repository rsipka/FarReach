/**
 * Moves a terminal between two windows of the real app, in two browser
 * pages with a stand-in for Tauri: the main window and a floating one. The
 * backend's part (events to every window, emit_to one window, the payload
 * kept for a new window) is played by this script, as Tauri routes them.
 *
 * What it proves:
 *   - "Move to new window" opens a window showing the tab, and the main
 *     window lets it go without closing its shell.
 *   - Output streaming the whole time arrives once, in order, with nothing
 *     lost or repeated across the handover, scrollback included.
 *   - "Move all to the main window" brings it back and the floating window
 *     closes itself.
 *
 * Needs Playwright's Chromium; skips cleanly without it. Starts `vite dev`
 * on a free port.
 *
 * Run: node scripts/float-windows-live-test.mjs
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
const context = await browser.newContext({ viewport: { width: 1300, height: 820 } });
await context.addInitScript(() => {
	localStorage.setItem('reach-settings', JSON.stringify({ setupComplete: true, seenFeatures: [], checkForUpdates: false, terminalBar: 'hidden', serverMonitoring: false }));
});

/** Windows by label: their page, and their event listeners (id → { event, target }). */
const windows = new Map();
/** What each new window is to show, until it asks (float_window_take). */
const pending = new Map();
const calls = [];

/** Deliver an event as Tauri does: an `Any` listener hears everything; one bound to a window hears global events and those sent to it. */
async function deliver(event, payload, to) {
	for (const [label, w] of windows) {
		const ids = [];
		for (const [id, l] of w.listeners) {
			if (l.event !== event) continue;
			const any = l.target.kind === 'Any';
			const mine = l.target.label === label;
			if (any || to === undefined || (to === label && mine)) ids.push(id);
		}
		if (ids.length === 0) continue;
		await w.page.evaluate(({ ids, event, payload }) => {
			for (const id of ids) window.__mock.fire(id, { event, id, payload });
		}, { ids, event, payload }).catch(() => {});
	}
}

async function openWindow(label, url) {
	const page = await context.newPage();
	const w = { page, listeners: new Map(), closed: false };
	windows.set(label, w);
	page.on('pageerror', (e) => console.log(`  [${label}] page error: ${e.message}${process.env.DEBUG ? '\n' + e.stack : ''}`));
	if (process.env.DEBUG) page.on('console', (m) => console.log(`  [${label}] ${m.type()}: ${m.text()}`));
	await page.exposeBinding('__bridge', async (_src, msg) => {
		const { cmd, args } = msg;
		calls.push({ label, cmd, args });
		switch (cmd) {
			case 'plugin:event|listen':
				w.listeners.set(args.handler, { event: args.event, target: args.target });
				return args.handler;
			case 'plugin:event|unlisten':
				for (const [id, l] of w.listeners) if (l.event === args.event && id === args.eventId) w.listeners.delete(id);
				return null;
			case 'plugin:event|emit':
				void deliver(args.event, args.payload);
				return null;
			case 'plugin:event|emit_to':
				void deliver(args.event, args.payload, typeof args.target === 'string' ? args.target : args.target.label);
				return null;
			case 'float_window_open': {
				const l = `float-${windows.size + 1}`;
				pending.set(l, args.payload);
				void openWindow(l, `/?float=${l}`);
				return l;
			}
			case 'float_window_take': {
				const p = pending.get(label) ?? null;
				pending.delete(label);
				return p;
			}
			case 'float_window_list':
				return [...windows].filter(([, x]) => !x.closed).map(([l]) => ({ label: l, title: l }));
			case 'plugin:window|close':
				// As Tauri: the page hears it first, and may keep the window.
				void deliver('tauri://close-requested', null, label);
				return null;
			case 'plugin:window|destroy':
				w.closed = true;
				windows.delete(label);
				void page.close();
				return null;
			case 'pty_spawn':
				return args.id ?? null;
			case 'vault_has_identity':
			case 'credential_has_master_password':
				return true;
			case 'vault_is_locked':
			case 'vault_is_held':
				return false;
			case 'vault_auto_unlock':
				return true;
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
				return { recording: false, level: null, logPath: null };
			default:
				return null;
		}
	});
	await page.addInitScript((label) => {
		const callbacks = new Map();
		let next = 1;
		window.__mock = {
			fire(id, data) {
				callbacks.get(id)?.(data);
			}
		};
		window.__TAURI_INTERNALS__ = {
			metadata: { currentWindow: { label }, currentWebview: { windowLabel: label, label } },
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
	}, label);
	await page.goto(`http://localhost:${port}${url}`);
	return page;
}

async function shot(page, name) {
	if (!process.env.SHOT_DIR) return;
	await sleep(500);
	await page.screenshot({ path: `${process.env.SHOT_DIR}/${name}.png` }).catch(() => {});
}

/** The terminal text a window shows (DOM renderer: WebGL is off in this browser). */
async function screenText(page) {
	return page.evaluate(() => [...document.querySelectorAll('.terminal-wrapper.active .xterm-rows')].map((r) => r.innerText).join('\n'));
}

try {
	console.log('\nmain window');
	const main = await openWindow('main', '/');
	await main.waitForSelector('.tabbar', { timeout: 30000 });
	await main.locator('.new-tab-btn').last().click();
	const spawned = await until(() => calls.some((c) => c.cmd === 'pty_spawn'));
	if (process.env.DEBUG) console.log([...new Set(calls.map((c) => c.cmd))].join(' '));
	check('a local shell starts', spawned);
	const ptyId = calls.find((c) => c.cmd === 'pty_spawn')?.args.id;

	// A stream of numbered lines, the whole time, to every window.
	let n = 0;
	let streaming = true;
	const stream = (async () => {
		while (streaming) {
			n++;
			await deliver(`pty-data-${ptyId}`, `line ${n}\r\n`);
			// Slow enough that every line still fits on the screen at the end,
			// so the whole run (handover included) can be read off it.
			await sleep(70);
		}
	})();
	await until(async () => (await screenText(main)).includes('line 5'));
	check('output shows in the main window', (await screenText(main)).includes('line 5'));

	console.log('\nmove to a new window');
	await main.locator('.tabbar .tab').first().click({ button: 'right' });
	await shot(main, 'tab-menu');
	await main.getByText('Move to new window', { exact: true }).click();
	const opened = await until(() => windows.has('float-2'));
	check('a floating window opens', opened);
	const float = windows.get('float-2')?.page;
	const moved = await until(async () => (await main.locator('.tabbar .tab').count()) === 0 && (await float.locator('.tabbar .tab').count()) === 1, 20000);
	check('the tab is in the new window and gone from the main one', moved);
	check('the shell was not closed', !calls.some((c) => c.cmd === 'pty_close'));
	check('no second shell was started', calls.filter((c) => c.cmd === 'pty_spawn').length === 1);
	await sleep(200);
	streaming = false;
	await stream;
	await sleep(300);

	await shot(float, 'float-window');
	const text = await screenText(float);
	const seen = [...text.matchAll(/line (\d+)/g)].map((m) => Number(m[1]));
	const first = seen[0];
	const inOrder = seen.every((v, i) => i === 0 || v === seen[i - 1] + 1);
	check('every line arrives once, in order, across the handover', inOrder && first === 1 && seen[seen.length - 1] === n, `saw ${seen.length} lines ${first}..${seen[seen.length - 1]} of ${n}: ${seen.join(',')}`);
	check('the screen came with it (lines from before the move)', first === 1, `first line shown: ${first}`);

	console.log('\nmove back');
	await float.getByRole('button', { name: 'Move all tabs to the main window' }).click();
	const back = await until(async () => !windows.has('float-2') && (await main.locator('.tabbar .tab').count()) === 1, 20000);
	check('the tab is back in the main window and the floating one closed', back);
	check('the shell is still not closed', !calls.some((c) => c.cmd === 'pty_close'));
	const again = await screenText(main);
	check('its lines came back with it', again.includes(`line ${n}`), again.slice(-200));

	console.log('\nmove a split');
	await main.locator('.new-tab-btn').last().click();
	await until(async () => (await main.locator('.tabbar .tab').count()) === 2);
	await main.keyboard.press('Control+Shift+E');
	const split = await until(async () => (await main.locator('.terminal-wrapper.pane').count()) === 2);
	check('two tabs side by side in the main window', split);
	await main.locator('.tabbar .tab').first().click({ button: 'right' });
	await main.getByText('Move split to new window', { exact: true }).click();
	const splitMoved = await until(async () => {
		const f = [...windows.keys()].find((l) => l !== 'main');
		if (!f) return false;
		const pg = windows.get(f).page;
		return (await main.locator('.tabbar .tab').count()) === 0 && (await pg.locator('.terminal-wrapper.pane').count()) === 2;
	}, 20000);
	check('both panes moved together, still side by side', splitMoved);
	check('still no shell closed', !calls.some((c) => c.cmd === 'pty_close'));
	const fl = [...windows.keys()].find((l) => l !== 'main');
	if (fl) {
		await sleep(500);
		await shot(windows.get(fl).page, 'float-split');
		const pg = windows.get(fl).page;
		await pg.evaluate(() => document.querySelector('.window-btn-close')?.click());
		const asked = await until(() => pg.getByText('Close this window?').isVisible().catch(() => false), 3000);
		check('closing a window with sessions asks first', asked);
		await shot(pg, 'close-dialog');
	}
} finally {
	await browser.close();
	vite.kill();
}

console.log(`\n${failures === 0 ? 'All passed' : `${failures} failed`}`);
process.exit(failures === 0 ? 0 : 1);
