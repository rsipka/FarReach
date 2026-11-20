/**
 * System's light/dark pair (#lib/themes/resolve): which app theme is in use
 * for each mix of settings and OS appearance.
 *
 * Run: node --import ./scripts/resolve-lib.mjs scripts/theme-pair-test.mjs
 */

import { resolveTheme, pairTheme } from '#lib/themes/resolve.js';
import { BUNDLED } from '#lib/themes/bundled.js';

let failures = 0;
const check = (name, cond, extra = '') => {
	if (cond) console.log(`  PASS  ${name}`);
	else {
		failures++;
		console.log(`  FAIL  ${name}${extra ? '\n        ' + extra : ''}`);
	}
};

const LIGHT = { id: 'reach-light', appearance: 'light' };
const DARK = { id: 'reach-dark', appearance: 'dark' };
const all = [DARK, LIGHT, ...BUNDLED];
const fb = { light: LIGHT, dark: DARK };
const id = (s, dark) => resolveTheme(s, dark, all, fb).id;

console.log('\nSystem without a pair');
check('light OS: Reacher CM Light', id({ theme: 'system' }, false) === 'reach-light');
check('dark OS: Reacher CM Dark', id({ theme: 'system' }, true) === 'reach-dark');

console.log('\nSystem with a pair');
const pair = { theme: 'system', themeLight: 'ocean-light', themeDark: 'neon-night' };
check('light OS: the light theme', id(pair, false) === 'ocean-light');
check('dark OS: the dark theme', id(pair, true) === 'neon-night');
check('only one half set: the other is the default', id({ theme: 'system', themeDark: 'dracula' }, false) === 'reach-light');
check('a removed theme falls back', id({ theme: 'system', themeDark: 'gone' }, true) === 'reach-dark');

console.log('\nA theme picked by hand');
check('wins over the OS', id({ ...pair, themeId: 'gruvbox-light' }, true) === 'gruvbox-light');
check('a removed one falls back to theme', id({ theme: 'light', themeId: 'gone' }, true) === 'reach-light');
check('a removed one under System uses the pair', id({ ...pair, themeId: 'gone' }, true) === 'neon-night');

console.log('\nOlder settings');
check('dark', id({ theme: 'dark' }, false) === 'reach-dark');
check('light', id({ theme: 'light' }, true) === 'reach-light');
check('the pair is ignored outside System', id({ theme: 'dark', themeDark: 'dracula' }, true) === 'reach-dark');

console.log('\npairTheme');
check('light half', pairTheme(pair, false, all, fb).id === 'ocean-light');
check('dark half', pairTheme(pair, true, all, fb).id === 'neon-night');

console.log('\nEvery colourful family has both sides');
for (const fam of ['ocean', 'forest', 'sunset', 'amethyst', 'cherry']) {
	const d = all.find((t) => t.id === `${fam}-dark`);
	const l = all.find((t) => t.id === `${fam}-light`);
	check(fam, d?.appearance === 'dark' && l?.appearance === 'light');
}

console.log(failures ? `\n${failures} failed` : '\nAll passed');
process.exit(failures ? 1 : 0);
