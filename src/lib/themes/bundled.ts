import type { TerminalPalette, Theme, ThemeColors } from '#lib/state/theme.svelte.js';

/**
 * App themes that ship with Reacher CM, beside Reach Dark and Reach Light.
 *
 * Each is our own mapping of a published, MIT-licensed palette onto the UI
 * tokens, plus the palette's terminal colours; the two High Contrast themes are
 * our own. Sources and licences: src/lib/data/app-themes-licenses.md.
 *
 * Every theme here sets `accent-fill` explicitly: filled buttons carry white
 * text, and most of these palettes' accents are too light for it, so the
 * fallback (the accent itself) would be unreadable. scripts/app-themes-test.mjs
 * checks that and the other contrast floors.
 *
 * Ids `nord` and `solarized-light` match upstream Reach's theme registry, so a
 * copy installed from there is replaced by the built-in one rather than
 * listed twice.
 */

type Colors = Omit<ThemeColors, 'surface-hover' | 'surface-active' | 'surface-sunken' | 'surface-strong'> &
	Partial<Pick<ThemeColors, 'surface-hover' | 'surface-active' | 'surface-sunken' | 'surface-strong'>> & {
		'accent-fill': string;
	};

/** The translucent overlays: white over a dark theme, black over a light one. */
function surfaces(appearance: 'dark' | 'light') {
	return appearance === 'dark'
		? {
				'surface-hover': 'rgba(255, 255, 255, 0.07)',
				'surface-active': 'rgba(255, 255, 255, 0.12)',
				'surface-sunken': 'rgba(0, 0, 0, 0.25)',
				'surface-strong': 'rgba(255, 255, 255, 0.22)'
			}
		: {
				'surface-hover': 'rgba(0, 0, 0, 0.05)',
				'surface-active': 'rgba(0, 0, 0, 0.09)',
				'surface-sunken': 'rgba(0, 0, 0, 0.03)',
				'surface-strong': 'rgba(0, 0, 0, 0.22)'
			};
}

function theme(
	id: string,
	name: string,
	author: string,
	appearance: 'dark' | 'light',
	colors: Colors,
	terminal: TerminalPalette
): Theme {
	return { id, name, author, appearance, colors: { ...surfaces(appearance), ...colors }, terminal };
}

export const NORD = theme(
	'nord',
	'Nord',
	'Arctic Ice Studio',
	'dark',
	{
		'bg-primary': '#2e3440',
		'bg-secondary': '#292e39',
		'bg-elevated': '#3b4252',
		border: '#434c5e',
		'text-primary': '#eceff4',
		'text-secondary': '#c2c9d6',
		'text-tertiary': '#949eb2',
		accent: '#88c0d0',
		'accent-hover': '#8fbcbb',
		'accent-fill': '#4c6a92',
		success: '#a3be8c',
		warning: '#ebcb8b',
		danger: '#d57780',
		'surface-active': 'rgba(136, 192, 208, 0.18)',
		'accent-subtle': 'rgba(136, 192, 208, 0.16)'
	},
	{
		background: '#2e3440',
		foreground: '#d8dee9',
		cursor: '#d8dee9',
		cursorAccent: '#2e3440',
		selectionBackground: 'rgba(136, 192, 208, 0.30)',
		selectionForeground: '#eceff4',
		black: '#3b4252',
		red: '#bf616a',
		green: '#a3be8c',
		yellow: '#ebcb8b',
		blue: '#81a1c1',
		magenta: '#b48ead',
		cyan: '#88c0d0',
		white: '#e5e9f0',
		brightBlack: '#4c566a',
		brightRed: '#bf616a',
		brightGreen: '#a3be8c',
		brightYellow: '#ebcb8b',
		brightBlue: '#81a1c1',
		brightMagenta: '#b48ead',
		brightCyan: '#8fbcbb',
		brightWhite: '#eceff4'
	}
);

