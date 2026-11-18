/**
 * Checks the bundled app themes (#lib/themes/bundled) in Node: every key is
 * there, ids are unique and usable as file names, and the colours people read
 * clear a contrast floor (WCAG ratios; translucent colours are laid over the
 * background first).
 *
 * Run: node scripts/app-themes-test.mjs
 */

import { readFileSync } from 'node:fs';
import { BUNDLED } from '../src/lib/themes/bundled.ts';

let failures = 0;
const check = (name, cond, extra = '') => {
	if (cond) console.log(`  PASS  ${name}`);
	else {
		failures++;
		console.log(`  FAIL  ${name}${extra ? '\n        ' + extra : ''}`);
	}
};

// The key lists live in theme.svelte.ts, which imports Tauri and so cannot
// load here: read them out of the source instead, so they cannot drift.
const src = readFileSync(new URL('../src/lib/state/theme.svelte.ts', import.meta.url), 'utf8');
const list = (name) => {
	const m = src.match(new RegExp(`export const ${name}[^=]*=\\s*\\[([^\\]]*)\\]`));
	return [...m[1].matchAll(/'([^']+)'/g)].map((x) => x[1]);
};
const COLOR_KEYS = list('COLOR_KEYS');
const TERMINAL_KEYS = list('TERMINAL_KEYS');
const builtInIds = ['reach-dark', 'reach-light'];

function rgba(c) {
	let m = c.match(/^#([0-9a-f]{6})$/i);
	if (m) {
		const n = parseInt(m[1], 16);
		return [(n >> 16) & 255, (n >> 8) & 255, n & 255, 1];
	}
	m = c.match(/^rgba?\(\s*(\d+),\s*(\d+),\s*(\d+)(?:,\s*([\d.]+))?\s*\)$/);
	if (m) return [+m[1], +m[2], +m[3], m[4] === undefined ? 1 : +m[4]];
	return null;
}
const over = (fg, bg) => fg.slice(0, 3).map((v, i) => v * fg[3] + bg[i] * (1 - fg[3]));
const lum = ([r, g, b]) => {
	const f = (v) => ((v /= 255) <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4);
	return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
};
function contrast(fg, bg) {
	const b = rgba(bg);
	const a = over(rgba(fg), b);
	const [hi, lo] = [lum(a), lum(b)].sort((x, y) => y - x);
	return (hi + 0.05) / (lo + 0.05);
}

console.log(`\n${BUNDLED.length} bundled themes`);
const ids = new Set();
for (const t of BUNDLED) {
	console.log(`\n${t.name}`);
	check('id is a safe file name', /^[A-Za-z0-9_-]+$/.test(t.id), t.id);
	check('id is unique', !ids.has(t.id) && !builtInIds.includes(t.id), t.id);
	ids.add(t.id);
	check('appearance', t.appearance === 'dark' || t.appearance === 'light');
	const missing = [
		...COLOR_KEYS.filter((k) => typeof t.colors[k] !== 'string'),
		...TERMINAL_KEYS.filter((k) => typeof t.terminal[k] !== 'string').map((k) => 'terminal.' + k)
	];
	check('every key set', missing.length === 0, missing.join(', '));
	const all = [...Object.entries(t.colors), ...Object.entries(t.terminal)];
	const bad = all.filter(([, v]) => !rgba(v)).map(([k, v]) => `${k}=${v}`);
	check('every colour parses', bad.length === 0, bad.join(', '));
	if (bad.length) continue;

	const c = t.colors;
	const bgs = ['bg-primary', 'bg-secondary', 'bg-elevated'];
	const floor = (fg, min, on = bgs) => {
		const worst = Math.min(...on.map((b) => contrast(c[fg], c[b])));
		check(`${fg} ≥ ${min}:1 (${worst.toFixed(2)})`, worst >= min);
	};
	// The high-contrast pair is there for people who need it: AAA throughout.
	const hc = t.id.includes('high-contrast');
	floor('text-primary', hc ? 15 : 6);
	floor('text-secondary', hc ? 7 : 4.5);
	floor('text-tertiary', hc ? 7 : 3);
	floor('accent', 3);
	for (const k of ['success', 'warning', 'danger']) floor(k, 3, ['bg-primary']);
	const fill = contrast('#ffffff', c['accent-fill']);
	check(`white on accent-fill ≥ 4.5:1 (${fill.toFixed(2)})`, fill >= 4.5);
	const term = contrast(t.terminal.foreground, t.terminal.background);
	check(`terminal text ≥ 4.5:1 (${term.toFixed(2)})`, term >= 4.5);
	const dark = lum(rgba(c['bg-primary'])) < 0.2;
	check('background matches appearance', dark === (t.appearance === 'dark'));
}

console.log(failures ? `\n${failures} failed` : '\nAll passed');
process.exit(failures ? 1 : 0);
