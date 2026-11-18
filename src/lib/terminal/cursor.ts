/**
 * The terminal cursor: its shape, whether it blinks, how thick a bar is, and
 * what it looks like in a terminal that does not have the keyboard (a split's
 * other pane, or the app in the background).
 *
 * Settings hold the default; a saved session can choose its own shape and
 * blink. A program can still change the shape while it runs (vim's insert
 * mode asks for a bar); this is what the terminal starts with and returns to.
 */

export type CursorStyle = 'block' | 'underline' | 'bar';
export type InactiveCursorStyle = 'outline' | 'block' | 'bar' | 'underline' | 'none';

export const CURSOR_STYLES: readonly CursorStyle[] = ['block', 'underline', 'bar'];
export const INACTIVE_CURSOR_STYLES: readonly InactiveCursorStyle[] = ['outline', 'block', 'bar', 'underline', 'none'];
/** A bar's thickness, in CSS pixels. */
export const CURSOR_WIDTHS: readonly number[] = [1, 2, 3, 4];

/** What Reach always drew before this was a setting. */
export const DEFAULT_CURSOR_STYLE: CursorStyle = 'bar';
export const DEFAULT_CURSOR_BLINK = true;
/** xterm.js's own: a hollow box. */
export const DEFAULT_INACTIVE_CURSOR: InactiveCursorStyle = 'outline';
export const DEFAULT_CURSOR_WIDTH = 1;

/** A stored shape read back, whatever an older or newer version wrote. */
export function cursorStyle(value: unknown): CursorStyle {
	return CURSOR_STYLES.includes(value as CursorStyle) ? (value as CursorStyle) : DEFAULT_CURSOR_STYLE;
}

export function inactiveCursorStyle(value: unknown): InactiveCursorStyle {
	return INACTIVE_CURSOR_STYLES.includes(value as InactiveCursorStyle)
		? (value as InactiveCursorStyle)
		: DEFAULT_INACTIVE_CURSOR;
}

export function cursorWidth(value: unknown): number {
	return CURSOR_WIDTHS.includes(value as number) ? (value as number) : DEFAULT_CURSOR_WIDTH;
}

/** The cursor settings, as Settings keeps them. */
export interface CursorDefaults {
	terminalCursorStyle: unknown;
	terminalCursorBlink: unknown;
	terminalCursorInactive: unknown;
	terminalCursorWidth: unknown;
}

/** A session's own choice; unset (or unknown) follows Settings. */
export interface CursorOverride {
	style?: string | null;
	blink?: boolean | null;
}

/** xterm.js's options for the cursor. */
export interface CursorOptions {
	cursorStyle: CursorStyle;
	cursorBlink: boolean;
	cursorInactiveStyle: InactiveCursorStyle;
	cursorWidth: number;
}

/** The cursor a terminal draws: the session's choice over the default. */
export function cursorOptions(defaults: CursorDefaults, session?: CursorOverride | null): CursorOptions {
	const own = session?.style;
	return {
		cursorStyle: CURSOR_STYLES.includes(own as CursorStyle) ? (own as CursorStyle) : cursorStyle(defaults.terminalCursorStyle),
		cursorBlink:
			typeof session?.blink === 'boolean'
				? session.blink
				: typeof defaults.terminalCursorBlink === 'boolean'
					? defaults.terminalCursorBlink
					: DEFAULT_CURSOR_BLINK,
		cursorInactiveStyle: inactiveCursorStyle(defaults.terminalCursorInactive),
		cursorWidth: cursorWidth(defaults.terminalCursorWidth)
	};
}