export const SOLARIZED_DARK = theme(
	'solarized-dark',
	'Solarized Dark',
	'Ethan Schoonover',
	'dark',
	{
		'bg-primary': '#002b36',
		'bg-secondary': '#00222b',
		'bg-elevated': '#073642',
		border: '#0d4553',
		'text-primary': '#eee8d5',
		'text-secondary': '#93a1a1',
		'text-tertiary': '#839496',
		accent: '#2aa1e8',
		'accent-hover': '#58b6ee',
		'accent-fill': '#1f6fa8',
		success: '#9cb200',
		warning: '#d4a20a',
		danger: '#ef5350',
		'accent-subtle': 'rgba(38, 139, 210, 0.22)'
	},
	{
		background: '#002b36',
		foreground: '#839496',
		cursor: '#93a1a1',
		cursorAccent: '#002b36',
		selectionBackground: 'rgba(147, 161, 161, 0.25)',
		selectionForeground: '#eee8d5',
		black: '#073642',
		red: '#dc322f',
		green: '#859900',
		yellow: '#b58900',
		blue: '#268bd2',
		magenta: '#d33682',
		cyan: '#2aa198',
		white: '#eee8d5',
		brightBlack: '#586e75',
		brightRed: '#cb4b16',
		brightGreen: '#586e75',
		brightYellow: '#657b83',
		brightBlue: '#839496',
		brightMagenta: '#6c71c4',
		brightCyan: '#93a1a1',
		brightWhite: '#fdf6e3'
	}
);

export const SOLARIZED_LIGHT = theme(
	'solarized-light',
	'Solarized Light',
	'Ethan Schoonover',
	'light',
	{
		'bg-primary': '#fdf6e3',
		'bg-secondary': '#eee8d5',
		'bg-elevated': '#fffbf0',
		border: '#ddd6c1',
		'text-primary': '#073642',
		'text-secondary': '#4f6168',
		'text-tertiary': '#657b83',
		accent: '#1a6fae',
		'accent-hover': '#155c91',
		'accent-fill': '#1a6fae',
		success: '#5f6e00',
		warning: '#8a6800',
		danger: '#c0271f',
		'accent-subtle': 'rgba(38, 139, 210, 0.14)'
	},
	{
		background: '#fdf6e3',
		foreground: '#586e75',
		cursor: '#586e75',
		cursorAccent: '#fdf6e3',
		selectionBackground: 'rgba(38, 139, 210, 0.22)',
		selectionForeground: '#073642',
		black: '#073642',
		red: '#dc322f',
		green: '#859900',
		yellow: '#b58900',
		blue: '#268bd2',
		magenta: '#d33682',
		cyan: '#2aa198',
		white: '#93a1a1',
		brightBlack: '#002b36',
		brightRed: '#cb4b16',
		brightGreen: '#586e75',
		brightYellow: '#657b83',
		brightBlue: '#839496',
		brightMagenta: '#6c71c4',
		brightCyan: '#93a1a1',
		brightWhite: '#073642'
	}
);

export const DRACULA = theme(
	'dracula',
	'Dracula',
	'Dracula Theme',
	'dark',
	{
		'bg-primary': '#282a36',
		'bg-secondary': '#21222c',
		'bg-elevated': '#343746',
		border: '#44475a',
		'text-primary': '#f8f8f2',
		'text-secondary': '#c3c6d4',
		'text-tertiary': '#9aa3c8',
		accent: '#bd93f9',
		'accent-hover': '#d0b0fb',
		'accent-fill': '#7144c2',
		success: '#50fa7b',
		warning: '#f1fa8c',
		danger: '#ff5555',
		'accent-subtle': 'rgba(189, 147, 249, 0.18)'
	},
	{
		background: '#282a36',
		foreground: '#f8f8f2',
		cursor: '#f8f8f2',
		cursorAccent: '#282a36',
		selectionBackground: 'rgba(68, 71, 90, 0.9)',
		selectionForeground: '#f8f8f2',
		black: '#21222c',
		red: '#ff5555',
		green: '#50fa7b',
		yellow: '#f1fa8c',
		blue: '#bd93f9',
		magenta: '#ff79c6',
		cyan: '#8be9fd',
		white: '#f8f8f2',
		brightBlack: '#6272a4',
		brightRed: '#ff6e6e',
		brightGreen: '#69ff94',
		brightYellow: '#ffffa5',
		brightBlue: '#d6acff',
		brightMagenta: '#ff92df',
		brightCyan: '#a4ffff',
		brightWhite: '#ffffff'
	}
);

