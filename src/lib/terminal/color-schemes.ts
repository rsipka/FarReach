/**
 * Terminal colour schemes as data: the bundled set's packed form, and the
 * files people bring in (iTerm2's .itermcolors, a Windows Terminal scheme).
 *
 * No Svelte and no DOM here, so `scripts/color-schemes-test.mjs` runs it in
 * Node. The palette's shape is TerminalPalette from #lib/state/theme.
 */

import type { TerminalPalette } from '#lib/state/theme.svelte.js';

/**
 * The palette's keys in the order the bundled schemes store their colours
 * (scripts/gen-terminal-schemes.py writes them in this order).
 */
export const PALETTE_ORDER: (keyof TerminalPalette)[] = [
	'background',
	'foreground',
	'cursor',
	'cursorAccent',
	'selectionBackground',
	'selectionForeground',
	'black',
	'red',
	'green',
	'yellow',
	'blue',
	'magenta',
	'cyan',
	'white',
	'brightBlack',
	'brightRed',
	'brightGreen',
	'brightYellow',
	'brightBlue',
	'brightMagenta',
	'brightCyan',
	'brightWhite'
];

/** A scheme read from a file: its name and colours. */
export interface ParsedScheme {
	name: string;
	palette: TerminalPalette;
}

const HEX = /^#[0-9a-f]{6}$/i;

/** A bundled scheme's colours, stored as 22 hex colours in PALETTE_ORDER. */
export function unpackPalette(packed: string): TerminalPalette {
	const colours = packed.trim().split(/\s+/);
	if (colours.length !== PALETTE_ORDER.length || !colours.every((c) => HEX.test(c))) {
		throw new Error('Not a packed palette');
	}
	const out = {} as TerminalPalette;
	PALETTE_ORDER.forEach((k, i) => (out[k] = colours[i].toLowerCase()));
	return out;
}

/** Whether a colour is a #rrggbb this app can store. */
export function isHexColour(value: unknown): value is string {
	return typeof value === 'string' && HEX.test(value);
}

/** Relative luminance (WCAG) of a #rrggbb colour, 0 black to 1 white. */
export function luminance(hex: string): number {
	const c = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255);
	const [r, g, b] = c.map((v) => (v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4));
	return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** A light scheme: dark text on a light background. */
export function isLightPalette(p: TerminalPalette): boolean {
	return isHexColour(p.background) && luminance(p.background) > 0.4;
}

const hex2 = (v: number) =>
	Math.round(Math.max(0, Math.min(1, v)) * 255)
		.toString(16)
		.padStart(2, '0');

/** The colours a scheme file may leave out, from the ones it has. */
function complete(p: Partial<TerminalPalette>): TerminalPalette {
	const required: (keyof TerminalPalette)[] = ['background', 'foreground'];
	for (let i = 6; i < PALETTE_ORDER.length; i++) required.push(PALETTE_ORDER[i]);
	const missing = required.find((k) => !isHexColour(p[k]));
	if (missing) throw new Error(`The scheme has no ${missing} colour`);
	const out = { ...p } as TerminalPalette;
	if (!isHexColour(out.cursor)) out.cursor = out.foreground;
	if (!isHexColour(out.cursorAccent)) out.cursorAccent = out.background;
	if (!isHexColour(out.selectionBackground)) out.selectionBackground = out.brightBlack;
	if (!isHexColour(out.selectionForeground)) out.selectionForeground = out.foreground;
	for (const k of PALETTE_ORDER) out[k] = out[k].toLowerCase();
	// Only the palette's own keys, in its order.
	const ordered = {} as TerminalPalette;
	for (const k of PALETTE_ORDER) ordered[k] = out[k];
	return ordered;
}

