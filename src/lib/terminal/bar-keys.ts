/**
 * The keys of the extra-keys bar above a phone's keyboard, and the order the
 * user put them in (Settings > Terminal > Phone keys).
 *
 * A key is stored as a short id: 'esc', 'f5', or 'sym:|' for a character.
 * Ids this version does not know are dropped when read, so a list written by
 * a newer version still works.
 */

export const DEFAULT_BAR_KEYS = [
	'esc', 'tab', 'ctrl', 'alt', 'left', 'up', 'down', 'right', 'home', 'end', 'pgup', 'pgdn',
	'sym:|', 'sym:~', 'sym:/', 'sym:-', 'sym::', 'sym:_'
];

const NAMED = ['esc', 'tab', 'ctrl', 'alt', 'left', 'up', 'down', 'right', 'home', 'end', 'pgup', 'pgdn', 'del'];
export const FUNCTION_KEYS = Array.from({ length: 12 }, (_, i) => `f${i + 1}`);
export const SYMBOLS = ['|', '~', '/', '\\', '-', '_', ':', ';', '=', '+', '*', '&', '$', '#', '!', '?', '@', '%', '^', '`', "'", '"', '<', '>', '(', ')', '[', ']', '{', '}'];

/** Every key that can go on the bar, in the order the editor offers them. */
export const ALL_BAR_KEYS = [...NAMED, ...FUNCTION_KEYS, ...SYMBOLS.map((c) => `sym:${c}`)];

export function isBarKeyId(id: string): boolean {
	return ALL_BAR_KEYS.includes(id);
}

/** The user's list, or the default when there is none or nothing usable in it. */
export function barKeys(saved: string[] | null | undefined): string[] {
	if (!saved) return DEFAULT_BAR_KEYS;
	const known = saved.filter(isBarKeyId);
	return known.length > 0 ? known : DEFAULT_BAR_KEYS;
}

/** What the key's button says. */
export function barKeyLabel(id: string): string {
	if (id.startsWith('sym:')) return id.slice(4);
	const arrows: Record<string, string> = { left: '←', up: '↑', down: '↓', right: '→' };
	if (arrows[id]) return arrows[id];
	const names: Record<string, string> = {
		esc: 'Esc', tab: 'Tab', ctrl: 'Ctrl', alt: 'Alt', home: 'Home', end: 'End', pgup: 'PgUp', pgdn: 'PgDn', del: 'Del'
	};
	return names[id] ?? id.toUpperCase();
}