export const CATPPUCCIN_MOCHA = theme(
	'catppuccin-mocha',
	'Catppuccin Mocha',
	'Catppuccin',
	'dark',
	{
		'bg-primary': '#1e1e2e',
		'bg-secondary': '#181825',
		'bg-elevated': '#313244',
		border: '#45475a',
		'text-primary': '#cdd6f4',
		'text-secondary': '#a6adc8',
		'text-tertiary': '#8a8fa8',
		accent: '#cba6f7',
		'accent-hover': '#b4befe',
		'accent-fill': '#7b3fd1',
		success: '#a6e3a1',
		warning: '#f9e2af',
		danger: '#f38ba8',
		'accent-subtle': 'rgba(203, 166, 247, 0.16)'
	},
	{
		background: '#1e1e2e',
		foreground: '#cdd6f4',
		cursor: '#f5e0dc',
		cursorAccent: '#1e1e2e',
		selectionBackground: 'rgba(88, 91, 112, 0.6)',
		selectionForeground: '#cdd6f4',
		black: '#45475a',
		red: '#f38ba8',
		green: '#a6e3a1',
		yellow: '#f9e2af',
		blue: '#89b4fa',
		magenta: '#f5c2e7',
		cyan: '#94e2d5',
		white: '#bac2de',
		brightBlack: '#585b70',
		brightRed: '#f38ba8',
		brightGreen: '#a6e3a1',
		brightYellow: '#f9e2af',
		brightBlue: '#89b4fa',
		brightMagenta: '#f5c2e7',
		brightCyan: '#94e2d5',
		brightWhite: '#a6adc8'
	}
);

export const CATPPUCCIN_LATTE = theme(
	'catppuccin-latte',
	'Catppuccin Latte',
	'Catppuccin',
	'light',
	{
		'bg-primary': '#eff1f5',
		'bg-secondary': '#e6e9ef',
		'bg-elevated': '#f7f8fa',
		border: '#ccd0da',
		'text-primary': '#4c4f69',
		'text-secondary': '#5c5f77',
		'text-tertiary': '#6c6f85',
		accent: '#8839ef',
		'accent-hover': '#7228d6',
		'accent-fill': '#8839ef',
		success: '#2f7a1f',
		warning: '#9a5b0c',
		danger: '#d20f39',
		'accent-subtle': 'rgba(136, 57, 239, 0.10)'
	},
	{
		background: '#eff1f5',
		foreground: '#4c4f69',
		cursor: '#dc8a78',
		cursorAccent: '#eff1f5',
		selectionBackground: 'rgba(124, 127, 147, 0.25)',
		selectionForeground: '#4c4f69',
		black: '#5c5f77',
		red: '#d20f39',
		green: '#40a02b',
		yellow: '#df8e1d',
		blue: '#1e66f5',
		magenta: '#ea76cb',
		cyan: '#179299',
		white: '#acb0be',
		brightBlack: '#6c6f85',
		brightRed: '#d20f39',
		brightGreen: '#40a02b',
		brightYellow: '#df8e1d',
		brightBlue: '#1e66f5',
		brightMagenta: '#ea76cb',
		brightCyan: '#179299',
		brightWhite: '#bcc0cc'
	}
);

