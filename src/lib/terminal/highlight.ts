/**
 * What the terminal highlights in what a server prints: words that report a
 * problem or a success, and network addresses (IPv4, IPv6, MAC).
 *
 * Only the colour of the matched text changes, and only on screen: nothing
 * is sent to the server, written to a log, or copied differently. Programs
 * that draw a full screen of their own (vim, htop, mc) are left alone; see
 * the terminal component.
 *
 * Kept free of xterm so a Node test can check it (scripts/highlight-test.mjs).
 */

export type HighlightKind = 'error' | 'warning' | 'success' | 'ipv4' | 'ipv6' | 'mac';

/** In the order Settings lists them. */
export const HIGHLIGHT_KINDS: readonly HighlightKind[] = ['error', 'warning', 'success', 'ipv4', 'ipv6', 'mac'];

/** Off until turned on: colour that a program did not ask for surprises. */
export const DEFAULT_HIGHLIGHT = false;
/** What is highlighted once it is on. Success words are common enough
 *  (ok, done) to be noise for some, so they start off. */
export const DEFAULT_HIGHLIGHT_KINDS: readonly HighlightKind[] = ['error', 'warning', 'ipv4', 'ipv6', 'mac'];

/** A stored list read back, whatever an older or newer version wrote. */
export function highlightKinds(value: unknown): HighlightKind[] {
	if (!Array.isArray(value)) return [...DEFAULT_HIGHLIGHT_KINDS];
	return HIGHLIGHT_KINDS.filter((k) => value.includes(k));
}

export interface Highlight {
	kind: HighlightKind | 'custom';
	/** A custom pattern's own colour, when it gives one. */
	color?: string;
	/** Index in the line's text, as a JS string index. */
	start: number;
	/** One past the last character. */
	end: number;
}

// ---- Words ------------------------------------------------------------------

/** Whole words only, any case: `error` and `ERROR:` but not `errorlevel`. */
const word = (words: string[]) => new RegExp(`(?<![\\w-])(?:${words.join('|')})(?![\\w-])`, 'gi');

const ERROR_WORDS = word([
	'errors?',
	'err',
	'failed',
	'failures?',
	'fail',
	'fatal',
	'panic(?:ked)?',
	'critical',
	'crit',
	'emerg(?:ency)?',
	'denied',
	'refused',
	'unreachable',
	'exception',
	'segmentation fault',
	'segfault',
	'aborted',
	'traceback',
	'not found',
	'no such file or directory',
	'permission denied',
	'connection reset'
]);

const WARNING_WORDS = word(['warn(?:ings?)?', 'deprecated', 'timed out', 'time-?out', 'retrying', 'notice']);

const SUCCESS_WORDS = word(['ok', 'success(?:ful(?:ly)?)?', 'succeeded', 'passed', 'done', 'completed', 'connected']);

// ---- Addresses --------------------------------------------------------------

const OCTET = '(?:25[0-5]|2[0-4]\\d|1\\d\\d|[1-9]?\\d)';
/**
 * Four octets, with a prefix length if one follows. Not part of a longer
 * dotted number (a version such as 1.2.3.4.5) or of a word.
 */
const IPV4 = new RegExp(`(?<![\\w.])${OCTET}(?:\\.${OCTET}){3}(?:/(?:3[0-2]|[12]?\\d))?(?![\\w]|\\.\\d|/\\d)`, 'g');

/**
 * Six pairs split by `:` or `-` (Linux, Windows, most switches), or three
 * groups of four split by dots (Cisco).
 */
const MAC = new RegExp(
	'(?<![\\w:.-])(?:[0-9a-f]{2}([:-])[0-9a-f]{2}(?:\\1[0-9a-f]{2}){4}|[0-9a-f]{4}\\.[0-9a-f]{4}\\.[0-9a-f]{4})(?![\\w]|[:.-][0-9a-f])',
	'gi'
);

/**
 * Something that may be an IPv6 address: hex digits and colons, with an
 * IPv4 tail, a zone (`%eth0`) or a prefix length. `isIpv6` decides.
 */
const IPV6_CANDIDATE = /(?<![\w:.])[0-9a-f]{0,4}(?::[0-9a-f]{0,4}){2,7}(?::\d{1,3}(?:\.\d{1,3}){3})?(?:%[\w.-]+)?(?:\/\d{1,3})?(?![\w:]|\.\d)/gi;