/** iTerm2's names for the palette's colours. */
const ITERM_KEYS: Record<string, keyof TerminalPalette> = {
	'Background Color': 'background',
	'Foreground Color': 'foreground',
	'Cursor Color': 'cursor',
	'Cursor Text Color': 'cursorAccent',
	'Selection Color': 'selectionBackground',
	'Selected Text Color': 'selectionForeground'
};
for (let i = 0; i < 16; i++) ITERM_KEYS[`Ansi ${i} Color`] = PALETTE_ORDER[6 + i];

/**
 * An iTerm2 .itermcolors file: an XML property list with one dict of red,
 * green and blue components (0 to 1) per colour. Read with patterns rather
 * than an XML parser so it also runs outside a browser; anything that is not
 * a colour (iTerm2's Badge, Link, Bold colours) is skipped.
 */
export function parseItermColors(xml: string): TerminalPalette {
	if (!/<plist[\s>]/.test(xml)) throw new Error('Not an .itermcolors file');
	const p: Partial<TerminalPalette> = {};
	const entry = /<key>([^<]+)<\/key>\s*<dict>([\s\S]*?)<\/dict>/g;
	for (const m of xml.matchAll(entry)) {
		const key = ITERM_KEYS[m[1].trim()];
		if (!key) continue;
		const part = (name: string) => {
			const r = new RegExp(`<key>${name} Component</key>\\s*<(?:real|integer)>([^<]+)</`).exec(m[2]);
			return r ? Number(r[1]) : 0;
		};
		const [r, g, b] = ['Red', 'Green', 'Blue'].map(part);
		if (![r, g, b].every(Number.isFinite)) continue;
		p[key] = `#${hex2(r)}${hex2(g)}${hex2(b)}`;
	}
	return complete(p);
}

/** Windows Terminal's names, where they differ from the palette's. */
const WT_KEYS: Record<string, keyof TerminalPalette> = {
	purple: 'magenta',
	brightPurple: 'brightMagenta',
	cursorColor: 'cursor',
	selectionBackground: 'selectionBackground'
};

/** One scheme object from Windows Terminal's settings.json. */
function windowsTerminalScheme(o: Record<string, unknown>): ParsedScheme {
	const p: Partial<TerminalPalette> = {};
	for (const [k, v] of Object.entries(o)) {
		if (typeof v !== 'string') continue;
		const key = WT_KEYS[k] ?? (PALETTE_ORDER.includes(k as keyof TerminalPalette) ? (k as keyof TerminalPalette) : null);
		if (key) p[key] = v;
	}
	return { name: typeof o.name === 'string' ? o.name : '', palette: complete(p) };
}

/**
 * Every scheme in a file someone picked: an .itermcolors file, or JSON from
 * Windows Terminal (one scheme, a list of them, or a settings.json with a
 * "schemes" list). The name of an .itermcolors scheme is its file name.
 */
export function parseSchemeFile(text: string, fileName: string): ParsedScheme[] {
	const base = fileName.replace(/^.*[\\/]/, '').replace(/\.[^.]+$/, '').trim() || 'Imported';
	const trimmed = text.trim();
	if (trimmed.startsWith('<')) return [{ name: base, palette: parseItermColors(trimmed) }];
	let json: unknown;
	try {
		json = JSON.parse(trimmed);
	} catch {
		throw new Error('Not an .itermcolors or Windows Terminal scheme file');
	}
	const list = Array.isArray(json)
		? json
		: json && typeof json === 'object' && Array.isArray((json as { schemes?: unknown }).schemes)
			? (json as { schemes: unknown[] }).schemes
			: [json];
	const out = list
		.filter((o): o is Record<string, unknown> => !!o && typeof o === 'object')
		.map((o) => windowsTerminalScheme(o))
		.map((s) => ({ ...s, name: s.name || base }));
	if (out.length === 0) throw new Error('The file has no colour scheme');
	return out;
}

/** A scheme name the iTerm2 collection could have: no path, no control characters. */
export function isSafeSchemeName(name: string): boolean {
	return name.length > 0 && name.length <= 100 && !/[\\/\u0000-\u001f]|\.\./.test(name);
}