export const GRUVBOX_DARK = theme(
	'gruvbox-dark',
	'Gruvbox Dark',
	'Pavel Pertsev',
	'dark',
	{
		'bg-primary': '#282828',
		'bg-secondary': '#1d2021',
		'bg-elevated': '#3c3836',
		border: '#504945',
		'text-primary': '#ebdbb2',
		'text-secondary': '#bdae93',
		'text-tertiary': '#a89984',
		accent: '#fe8019',
		'accent-hover': '#ff9a47',
		'accent-fill': '#af3a03',
		success: '#b8bb26',
		warning: '#fabd2f',
		danger: '#fb4934',
		'accent-subtle': 'rgba(254, 128, 25, 0.15)'
	},
	{
		background: '#282828',
		foreground: '#ebdbb2',
		cursor: '#ebdbb2',
		cursorAccent: '#282828',
		selectionBackground: 'rgba(102, 92, 84, 0.7)',
		selectionForeground: '#fbf1c7',
		black: '#282828',
		red: '#cc241d',
		green: '#98971a',
		yellow: '#d79921',
		blue: '#458588',
		magenta: '#b16286',
		cyan: '#689d6a',
		white: '#a89984',
		brightBlack: '#928374',
		brightRed: '#fb4934',
		brightGreen: '#b8bb26',
		brightYellow: '#fabd2f',
		brightBlue: '#83a598',
		brightMagenta: '#d3869b',
		brightCyan: '#8ec07c',
		brightWhite: '#ebdbb2'
	}
);

export const GRUVBOX_LIGHT = theme(
	'gruvbox-light',
	'Gruvbox Light',
	'Pavel Pertsev',
	'light',
	{
		'bg-primary': '#fbf1c7',
		'bg-secondary': '#f2e5bc',
		'bg-elevated': '#f9f5d7',
		border: '#d5c4a1',
		'text-primary': '#3c3836',
		'text-secondary': '#504945',
		'text-tertiary': '#665c54',
		accent: '#af3a03',
		'accent-hover': '#8f2f02',
		'accent-fill': '#af3a03',
		success: '#5f5b09',
		warning: '#8a5a0f',
		danger: '#9d0006',
		'accent-subtle': 'rgba(175, 58, 3, 0.10)'
	},
	{
		background: '#fbf1c7',
		foreground: '#3c3836',
		cursor: '#3c3836',
		cursorAccent: '#fbf1c7',
		selectionBackground: 'rgba(189, 174, 147, 0.6)',
		selectionForeground: '#282828',
		black: '#fbf1c7',
		red: '#cc241d',
		green: '#98971a',
		yellow: '#d79921',
		blue: '#458588',
		magenta: '#b16286',
		cyan: '#689d6a',
		white: '#7c6f64',
		brightBlack: '#928374',
		brightRed: '#9d0006',
		brightGreen: '#79740e',
		brightYellow: '#b57614',
		brightBlue: '#076678',
		brightMagenta: '#8f3f71',
		brightCyan: '#427b58',
		brightWhite: '#3c3836'
	}
);

export const TOKYO_NIGHT = theme(
	'tokyo-night',
	'Tokyo Night',
	'enkia',
	'dark',
	{
		'bg-primary': '#1a1b26',
		'bg-secondary': '#16161e',
		'bg-elevated': '#24283b',
		border: '#2f334d',
		'text-primary': '#c0caf5',
		'text-secondary': '#a9b1d6',
		'text-tertiary': '#828bb8',
		accent: '#7aa2f7',
		'accent-hover': '#9ab8ff',
		'accent-fill': '#3d59a1',
		success: '#9ece6a',
		warning: '#e0af68',
		danger: '#f7768e',
		'accent-subtle': 'rgba(122, 162, 247, 0.16)'
	},
	{
		background: '#1a1b26',
		foreground: '#c0caf5',
		cursor: '#c0caf5',
		cursorAccent: '#1a1b26',
		selectionBackground: 'rgba(51, 70, 124, 0.8)',
		selectionForeground: '#c0caf5',
		black: '#15161e',
		red: '#f7768e',
		green: '#9ece6a',
		yellow: '#e0af68',
		blue: '#7aa2f7',
		magenta: '#bb9af7',
		cyan: '#7dcfff',
		white: '#a9b1d6',
		brightBlack: '#414868',
		brightRed: '#ff899d',
		brightGreen: '#9fe044',
		brightYellow: '#faba4a',
		brightBlue: '#8db0ff',
		brightMagenta: '#c7a9ff',
		brightCyan: '#a4daff',
		brightWhite: '#c0caf5'
	}
);