/** A real IPv6 address: eight groups, or fewer with one `::`. */
export function isIpv6(text: string): boolean {
	const bare = text.replace(/\/\d{1,3}$/, '').replace(/%[\w.-]+$/, '');
	const prefix = /\/(\d{1,3})$/.exec(text);
	if (prefix && Number(prefix[1]) > 128) return false;
	// A time (12:34:56) or a MAC has no `::` and too few groups; a lone `::`
	// says nothing.
	if (!/\d/.test(bare) && !/[a-f]/i.test(bare)) return false;
	const halves = bare.split('::');
	if (halves.length > 2) return false;
	const groups = (s: string): string[] | null => {
		if (s === '') return [];
		const parts = s.split(':');
		const out: string[] = [];
		for (let i = 0; i < parts.length; i++) {
			const p = parts[i];
			if (i === parts.length - 1 && p.includes('.')) {
				if (!new RegExp(`^${OCTET}(?:\\.${OCTET}){3}$`).test(p)) return null;
				out.push('v4a', 'v4b');
			} else if (/^[0-9a-f]{1,4}$/i.test(p)) out.push(p);
			else return null;
		}
		return out;
	};
	const left = groups(halves[0]);
	const right = halves.length === 2 ? groups(halves[1]) : [];
	if (!left || !right) return false;
	// An IPv4 tail only at the very end.
	if (halves.length === 2 && left.includes('v4a')) return false;
	const count = left.length + right.length;
	return halves.length === 2 ? count <= 7 : count === 8;
}

// ---- Custom patterns --------------------------------------------------------

/** One of the user's own patterns, ready to match. */
export interface CustomRule {
	re: RegExp;
	/** `#rrggbb`, or null for the default custom colour. */
	color: string | null;
	/** The line it came from, as typed. */
	source: string;
}

export interface CustomRuleError {
	/** 1-based line number in the field. */
	line: number;
	message: string;
}

/** Longest pattern taken; anything longer is almost surely a paste gone wrong. */
const MAX_PATTERN = 500;

/**
 * The Settings field read as patterns: one regular expression per line.
 *
 *  - `pattern` matches as written (case-sensitive);
 *  - `/pattern/flags` takes JavaScript flags, such as `i` to ignore case;
 *  - `#rrggbb ` in front gives that pattern its own colour.
 *
 * Empty lines and lines starting with `//` are skipped. A line that is not a
 * valid expression, or one that can match nothing (an empty match), is left
 * out and reported, so Settings can show what is wrong while the rest work.
 */
