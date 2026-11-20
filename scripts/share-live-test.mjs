/**
 * Runs a real share between two browser pages: offer, answer, DataChannel,
 * sealed frames both ways. `share-test.mjs` covers the pieces in Node, which
 * has no WebRTC; this covers the handshake the two people actually do.
 *
 * Two pages in one Chromium, no STUN, host candidates only. What it proves is
 * the logic around the handshake, not NAT traversal:
 *
 *   - Start then Join then Connect works, and messages flow both ways.
 *   - The host pasting its own code back is named as that, and the share
 *     still connects once the real reply is pasted.
 *   - A guest pasting a reply into Join is named as that.
 *   - The guest still connects when the host pastes the reply long after the
 *     guest made it. It used to give up after 30 seconds, before most people
 *     had finished copying the reply into a chat.
 *
 * Needs Playwright's Chromium; skips cleanly without it. The slow case waits
 * SHARE_LIVE_DELAY seconds (default 45, past the old limit); set 0 to skip it.
 *
 * Run: node scripts/share-live-test.mjs
 */

import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const ROOT = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const DELAY = Number(process.env.SHARE_LIVE_DELAY ?? 45);

let chromium;
try {
	({ chromium } = await import('playwright'));
} catch {
	try {
		// A global install is found by require (NODE_PATH), not by import.
		({ chromium } = createRequire(import.meta.url)('playwright'));
	} catch {
		console.log('SKIP: playwright is not installed');
		process.exit(0);
	}
}

const { build } = await import('esbuild');
const bundled = await build({
	stdin: {
		contents: `import { SharePeer } from './src/lib/share/peer.ts'; window.SharePeer = SharePeer;`,
		resolveDir: ROOT,
		loader: 'ts'
	},
	bundle: true,
	format: 'iife',
	write: false,
	logLevel: 'silent'
});
const bundle = bundled.outputFiles[0].text;

let failures = 0;
const check = (name, cond, extra = '') => {
	if (cond) console.log(`  PASS  ${name}`);
	else {
		failures++;
		console.log(`  FAIL  ${name}${extra ? '\n        ' + extra : ''}`);
	}
};
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

let browser;
try {
	// Real addresses in the candidates rather than mDNS names, which a
	// headless browser may not resolve.
	browser = await chromium.launch({ args: ['--disable-features=WebRtcHideLocalIpsWithMdns'] });
} catch (e) {
	console.log(`SKIP: Chromium would not start (${e.message.split('\n')[0]})`);
	process.exit(0);
}

/** A page with the peer module on it. A secure origin, for WebCrypto. */
async function side(role) {
	const page = await (await browser.newContext()).newPage();
	await page.route('http://localhost/', (r) => r.fulfill({ body: '<!doctype html><title>share</title>', contentType: 'text/html' }));
	await page.goto('http://localhost/');
	await page.addScriptTag({ content: bundle });
	await page.evaluate((role) => {
		window.states = [];
		window.failures = [];
		window.got = [];
		window.peer = new window.SharePeer([], role, {
			onstate: (s) => window.states.push(s),
			onfailure: (f) => window.failures.push(f),
			onmessage: (m) => window.got.push(m.type === 'out' || m.type === 'in' ? { type: m.type, text: new TextDecoder().decode(m.data) } : m)
		});
	}, role);
	return page;
}

const read = (page) => page.evaluate(() => ({ states: window.states, failures: window.failures, got: window.got }));

async function waitConnected(page, ms = 15_000) {
	const until = Date.now() + ms;
	while (Date.now() < until) {
		const { states } = await read(page);
		if (states.includes('connected') || states.includes('failed')) return states.includes('connected');
		await sleep(200);
	}
	return false;
}

async function pair({ delay = 0, ownCodeFirst = false } = {}) {
	const host = await side('host');
	const guest = await side('guest');
	const invite = await host.evaluate(() => window.peer.createOffer());
	const reply = await guest.evaluate((c) => window.peer.acceptOffer(c), invite);

	let ownCode = null;
	if (ownCodeFirst) {
		ownCode = await host.evaluate(async (c) => {
			try {
				await window.peer.acceptAnswer(c);
				return null;
			} catch (e) {
				return e.failure ?? { kind: 'thrown', detail: String(e) };
			}
		}, invite);
	}
	if (delay) await sleep(delay * 1000);
	await host.evaluate((c) => window.peer.acceptAnswer(c), reply);
	const hostUp = await waitConnected(host);
	const guestUp = await waitConnected(guest);
	return { host, guest, invite, reply, ownCode, hostUp, guestUp };
}

console.log('\nstart, join, connect');
{
	const { host, guest, invite, reply, hostUp, guestUp } = await pair();
	check('the host connects', hostUp);
	check('the guest connects', guestUp);
	await host.evaluate(() => window.peer.send({ type: 'out', data: new TextEncoder().encode('from host') }));
	await guest.evaluate(() => window.peer.send({ type: 'in', data: new TextEncoder().encode('from guest') }));
	await sleep(500);
	const h = await read(host);
	const g = await read(guest);
	check('host output reaches the guest', g.got.some((m) => m.text === 'from host'), JSON.stringify(g.got));
	check('guest input reaches the host', h.got.some((m) => m.text === 'from guest'), JSON.stringify(h.got));
	check('no failures on either side', h.failures.length === 0 && g.failures.length === 0, JSON.stringify([h.failures, g.failures]));

	// A second guest pasting the reply into Join, by mistake.
	const third = await side('guest');
	const wrong = await third.evaluate(async (c) => {
		try {
			await window.peer.acceptOffer(c);
			return null;
		} catch (e) {
			return e.failure ?? { kind: 'thrown' };
		}
	}, reply);
	check('a reply pasted into Join is named as a reply', wrong?.kind === 'bad-code' && wrong?.reason === 'reply-not-invite', JSON.stringify(wrong));
	check('and the invitation still joins there', !!(await third.evaluate((c) => window.peer.acceptOffer(c).then(() => true), invite)));
}

console.log('\nhost pastes its own code first');
{
	const { ownCode, hostUp, guestUp } = await pair({ ownCodeFirst: true });
	check('its own code is named as its own', ownCode?.kind === 'bad-code' && ownCode?.reason === 'own-code', JSON.stringify(ownCode));
	check('the real reply still connects', hostUp && guestUp);
}

if (DELAY > 0) {
	console.log(`\nthe host pastes the reply ${DELAY}s later`);
	const { host, guest, hostUp, guestUp } = await pair({ delay: DELAY });
	check('the host connects', hostUp, JSON.stringify(await read(host)));
	check('the guest waited and connects', guestUp, JSON.stringify(await read(guest)));
}

await browser.close();
console.log(`\n${failures === 0 ? 'ALL PASS' : failures + ' FAILURE(S)'}`);
process.exitCode = failures ? 1 : 0;