export const TOKYO_NIGHT_DAY = theme(
	'tokyo-night-day',
	'Tokyo Night Day',
	'enkia',
	'light',
	{
		'bg-primary': '#e1e2e7',
		'bg-secondary': '#d6d8e0',
		'bg-elevated': '#ebecf0',
		border: '#c4c8da',
		'text-primary': '#343b58',
		'text-secondary': '#4c5374',
		'text-tertiary': '#5c6489',
		accent: '#2e5ec4',
		'accent-hover': '#244ea6',
		'accent-fill': '#2e5ec4',
		success: '#485f2f',
		warning: '#7a5d33',
		danger: '#c4204f',
		'accent-subtle': 'rgba(46, 125, 233, 0.14)'
	},
	{
		background: '#e1e2e7',
		foreground: '#3760bf',
		cursor: '#3760bf',
		cursorAccent: '#e1e2e7',
		selectionBackground: 'rgba(153, 167, 223, 0.6)',
		selectionForeground: '#3760bf',
		black: '#b4b5b9',
		red: '#f52a65',
		green: '#587539',
		yellow: '#8c6c3e',
		blue: '#2e7de9',
		magenta: '#9854f1',
		cyan: '#007197',
		white: '#6172b0',
		brightBlack: '#a1a6c5',
		brightRed: '#ff4774',
		brightGreen: '#5c8524',
		brightYellow: '#a27629',
		brightBlue: '#358aff',
		brightMagenta: '#a463ff',
		brightCyan: '#007ea8',
		brightWhite: '#3760bf'
	}
);

export const ONE_DARK = theme(
	'one-dark',
	'One Dark',
	'Atom',
	'dark',
	{
		'bg-primary': '#282c34',
		'bg-secondary': '#21252b',
		'bg-elevated': '#2f343e',
		border: '#3a3f4b',
		'text-primary': '#dcdfe4',
		'text-secondary': '#abb2bf',
		'text-tertiary': '#8b929e',
		accent: '#61afef',
		'accent-hover': '#82c0f2',
		'accent-fill': '#2f67c4',
		success: '#98c379',
		warning: '#e5c07b',
		danger: '#e06c75',
		'accent-subtle': 'rgba(97, 175, 239, 0.15)'
	},
	{
		background: '#282c34',
		foreground: '#abb2bf',
		cursor: '#528bff',
		cursorAccent: '#282c34',
		selectionBackground: 'rgba(62, 68, 81, 0.9)',
		selectionForeground: '#dcdfe4',
		black: '#3f4451',
		red: '#e06c75',
		green: '#98c379',
		yellow: '#e5c07b',
		blue: '#61afef',
		magenta: '#c678dd',
		cyan: '#56b6c2',
		white: '#d7dae0',
		brightBlack: '#5c6370',
		brightRed: '#ff7b86',
		brightGreen: '#b1e18b',
		brightYellow: '#efd08d',
		brightBlue: '#67cdff',
		brightMagenta: '#e48bff',
		brightCyan: '#63d4e0',
		brightWhite: '#ffffff'
	}
);

