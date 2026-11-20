import type { TerminalPalette, Theme } from '#lib/state/theme.svelte.js';

/**
 * Reacher CM's own colourful app themes: tinted backgrounds and a strong
 * accent, rather than the grey of most palettes. Five families with a dark
 * and a light side, so each makes a natural System pair (one for the day, one
 * for the night), plus Neon Night.
 *
 * Our own palettes, under this project's licence. Contrast floors are checked
 * with the other bundled themes by scripts/app-themes-test.mjs.
 */

interface Spec {
	id: string;
	name: string;
	appearance: 'dark' | 'light';
	/** bg-primary, bg-secondary, bg-elevated, border */
	bg: [string, string, string, string];
	/** text-primary, text-secondary, text-tertiary */
	text: [string, string, string];
	/** accent, accent-hover, accent-fill (carries white text) */
	accent: [string, string, string];
	/** success, warning, danger */
	status: [string, string, string];
	/** Terminal colours this family changes from the shared set. */
	terminal?: Partial<TerminalPalette>;
}

function alpha(hex: string, a: number): string {
	const n = parseInt(hex.slice(1), 16);
	return `rgba(${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255}, ${a})`;
}

/** Vivid ANSI colours for the dark side; black is the theme's own. */
const DARK_ANSI = {
	red: '#ff6b7a',
	green: '#3ddc97',
	yellow: '#f7c948',
	blue: '#5aa9ff',
	magenta: '#d38cff',
	cyan: '#3fd5e8',
	white: '#d8e2ea',
	brightRed: '#ff95a0',
	brightGreen: '#74ecb6',
	brightYellow: '#ffdc7a',
	brightBlue: '#8cc4ff',
	brightMagenta: '#e4b3ff',
	brightCyan: '#7ae6f3',
	brightWhite: '#ffffff'
};

/** And for the light side. brightWhite stays light: TUI apps draw it on ANSI
 *  black (upstream Reach issue #174). */
const LIGHT_ANSI = {
	black: '#1f2328',
	red: '#c62839',
	green: '#17803d',
	yellow: '#8a6100',
	blue: '#0b5cc4',
	magenta: '#9c27b0',
	cyan: '#00758a',
	white: '#6b7280',
	brightBlack: '#57606a',
	brightRed: '#e0343f',
	brightGreen: '#1f9d4f',
	brightYellow: '#b07800',
	brightBlue: '#2f7ae5',
	brightMagenta: '#b545c9',
	brightCyan: '#0896ad',
	brightWhite: '#e7e9ec'
};

function make(s: Spec): Theme {
	const dark = s.appearance === 'dark';
	const [bg, bg2, elev, border] = s.bg;
	const [text, text2, text3] = s.text;
	const [accent, hover, fill] = s.accent;
	const [success, warning, danger] = s.status;
	return {
		id: s.id,
		name: s.name,
		author: 'Reacher CM',
		appearance: s.appearance,
		colors: {
			'bg-primary': bg,
			'bg-secondary': bg2,
			'bg-elevated': elev,
			border,
			'text-primary': text,
			'text-secondary': text2,
			'text-tertiary': text3,
			accent,
			'accent-hover': hover,
			'accent-fill': fill,
			'accent-subtle': alpha(accent, dark ? 0.18 : 0.12),
			focus: accent,
			success,
			warning,
			danger,
			'surface-hover': dark ? 'rgba(255, 255, 255, 0.07)' : 'rgba(0, 0, 0, 0.05)',
			'surface-active': alpha(accent, dark ? 0.2 : 0.14),
			'surface-sunken': dark ? 'rgba(0, 0, 0, 0.25)' : 'rgba(0, 0, 0, 0.03)',
			'surface-strong': dark ? 'rgba(255, 255, 255, 0.22)' : 'rgba(0, 0, 0, 0.22)'
		},
		terminal: {
			background: bg,
			foreground: text,
			cursor: accent,
			cursorAccent: bg,
			selectionBackground: alpha(accent, dark ? 0.32 : 0.24),
			selectionForeground: text,
			...(dark ? { black: elev, brightBlack: text3, ...DARK_ANSI } : LIGHT_ANSI),
			...s.terminal
		} as TerminalPalette
	};
}

