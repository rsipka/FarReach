import { themeListInstalled } from '#lib/ipc/theme.js';

/**
 * Theme engine.
 *
 * A theme is data, not code: a map of design tokens plus a terminal palette.
 * Applying one writes CSS custom properties onto the document root, so every
 * component styled with var(--color-...) re-themes instantly, and pushes the
 * terminal palette into xterm, which keeps its colours in JS rather than CSS.
 *
 * Because themes carry no executable code, installing one from the marketplace
 * is far lower risk than installing a plugin — the worst a bad theme can do is
 * look wrong.
 */

/** The 16 ANSI colours plus the terminal chrome colours xterm needs. */
export interface TerminalPalette {
	background: string;
	foreground: string;
	cursor: string;
	cursorAccent: string;
	selectionBackground: string;
	selectionForeground: string;
	black: string;
	red: string;
	green: string;
	yellow: string;
	blue: string;
	magenta: string;
	cyan: string;
	white: string;
	brightBlack: string;
	brightRed: string;
	brightGreen: string;
	brightYellow: string;
	brightBlue: string;
	brightMagenta: string;
	brightCyan: string;
	brightWhite: string;
}

/** UI tokens. Each key maps to a --color-<key> custom property. */
export interface ThemeColors {
	'bg-primary': string;
	'bg-secondary': string;
	'bg-elevated': string;
	border: string;
	'text-primary': string;
	'text-secondary': string;
	'text-tertiary': string;
	accent: string;
	'accent-hover': string;
	success: string;
	warning: string;
	danger: string;
	'surface-hover': string;
	'surface-active': string;
	'surface-sunken': string;
	'surface-strong': string;
	/** Optional: added later, so older and marketplace themes may lack them.
	 *  Missing ones are derived from the colours above (see derivedColors). */
	'border-strong'?: string;
	/** Filled buttons that carry white text; `accent` is for text and icons. */
	'accent-fill'?: string;
	/** Tint behind a selected row or the active item. */
	'accent-subtle'?: string;
	/** Keyboard focus ring. */
	focus?: string;
}

export interface Theme {
	id: string;
	name: string;
	author?: string;
	version?: string;
	/** Drives the light/dark class so native scrollbars and controls match. */
	appearance: 'dark' | 'light';
	colors: ThemeColors;
	terminal: TerminalPalette;
}

/** Every key a complete theme must define. Used to validate imported themes. */
export const COLOR_KEYS: (keyof ThemeColors)[] = [
	'bg-primary',
	'bg-secondary',
	'bg-elevated',
	'border',
	'text-primary',
	'text-secondary',
	'text-tertiary',
	'accent',
	'accent-hover',
	'success',
	'warning',
	'danger',
	'surface-hover',
	'surface-active',
	'surface-sunken',
	'surface-strong'
];

/** Tokens a theme may leave out; applyTheme fills them in. */
export const OPTIONAL_COLOR_KEYS = ['border-strong', 'accent-fill', 'accent-subtle', 'focus'] as const;

/**
 * Values for the optional tokens a theme does not set, from the colours it
 * does: a field edge halfway from the divider to the secondary text, the
 * accent itself for fills and focus, and a light tint of it for selection.
 */
export function derivedColors(c: ThemeColors): Record<(typeof OPTIONAL_COLOR_KEYS)[number], string> {
	return {
		'border-strong': c['border-strong'] ?? `color-mix(in srgb, ${c['text-secondary']} 55%, ${c.border})`,
		'accent-fill': c['accent-fill'] ?? c.accent,
		'accent-subtle': c['accent-subtle'] ?? `color-mix(in srgb, ${c.accent} 14%, transparent)`,
		focus: c.focus ?? c.accent
	};
}