export const ONE_LIGHT = theme(
	'one-light',
	'One Light',
	'Atom',
	'light',
	{
		'bg-primary': '#fafafa',
		'bg-secondary': '#eaeaeb',
		'bg-elevated': '#ffffff',
		border: '#dbdbdc',
		'text-primary': '#383a42',
		'text-secondary': '#545862',
		'text-tertiary': '#696c77',
		accent: '#2f63d6',
		'accent-hover': '#2552b8',
		'accent-fill': '#2f63d6',
		success: '#3b7a3a',
		warning: '#8a5f00',
		danger: '#ca3b2e',
		'accent-subtle': 'rgba(64, 120, 242, 0.12)'
	},
	{
		background: '#fafafa',
		foreground: '#383a42',
		cursor: '#526fff',
		cursorAccent: '#fafafa',
		selectionBackground: 'rgba(64, 120, 242, 0.2)',
		selectionForeground: '#383a42',
		black: '#383a42',
		red: '#e45649',
		green: '#50a14f',
		yellow: '#c18401',
		blue: '#4078f2',
		magenta: '#a626a4',
		cyan: '#0184bc',
		white: '#a0a1a7',
		brightBlack: '#4f525e',
		brightRed: '#e06c75',
		brightGreen: '#98c379',
		brightYellow: '#e5c07b',
		brightBlue: '#61afef',
		brightMagenta: '#c678dd',
		brightCyan: '#56b6c2',
		brightWhite: '#202227'
	}
);

export const ROSE_PINE = theme(
	'rose-pine',
	'Rosé Pine',
	'Rosé Pine',
	'dark',
	{
		'bg-primary': '#191724',
		'bg-secondary': '#1f1d2e',
		'bg-elevated': '#26233a',
		border: '#403d52',
		'text-primary': '#e0def4',
		'text-secondary': '#b3afcc',
		'text-tertiary': '#908caa',
		accent: '#c4a7e7',
		'accent-hover': '#d6c2f0',
		'accent-fill': '#7a58b3',
		success: '#9ccfd8',
		warning: '#f6c177',
		danger: '#eb6f92',
		'accent-subtle': 'rgba(196, 167, 231, 0.15)'
	},
	{
		background: '#191724',
		foreground: '#e0def4',
		cursor: '#e0def4',
		cursorAccent: '#191724',
		selectionBackground: 'rgba(64, 61, 82, 0.9)',
		selectionForeground: '#e0def4',
		black: '#26233a',
		red: '#eb6f92',
		green: '#31748f',
		yellow: '#f6c177',
		blue: '#9ccfd8',
		magenta: '#c4a7e7',
		cyan: '#ebbcba',
		white: '#e0def4',
		brightBlack: '#6e6a86',
		brightRed: '#eb6f92',
		brightGreen: '#31748f',
		brightYellow: '#f6c177',
		brightBlue: '#9ccfd8',
		brightMagenta: '#c4a7e7',
		brightCyan: '#ebbcba',
		brightWhite: '#e0def4'
	}
);

export const ROSE_PINE_DAWN = theme(
	'rose-pine-dawn',
	'Rosé Pine Dawn',
	'Rosé Pine',
	'light',
	{
		'bg-primary': '#faf4ed',
		'bg-secondary': '#f2e9e1',
		'bg-elevated': '#fffaf3',
		border: '#dfdad9',
		'text-primary': '#575279',
		'text-secondary': '#625d82',
		'text-tertiary': '#797593',
		accent: '#286983',
		'accent-hover': '#1f5468',
		'accent-fill': '#286983',
		success: '#3f7780',
		warning: '#9c6417',
		danger: '#a8566d',
		'accent-subtle': 'rgba(40, 105, 131, 0.10)'
	},
	{
		background: '#faf4ed',
		foreground: '#575279',
		cursor: '#575279',
		cursorAccent: '#faf4ed',
		selectionBackground: 'rgba(223, 218, 217, 0.9)',
		selectionForeground: '#575279',
		black: '#f2e9e1',
		red: '#b4637a',
		green: '#286983',
		yellow: '#ea9d34',
		blue: '#56949f',
		magenta: '#907aa9',
		cyan: '#d7827e',
		white: '#575279',
		brightBlack: '#9893a5',
		brightRed: '#b4637a',
		brightGreen: '#286983',
		brightYellow: '#ea9d34',
		brightBlue: '#56949f',
		brightMagenta: '#907aa9',
		brightCyan: '#d7827e',
		brightWhite: '#575279'
	}
);

