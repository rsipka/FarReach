/**
 * System's light/dark theme pair in the real app, in a browser page with a
 * stand-in for Tauri.
 *
 * What it proves:
 *   - With Dracula picked by hand, choosing System keeps Dracula as the dark
 *     half of the pair and shows the two pickers.
 *   - The light half is picked from light themes only; the app follows the OS
 *     live, both ways, without a restart.
 *   - The pair is saved.
 *
 * Needs Playwright's Chromium; skips cleanly without it. Starts `vite dev`
 * on a free port. SHOT_DIR=<dir> saves screenshots.
 *
 * Run: node scripts/theme-pair-live-test.mjs
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
const page = await browser.newPage({ viewport: { width: 1200, height: 760 }, deviceScaleFactor: process.env.SHOT_DIR ? 2 : 1, colorScheme: 'dark' });
await page.addInitScript(() => {
	localStorage.setItem('reach-settings', JSON.stringify({ setupComplete: true, seenFeatures: ['devops', 'containers-k8s'], checkForUpdates: false, terminalBar: 'hidden', serverMonitoring: false, theme: 'dark', themeId: 'dracula' }));
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
		case 'session_list':
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

const bg = () => page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue('--color-bg-primary').trim());
const saved = () => page.evaluate(() => JSON.parse(localStorage.getItem('reach-settings') || '{}'));
async function shot(name) {
	if (!process.env.SHOT_DIR) return;
	await sleep(300);
	await page.screenshot({ path: `${process.env.SHOT_DIR}/${name}.png` }).catch(() => {});
}

try {
	await page.goto(`http://localhost:${port}/`);
	await page.waitForSelector('.tabbar', { timeout: 30000 });
	check('Dracula applies at start', await until(async () => (await bg()) === '#282a36'), await bg());

	await page.locator('.settings-btn').click();
	await page.locator('.menu-item', { hasText: 'Appearance' }).click();
	await page.locator('.theme-card', { hasText: 'System' }).click();

	console.log('\nSystem');
	check('the pair pickers show', await until(async () => (await page.locator('.theme-pair').count()) === 1));
	check('Dracula became the dark half', (await saved()).themeDark === 'dracula', JSON.stringify(await saved()));
	check('the OS is dark: Dracula stays', (await bg()) === '#282a36', await bg());
	check('no theme picked by hand any more', (await saved()).themeId === undefined && (await saved()).theme === 'system');

	console.log('\nthe light half');
	await page.locator('.pair-row').first().locator('.dropdown-trigger').click();
	const offered = await page.locator('.pair-row').first().locator('.dropdown-option, [role=option]').allInnerTexts();
	check('only light themes are offered', offered.length > 5 && offered.includes('Ocean Light') && !offered.includes('Ocean Dark'), offered.join(', '));
	await shot('pair-light-picker');
	await page.locator('.pair-row').first().getByText('Ocean Light', { exact: true }).click();
	check('saved', (await saved()).themeLight === 'ocean-light');
	check('still dark: the light half waits', (await bg()) === '#282a36', await bg());
	await shot('pair-dark-os');

	console.log('\nthe OS switches');
	await page.emulateMedia({ colorScheme: 'light' });
	check('to light: Ocean Light, live', await until(async () => (await bg()) === '#f2fbff', 3000), await bg());
	check('the root says light', await page.evaluate(() => document.documentElement.classList.contains('light')));
	await shot('pair-light-os');
	await page.emulateMedia({ colorScheme: 'dark' });
	check('and back to Dracula', await until(async () => (await bg()) === '#282a36', 3000), await bg());

	console.log('\npicking a theme by hand');
	await page.locator('.theme-card', { hasText: 'Reacher CM Light' }).click();
	check('overrides the OS', await until(async () => (await bg()) === '#ffffff'), await bg());
	check('the pickers hide', (await page.locator('.theme-pair').count()) === 0);
	check('the pair is kept for later', (await saved()).themeLight === 'ocean-light' && (await saved()).themeDark === 'dracula');

	console.log('\nthe colourful themes');
	await page.locator('.more-themes').click();
	for (const name of ['Ocean Dark', 'Forest Light', 'Sunset Dark', 'Amethyst Light', 'Cherry Dark', 'Neon Night']) {
		check(name + ' is offered', (await page.locator('.theme-card', { hasText: name }).count()) === 1);
	}
	await page.locator('.theme-card', { hasText: 'Neon Night' }).click();
	check('Neon Night applies', await until(async () => (await bg()) === '#0d0b1f'), await bg());
	await page.locator('.theme-card', { hasText: 'Neon Night' }).scrollIntoViewIfNeeded();
	await shot('colorful-neon');
	await page.locator('.theme-card', { hasText: 'Sunset Light' }).click();
	await shot('colorful-sunset-light');
} finally {
	await browser.close();
	vite.kill();
}

console.log(failures ? `\n${failures} failed` : '\nAll passed');
process.exit(failures ? 1 : 0);
