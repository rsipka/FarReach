/**
 * What the mouse does in the terminal: the right button, the middle button,
 * and where a double click's word ends.
 *
 * Terminals disagree, and people bring their habits with them: PuTTY pastes
 * on a right click, Windows Terminal copies a selection and pastes without
 * one, most others open a menu. So it is a setting, and the rule for each
 * choice lives here, where it can be tested without a terminal.
 */

/** What a right click does. */
export type RightClickMode = 'menu' | 'paste' | 'copy-paste';

export const RIGHT_CLICK_MODES: readonly RightClickMode[] = ['menu', 'paste', 'copy-paste'];

/** A menu: the one choice that never does anything by surprise. */
export const DEFAULT_RIGHT_CLICK: RightClickMode = 'menu';

/** A stored value read back, whatever an older or newer version wrote. */
export function rightClickMode(value: unknown): RightClickMode {
	return RIGHT_CLICK_MODES.includes(value as RightClickMode) ? (value as RightClickMode) : DEFAULT_RIGHT_CLICK;
}

export interface RightClick {
	mode: RightClickMode;
	hasSelection: boolean;
	/** The program in the terminal asked for mouse events (mc, vim, tmux). */
	mouseTracking: boolean;
	shift: boolean;
	ctrl: boolean;
}

export type RightClickAction = 'menu' | 'copy' | 'paste' | 'none';

/**
 * What this right click does.
 *
 * A program that tracks the mouse gets the click and nothing else happens:
 * a paste into mc on top of its own right-click was never wanted. Shift
 * takes the mouse back from the program, as it does for selecting. Ctrl
 * opens the menu in the modes that have none, as in PuTTY.
 */
export function rightClickAction(click: RightClick): RightClickAction {
	if (click.mouseTracking && !click.shift) return 'none';
	if (click.ctrl || click.mode === 'menu') return 'menu';
	if (click.mode === 'paste') return 'paste';
	return click.hasSelection ? 'copy' : 'paste';
}

/**
 * The characters that end a word on a double click, besides the space.
 *
 * xterm's own set: brackets and quotes. A path, a host name, an address and
 * a URL have none of them, so each is picked whole.
 */
export const DEFAULT_WORD_SEPARATORS = '()[]{}\',"`';

/**
 * The separators as xterm wants them. Blanks always end a word, whatever
 * was typed in the field, and a character given twice counts once.
 */
export function wordSeparator(chars: unknown): string {
	const typed = typeof chars === 'string' ? chars : DEFAULT_WORD_SEPARATORS;
	return ' ' + [...new Set(typed.replace(/\s/g, ''))].join('');
}

/**
 * How long after a middle click a paste event is taken to be its doing. On
 * Linux the webview pastes the selection itself, right after the click.
 */
export const MIDDLE_CLICK_PASTE_MS = 300;