export const GITHUB_DARK = theme(
	'github-dark',
	'GitHub Dark',
	'GitHub (Primer)',
	'dark',
	{
		'bg-primary': '#0d1117',
		'bg-secondary': '#161b22',
		'bg-elevated': '#21262d',
		border: '#30363d',
		'text-primary': '#e6edf3',
		'text-secondary': '#9198a1',
		'text-tertiary': '#7d8590',
		accent: '#4493f8',
		'accent-hover': '#58a6ff',
		'accent-fill': '#1f6feb',
		success: '#3fb950',
		warning: '#d29922',
		danger: '#f85149',
		'accent-subtle': 'rgba(56, 139, 253, 0.15)'
	},
	{
		background: '#0d1117',
		foreground: '#e6edf3',
		cursor: '#2f81f7',
		cursorAccent: '#0d1117',
		selectionBackground: 'rgba(56, 139, 253, 0.4)',
		selectionForeground: '#e6edf3',
		black: '#484f58',
		red: '#ff7b72',
		green: '#3fb950',
		yellow: '#d29922',
		blue: '#58a6ff',
		magenta: '#bc8cff',
		cyan: '#39c5cf',
		white: '#b1bac4',
		brightBlack: '#6e7681',
		brightRed: '#ffa198',
		brightGreen: '#56d364',
		brightYellow: '#e3b341',
		brightBlue: '#79c0ff',
		brightMagenta: '#d2a8ff',
		brightCyan: '#56d4dd',
		brightWhite: '#ffffff'
	}
);

export const GITHUB_LIGHT = theme(
	'github-light',
	'GitHub Light',
	'GitHub (Primer)',
	'light',
	{
		'bg-primary': '#ffffff',
		'bg-secondary': '#f6f8fa',
		'bg-elevated': '#ffffff',
		border: '#d0d7de',
		'text-primary': '#1f2328',
		'text-secondary': '#59636e',
		'text-tertiary': '#656d76',
		accent: '#0969da',
		'accent-hover': '#0550ae',
		'accent-fill': '#0969da',
		success: '#1a7f37',
		warning: '#9a6700',
		danger: '#d1242f',
		'accent-subtle': 'rgba(9, 105, 218, 0.10)'
	},
	{
		background: '#ffffff',
		foreground: '#1f2328',
		cursor: '#0969da',
		cursorAccent: '#ffffff',
		selectionBackground: 'rgba(84, 174, 255, 0.4)',
		selectionForeground: '#1f2328',
		black: '#24292f',
		red: '#cf222e',
		green: '#116329',
		yellow: '#4d2d00',
		blue: '#0969da',
		magenta: '#8250df',
		cyan: '#1b7c83',
		white: '#6e7781',
		brightBlack: '#57606a',
		brightRed: '#a40e26',
		brightGreen: '#1a7f37',
		brightYellow: '#633c01',
		brightBlue: '#218bff',
		brightMagenta: '#a475f9',
		brightCyan: '#3192aa',
		brightWhite: '#8c959f'
	}
);