export function parseCustomRules(text: string | undefined | null): { rules: CustomRule[]; errors: CustomRuleError[] } {
	const rules: CustomRule[] = [];
	const errors: CustomRuleError[] = [];
	(text ?? '').split(/\r?\n/).forEach((raw, i) => {
		let line = raw.trim();
		if (!line || line.startsWith('//')) return;
		let color: string | null = null;
		const c = /^(#[0-9a-f]{6}|#[0-9a-f]{3})\s+(.+)$/i.exec(line);
		if (c) {
			color = hexColor(c[1]);
			line = c[2].trim();
		}
		if (line.length > MAX_PATTERN) {
			errors.push({ line: i + 1, message: 'too long' });
			return;
		}
		let body = line;
		let flags = '';
		const slashed = /^\/(.+)\/([a-z]*)$/i.exec(line);
		if (slashed) {
			body = slashed[1];
			flags = slashed[2].toLowerCase();
			if (/[^imsu]/.test(flags) || new Set(flags).size !== flags.length) {
				errors.push({ line: i + 1, message: `unknown flags "${slashed[2]}" (use i, m, s, u)` });
				return;
			}
		}
		let re: RegExp;
		try {
			re = new RegExp(body, flags + 'g');
		} catch (e) {
			errors.push({ line: i + 1, message: e instanceof Error ? e.message.replace(/^Invalid regular expression: (?:\/.*\/[a-z]*: )?/, '') : String(e) });
			return;
		}
		if (re.test('')) {
			errors.push({ line: i + 1, message: 'matches empty text' });
			return;
		}
		rules.push({ re, color, source: raw.trim() });
	});
	return { rules, errors };
}

/**
 * What to highlight in one line of terminal text, in order and never
 * overlapping. Custom patterns go first: where one overlaps a built-in
 * match, the custom one is shown. Among the rest, the one that starts first
 * wins, then the longer one: an IPv4 address inside an IPv6 one is part of it.
 */
export function findHighlights(line: string, kinds: readonly HighlightKind[], custom: readonly CustomRule[] = []): Highlight[] {
	if (!line.trim() || (kinds.length === 0 && custom.length === 0)) return [];
	const placed: Highlight[] = [];
	const free = (start: number, end: number) => placed.every((h) => end <= h.start || start >= h.end);
	const place = (found: Highlight[]) => {
		found.sort((a, b) => a.start - b.start || b.end - b.start - (a.end - a.start));
		for (const h of found) if (free(h.start, h.end)) placed.push(h);
	};

	const mine: Highlight[] = [];
	for (const rule of custom) {
		rule.re.lastIndex = 0;
		for (const m of line.matchAll(rule.re)) {
			if (!m[0]) continue;
			mine.push({ kind: 'custom', start: m.index!, end: m.index! + m[0].length, ...(rule.color ? { color: rule.color } : {}) });
		}
	}
	place(mine);

	const found: Highlight[] = [];
	const add = (kind: HighlightKind, re: RegExp, test?: (s: string) => boolean) => {
		if (!kinds.includes(kind)) return;
		re.lastIndex = 0;
		for (const m of line.matchAll(re)) {
			if (test && !test(m[0])) continue;
			found.push({ kind, start: m.index!, end: m.index! + m[0].length });
		}
	};
	add('mac', MAC);
	add('ipv6', IPV6_CANDIDATE, isIpv6);
	add('ipv4', IPV4);
	add('error', ERROR_WORDS);
	add('warning', WARNING_WORDS);
	add('success', SUCCESS_WORDS);
	place(found);

	return placed.sort((a, b) => a.start - b.start);
}

// ---- Colours ----------------------------------------------------------------

/** The terminal theme's colours a highlight is drawn in. */
export interface HighlightPalette {
	red?: string;
	brightRed?: string;
	yellow?: string;
	brightYellow?: string;
	green?: string;
	brightGreen?: string;
	cyan?: string;
	brightCyan?: string;
	blue?: string;
	brightBlue?: string;
	magenta?: string;
	brightMagenta?: string;
}

/** Each kind's colour from the theme, so it reads on that theme's background. */
const FROM_THEME: Record<HighlightKind, [keyof HighlightPalette, string]> = {
	error: ['brightRed', '#ff5f5f'],
	warning: ['brightYellow', '#ffd75f'],
	success: ['brightGreen', '#5fd75f'],
	ipv4: ['brightCyan', '#5fd7ff'],
	ipv6: ['brightBlue', '#87afff'],
	mac: ['brightMagenta', '#d787ff']
};

/** `#rrggbb` for a theme colour, or null: xterm decorations take no other form. */
export function hexColor(value: string | undefined): string | null {
	if (!value) return null;
	const v = value.trim();
	if (/^#[0-9a-f]{6}$/i.test(v)) return v.toLowerCase();
	if (/^#[0-9a-f]{8}$/i.test(v)) return v.slice(0, 7).toLowerCase();
	if (/^#[0-9a-f]{3}$/i.test(v)) return ('#' + [...v.slice(1)].map((c) => c + c).join('')).toLowerCase();
	const rgb = /^rgba?\(\s*(\d{1,3})\s*,\s*(\d{1,3})\s*,\s*(\d{1,3})/i.exec(v);
	if (rgb) return '#' + rgb.slice(1, 4).map((n) => Math.min(255, Number(n)).toString(16).padStart(2, '0')).join('');
	return null;
}

/** What a custom pattern without its own colour is drawn in. */
const CUSTOM_FROM_THEME: [keyof HighlightPalette, string] = ['yellow', '#ffaf00'];

export function highlightColors(palette: HighlightPalette): Record<HighlightKind | 'custom', string> {
	const out = {} as Record<HighlightKind | 'custom', string>;
	out.custom = hexColor(palette[CUSTOM_FROM_THEME[0]]) ?? CUSTOM_FROM_THEME[1];
	for (const kind of HIGHLIGHT_KINDS) {
		const [key, fallback] = FROM_THEME[kind];
		out[kind] = hexColor(palette[key]) ?? fallback;
	}
	return out;
}