export const COLORFUL: Theme[] = [
	make({
		id: 'ocean-dark',
		name: 'Ocean Dark',
		appearance: 'dark',
		bg: ['#071a26', '#0c2433', '#123044', '#1d4258'],
		text: ['#e2f3fb', '#9cc3d6', '#7aa6bc'],
		accent: ['#22c7e8', '#5dd8f0', '#0a7393'],
		status: ['#3ddc97', '#f2c14e', '#ff6b7a'],
		terminal: { blue: '#4fa3ff', cyan: '#22c7e8' }
	}),
	make({
		id: 'ocean-light',
		name: 'Ocean Light',
		appearance: 'light',
		bg: ['#f2fbff', '#e0f2fa', '#ffffff', '#b9dcea'],
		text: ['#0b2533', '#2f5566', '#4f7484'],
		accent: ['#0377a8', '#025f87', '#0377a8'],
		status: ['#0f7a4f', '#9a6200', '#c62839']
	}),
	make({
		id: 'forest-dark',
		name: 'Forest Dark',
		appearance: 'dark',
		bg: ['#0b1a12', '#10241a', '#163023', '#22442f'],
		text: ['#e4f5e9', '#a3c9ae', '#80a98b'],
		accent: ['#4ade80', '#7ee8a4', '#15803d'],
		status: ['#4ade80', '#facc15', '#f87171'],
		terminal: { green: '#4ade80' }
	}),
	make({
		id: 'forest-light',
		name: 'Forest Light',
		appearance: 'light',
		bg: ['#f4fbf5', '#e2f3e6', '#ffffff', '#bfdec7'],
		text: ['#102a19', '#345a40', '#54785e'],
		accent: ['#15803d', '#116632', '#15803d'],
		status: ['#15803d', '#8a6100', '#c0262d']
	}),
	make({
		id: 'sunset-dark',
		name: 'Sunset Dark',
		appearance: 'dark',
		bg: ['#1f1020', '#2a1529', '#361b33', '#4a2645'],
		text: ['#fdeee6', '#e0b6b0', '#c09090'],
		accent: ['#ff8a4c', '#ffa877', '#c2410c'],
		status: ['#7bd88f', '#ffd166', '#ff5c7a'],
		terminal: { yellow: '#ffb347', red: '#ff5c7a' }
	}),
	make({
		id: 'sunset-light',
		name: 'Sunset Light',
		appearance: 'light',
		bg: ['#fff7f0', '#fde8da', '#ffffff', '#f1c9b2'],
		text: ['#2e1510', '#6b3b2e', '#8c5a4b'],
		accent: ['#c2410c', '#9a3412', '#c2410c'],
		status: ['#237a3b', '#8f5c00', '#c0233b']
	}),
	make({
		id: 'amethyst-dark',
		name: 'Amethyst Dark',
		appearance: 'dark',
		bg: ['#160f26', '#1e1533', '#281c42', '#392a5a'],
		text: ['#efe9ff', '#bcb0e0', '#998cc4'],
		accent: ['#b18cff', '#c8adff', '#7c3aed'],
		status: ['#5eead4', '#fbbf24', '#fb7185'],
		terminal: { magenta: '#b18cff' }
	}),
	make({
		id: 'amethyst-light',
		name: 'Amethyst Light',
		appearance: 'light',
		bg: ['#faf7ff', '#eee7fd', '#ffffff', '#d6c9f5'],
		text: ['#1f1536', '#4b3d70', '#6c5e92'],
		accent: ['#6d28d9', '#5b21b6', '#6d28d9'],
		status: ['#17803d', '#8f5f00', '#be123c']
	}),
	make({
		id: 'cherry-dark',
		name: 'Cherry Dark',
		appearance: 'dark',
		bg: ['#1e0d16', '#29121e', '#351828', '#4a2237'],
		text: ['#ffe9f2', '#e5b3c8', '#c48ea6'],
		accent: ['#ff6fae', '#ff95c3', '#be185d'],
		status: ['#86efac', '#fcd34d', '#ff7a6b'],
		terminal: { magenta: '#ff6fae' }
	}),
	make({
		id: 'cherry-light',
		name: 'Cherry Light',
		appearance: 'light',
		bg: ['#fff6fa', '#fde4ef', '#ffffff', '#f3c3d8'],
		text: ['#33111f', '#6e3550', '#8f5570'],
		accent: ['#be185d', '#9d174d', '#be185d'],
		status: ['#1b7a3d', '#8d5d00', '#c2261b']
	}),
	make({
		id: 'neon-night',
		name: 'Neon Night',
		appearance: 'dark',
		bg: ['#0d0b1f', '#15112d', '#1d1840', '#2c2560'],
		text: ['#f2eeff', '#b9b0e6', '#9188c8'],
		accent: ['#ff4fd8', '#ff85e4', '#a21caf'],
		status: ['#39ffa0', '#ffe14d', '#ff4f6d'],
		terminal: {
			red: '#ff4f6d',
			green: '#39ffa0',
			yellow: '#ffe14d',
			blue: '#4d9dff',
			magenta: '#ff4fd8',
			cyan: '#00e5ff'
		}
	})
];