export const HIGH_CONTRAST_DARK = theme(
	'reacher-high-contrast-dark',
	'High Contrast Dark',
	'Reacher CM',
	'dark',
	{
		'bg-primary': '#000000',
		'bg-secondary': '#0b0b0b',
		'bg-elevated': '#161616',
		border: '#8a8a8a',
		'text-primary': '#ffffff',
		'text-secondary': '#e6e6e6',
		'text-tertiary': '#c2c2c2',
		accent: '#66b3ff',
		'accent-hover': '#99ccff',
		'accent-fill': '#0050b3',
		success: '#4ade80',
		warning: '#ffd23f',
		danger: '#ff7070',
		'surface-hover': 'rgba(255, 255, 255, 0.14)',
		'surface-active': 'rgba(255, 255, 255, 0.22)',
		'surface-sunken': 'rgba(255, 255, 255, 0.04)',
		'surface-strong': 'rgba(255, 255, 255, 0.45)',
		'border-strong': '#d0d0d0',
		'accent-subtle': 'rgba(102, 179, 255, 0.28)',
		focus: '#ffd23f'
	},
	{
		background: '#000000',
		foreground: '#ffffff',
		cursor: '#ffd23f',
		cursorAccent: '#000000',
		selectionBackground: 'rgba(102, 179, 255, 0.45)',
		selectionForeground: '#ffffff',
		black: '#4d4d4d',
		red: '#ff7070',
		green: '#4ade80',
		yellow: '#ffd23f',
		blue: '#66b3ff',
		magenta: '#ff7ef5',
		cyan: '#5ef1ff',
		white: '#e0e0e0',
		brightBlack: '#9a9a9a',
		brightRed: '#ff9e9e',
		brightGreen: '#8af5ad',
		brightYellow: '#ffe680',
		brightBlue: '#99ccff',
		brightMagenta: '#ffaaf8',
		brightCyan: '#9ef7ff',
		brightWhite: '#ffffff'
	}
);

export const HIGH_CONTRAST_LIGHT = theme(
	'reacher-high-contrast-light',
	'High Contrast Light',
	'Reacher CM',
	'light',
	{
		'bg-primary': '#ffffff',
		'bg-secondary': '#f0f0f0',
		'bg-elevated': '#ffffff',
		border: '#5c5c5c',
		'text-primary': '#000000',
		'text-secondary': '#1f1f1f',
		'text-tertiary': '#3d3d3d',
		accent: '#0040a0',
		'accent-hover': '#002f78',
		'accent-fill': '#0040a0',
		success: '#006b21',
		warning: '#7a4a00',
		danger: '#b00020',
		'surface-hover': 'rgba(0, 0, 0, 0.09)',
		'surface-active': 'rgba(0, 0, 0, 0.16)',
		'surface-sunken': 'rgba(0, 0, 0, 0.05)',
		'surface-strong': 'rgba(0, 0, 0, 0.5)',
		'border-strong': '#1f1f1f',
		'accent-subtle': 'rgba(0, 64, 160, 0.16)',
		focus: '#b34700'
	},
	{
		background: '#ffffff',
		foreground: '#000000',
		cursor: '#0040a0',
		cursorAccent: '#ffffff',
		selectionBackground: 'rgba(0, 64, 160, 0.25)',
		selectionForeground: '#000000',
		black: '#000000',
		red: '#b00020',
		green: '#006b21',
		yellow: '#6b4a00',
		blue: '#0040a0',
		magenta: '#8a0080',
		cyan: '#005f6b',
		white: '#595959',
		brightBlack: '#3d3d3d',
		brightRed: '#d0002a',
		brightGreen: '#00802a',
		brightYellow: '#855c00',
		brightBlue: '#0050c8',
		brightMagenta: '#a3009a',
		brightCyan: '#00738a',
		brightWhite: '#1a1a1a'
	}
);

/** In picker order: dark and light of a family side by side. */
export const BUNDLED: Theme[] = [
	NORD,
	SOLARIZED_DARK,
	SOLARIZED_LIGHT,
	DRACULA,
	CATPPUCCIN_MOCHA,
	CATPPUCCIN_LATTE,
	GRUVBOX_DARK,
	GRUVBOX_LIGHT,
	TOKYO_NIGHT,
	TOKYO_NIGHT_DAY,
	ONE_DARK,
	ONE_LIGHT,
	ROSE_PINE,
	ROSE_PINE_DAWN,
	GITHUB_DARK,
	GITHUB_LIGHT,
	HIGH_CONTRAST_DARK,
	HIGH_CONTRAST_LIGHT
];