export const TERMINAL_KEYS: (keyof TerminalPalette)[] = [
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

export const DARK: Theme = {
	id: 'reach-dark',
	name: 'Reach Dark',
	appearance: 'dark',
	colors: {
		'bg-primary': '#0e0f12',
		'bg-secondary': '#1a1b20',
		'bg-elevated': '#24252b',
		border: '#2e3037',
		'text-primary': '#ececef',
		'text-secondary': '#a3a6ad',
		'text-tertiary': '#868a93',
		accent: '#4c9dff',
		'accent-hover': '#74b3ff',
		success: '#3fcf6a',
		warning: '#e8b730',
		danger: '#ff6b61',
		'surface-hover': 'rgba(255, 255, 255, 0.07)',
		'surface-active': 'rgba(255, 255, 255, 0.12)',
		'surface-sunken': 'rgba(0, 0, 0, 0.25)',
		'surface-strong': 'rgba(255, 255, 255, 0.22)',
		'border-strong': '#555963',
		'accent-fill': '#2563eb',
		'accent-subtle': 'rgba(76, 157, 255, 0.16)',
		focus: '#4c9dff'
	},
	terminal: {
		background: '#0e0f12',
		foreground: '#ececef',
		cursor: '#0a84ff',
		cursorAccent: '#0e0f12',
		selectionBackground: 'rgba(10, 132, 255, 0.3)',
		selectionForeground: '#f5f5f7',
		black: '#1d1f21',
		red: '#ff453a',
		green: '#30d158',
		yellow: '#ffd60a',
		blue: '#0a84ff',
		magenta: '#bf5af2',
		cyan: '#64d2ff',
		white: '#f5f5f7',
		brightBlack: '#6e6e73',
		brightRed: '#ff6961',
		brightGreen: '#4ae06a',
		brightYellow: '#ffe566',
		brightBlue: '#409cff',
		brightMagenta: '#da8fff',
		brightCyan: '#8be8ff',
		brightWhite: '#ffffff'
	}
};

export const LIGHT: Theme = {
	id: 'reach-light',
	name: 'Reach Light',
	appearance: 'light',
	colors: {
		'bg-primary': '#ffffff',
		'bg-secondary': '#eff1f4',
		'bg-elevated': '#ffffff',
		border: '#d5d9df',
		'text-primary': '#17191c',
		'text-secondary': '#50565f',
		'text-tertiary': '#656b75',
		accent: '#0b63ce',
		'accent-hover': '#0a56b3',
		success: '#1a7f37',
		warning: '#9a6700',
		danger: '#cf222e',
		'surface-hover': 'rgba(0, 0, 0, 0.05)',
		'surface-active': 'rgba(0, 0, 0, 0.09)',
		'surface-sunken': 'rgba(0, 0, 0, 0.03)',
		'surface-strong': 'rgba(0, 0, 0, 0.22)',
		'border-strong': '#8c929c',
		'accent-fill': '#0b63ce',
		'accent-subtle': 'rgba(11, 99, 206, 0.10)',
		focus: '#0b63ce'
	},
	terminal: {
		// The terminal stayed #0a0a0a in light mode before themes existed — a
		// black rectangle filling most of an otherwise white app.
		background: '#ffffff',
		foreground: '#1d1d1f',
		cursor: '#007aff',
		cursorAccent: '#ffffff',
		selectionBackground: 'rgba(0, 122, 255, 0.25)',
		selectionForeground: '#1d1d1f',
		black: '#1d1d1f',
		red: '#d70015',
		green: '#248a3d',
		yellow: '#a05a00',
		blue: '#0040dd',
		magenta: '#9f0d91',
		cyan: '#0071a4',
		white: '#8e8e93',
		brightBlack: '#6e6e73',
		brightRed: '#ff3b30',
		brightGreen: '#34c759',
		brightYellow: '#ff9f0a',
		brightBlue: '#007aff',
		brightMagenta: '#af52de',
		brightCyan: '#55bef0',
		brightWhite: '#1d1d1f'
	}
};

export const BUILT_IN: Theme[] = [DARK, LIGHT];

class ThemeState {
	current = $state<Theme>(DARK);
	/** Themes beyond the built-ins, e.g. installed from the marketplace. */
	installed = $state<Theme[]>([]);

	get all(): Theme[] {
		return [...BUILT_IN, ...this.installed];
	}
}

export const themeState = new ThemeState();

/**
 * Pull installed themes off disk into the engine.
 *
 * Must run once at startup. `themeState.all` is what every picker lists and
 * what the boot-time effect in +layout.svelte resolves `settings.themeId`
 * against, and until this has run it holds only the two built-ins — so an
 * installed theme did not apply on launch and did not appear in any picker
 * until Settings > Appearance happened to be opened, which was the only place
 * that loaded them.
 *
 * Themes are re-validated here as well as in the backend: a theme file could
 * have been edited on disk since it was written. An unreadable store leaves the
 * built-ins in place rather than throwing — a bad theme must not stop the app
 * from starting.
 */
export async function loadInstalledThemes(): Promise<void> {
	try {
		const docs = await themeListInstalled();
		themeState.installed = docs.filter((d) => validateTheme(d).ok);
	} catch (err) {
		console.error('Could not read installed themes:', err);
	}
}

/**
 * Write a theme onto the document.
 *
 * Each token becomes an inline custom property on :root, which beats the
 * stylesheet defaults without needing a class per theme, and the dark/light
 * class is swapped so native chrome stays consistent.
 */
export function applyTheme(theme: Theme): void {
	const root = document.documentElement;
	for (const key of COLOR_KEYS) {
		const value = theme.colors[key];
		if (value) root.style.setProperty('--color-' + key, value);
	}
	for (const [key, value] of Object.entries(derivedColors(theme.colors))) {
		root.style.setProperty('--color-' + key, value);
	}
	root.classList.remove('dark', 'light');
	root.classList.add(theme.appearance);
	themeState.current = theme;
}

/** Reject a theme that would leave part of the app unstyled. */
export function validateTheme(
	input: unknown
): { ok: true; theme: Theme } | { ok: false; error: string } {
	if (typeof input !== 'object' || input === null) {
		return { ok: false, error: 'Theme must be an object' };
	}
	const t = input as Partial<Theme>;
	if (typeof t.id !== 'string' || !t.id) return { ok: false, error: 'Missing id' };
	if (typeof t.name !== 'string' || !t.name) return { ok: false, error: 'Missing name' };
	if (t.appearance !== 'dark' && t.appearance !== 'light') {
		return { ok: false, error: 'appearance must be "dark" or "light"' };
	}
	if (typeof t.colors !== 'object' || t.colors === null) {
		return { ok: false, error: 'Missing colors' };
	}
	if (typeof t.terminal !== 'object' || t.terminal === null) {
		return { ok: false, error: 'Missing terminal palette' };
	}

	const colors = t.colors as ThemeColors;
	const missingColor = COLOR_KEYS.find((k) => typeof colors[k] !== 'string');
	if (missingColor) return { ok: false, error: 'Missing colors.' + missingColor };

	const terminal = t.terminal as TerminalPalette;
	const missingTerm = TERMINAL_KEYS.find((k) => typeof terminal[k] !== 'string');
	if (missingTerm) return { ok: false, error: 'Missing terminal.' + missingTerm };

	return { ok: true, theme: t as Theme };
}
