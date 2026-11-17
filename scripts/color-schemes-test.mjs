/**
 * Tests reading terminal colour schemes (#lib/terminal/color-schemes) and
 * the bundled set, in Node.
 *
 * Run: node scripts/color-schemes-test.mjs
 */

import { readFileSync } from 'node:fs';
import {
	PALETTE_ORDER,
	isLightPalette,
	isSafeSchemeName,
	parseItermColors,
	parseSchemeFile,
	unpackPalette
} from '../src/lib/terminal/color-schemes.ts';

let failures = 0;
const check = (name, cond, extra = '') => {
	if (cond) console.log(`  PASS  ${name}`);
	else {
		failures++;
		console.log(`  FAIL  ${name}${extra ? '\n        ' + extra : ''}`);
	}
};
const section = (s) => console.log(`\n${s}`);

const colour = (key, r, g, b) => `
	<key>${key}</key>
	<dict>
		<key>Blue Component</key>
		<real>${b}</real>
		<key>Color Space</key>
		<string>sRGB</string>
		<key>Green Component</key>
		<real>${g}</real>
		<key>Red Component</key>
		<integer>${r}</integer>
	</dict>`;

// Homebrew's colours, as Terminal.app and iTerm2 have them.
const ansi = [
	[0, 0, 0], [0.6, 0, 0], [0, 0.65098, 0], [0.6, 0.6, 0], [0, 0, 0.698], [0.698, 0, 0.698], [0, 0.65098, 0.698], [0.74902, 0.74902, 0.74902],
	[0.4, 0.4, 0.4], [0.898, 0, 0], [0, 0.85098, 0], [0.898, 0.898, 0], [0, 0, 1], [0.898, 0, 0.898], [0, 0.898, 0.898], [0.898, 0.898, 0.898]
];
const homebrew = `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>${ansi.map((c, i) => colour(`Ansi ${i} Color`, ...c)).join('')}
${colour('Background Color', 0, 0, 0)}
${colour('Foreground Color', 0, 1, 0)}
${colour('Cursor Color', 0.13725, 1, 0.09412)}
${colour('Link Color', 0, 0, 1)}
</dict>
</plist>`;

section('.itermcolors');
{
	const p = parseItermColors(homebrew);
	check('every colour is read', PALETTE_ORDER.every((k) => /^#[0-9a-f]{6}$/.test(p[k])));
	check('background and text', p.background === '#000000' && p.foreground === '#00ff00', JSON.stringify(p));
	check('ANSI 1 is red', p.red === '#990000', p.red);
	check('ANSI 12 is bright blue', p.brightBlue === '#0000ff', p.brightBlue);
	check('the cursor', p.cursor === '#23ff18', p.cursor);
	check('a missing cursor text colour is the background', p.cursorAccent === '#000000');
	check('a missing selection is bright black', p.selectionBackground === '#666666');
	check('a missing selected text colour is the text colour', p.selectionForeground === '#00ff00');
	check('only the palette keys', Object.keys(p).join() === PALETTE_ORDER.join());
	check('dark', !isLightPalette(p));
	let threw = false;
	try {
		parseItermColors('<plist><dict></dict></plist>');
	} catch {
		threw = true;
	}
	check('a plist without colours is refused', threw);
	threw = false;
	try {
		parseItermColors('hello');
	} catch {
		threw = true;
	}
	check('something else is refused', threw);
}

section('scheme files');
{
	const [one] = parseSchemeFile(homebrew, 'C:\\Users\\me\\Downloads\\Homebrew.itermcolors');
	check('an .itermcolors file is named after the file', one.name === 'Homebrew', one.name);
	const wt = {
		name: 'Campbell Light',
		black: '#0C0C0C', red: '#C50F1F', green: '#13A10E', yellow: '#C19C00', blue: '#0037DA', purple: '#881798', cyan: '#3A96DD', white: '#CCCCCC',
		brightBlack: '#767676', brightRed: '#E74856', brightGreen: '#16C60C', brightYellow: '#F9F1A5', brightBlue: '#3B78FF', brightPurple: '#B4009E', brightCyan: '#61D6D6', brightWhite: '#F2F2F2',
		background: '#F2F2F2', foreground: '#0C0C0C', cursorColor: '#0C0C0C', selectionBackground: '#AAAAAA'
	};
	const [w] = parseSchemeFile(JSON.stringify(wt), 'x.json');
	check('a Windows Terminal scheme keeps its name', w.name === 'Campbell Light');
	check('purple is magenta', w.palette.magenta === '#881798' && w.palette.brightMagenta === '#b4009e');
	check('cursorColor is the cursor', w.palette.cursor === '#0c0c0c');
	check('light', isLightPalette(w.palette));
	const all = parseSchemeFile(JSON.stringify({ profiles: {}, schemes: [wt, { ...wt, name: 'Two' }] }), 'settings.json');
	check('a settings.json gives every scheme', all.length === 2 && all[1].name === 'Two');
	let threw = false;
	try {
		parseSchemeFile(JSON.stringify({ name: 'half', black: '#000000' }), 'half.json');
	} catch {
		threw = true;
	}
	check('a scheme missing colours is refused', threw);
}

section('bundled schemes');
{
	const data = JSON.parse(readFileSync(new URL('../src/lib/data/terminal-schemes.json', import.meta.url), 'utf8'));
	let bad = [];
	for (const [name, packed] of data.schemes) {
		try {
			unpackPalette(packed);
		} catch {
			bad.push(name);
		}
	}
	check('every bundled scheme unpacks', bad.length === 0, bad.join(', '));
	check('more than 300 are bundled', data.schemes.length > 300, String(data.schemes.length));
	const names = data.schemes.map(([n]) => n);
	check('names are unique', new Set(names).size === names.length);
	check('Homebrew is offered as a download', data.more.includes('Homebrew'));
	check('every downloadable name is safe', data.more.every(isSafeSchemeName));
	check('a path is not a scheme name', !isSafeSchemeName('../x') && !isSafeSchemeName('a/b') && !isSafeSchemeName(''));
}

console.log(failures ? `\n${failures} failed` : '\nAll passed');
process.exit(failures ? 1 : 0);
