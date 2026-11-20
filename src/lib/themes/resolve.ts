import type { Theme } from '#lib/state/theme.svelte.js';

/**
 * Which app theme is in use, from the settings and the system appearance.
 *
 * Kept free of Svelte and Tauri so scripts/theme-pair-test.mjs can run it in
 * Node.
 *
 * - A theme picked by id wins (`themeId`).
 * - Otherwise "System" (`theme: 'system'`) follows the OS: the light theme of
 *   the pair when it is light, the dark one when it is dark. The pair is
 *   `themeLight` / `themeDark`, Reacher CM Light / Dark when unset or when the
 *   theme named there is gone (an uninstalled marketplace theme).
 * - Settings from before themes had ids say only 'light' or 'dark'.
 */
export interface ThemeChoice {
	theme: 'dark' | 'light' | 'system';
	themeId?: string;
	themeLight?: string;
	themeDark?: string;
}

export function resolveTheme(
	s: ThemeChoice,
	prefersDark: boolean,
	all: Theme[],
	fallback: { light: Theme; dark: Theme }
): Theme {
	const byId = (id: string | undefined) => (id ? all.find((t) => t.id === id) : undefined);
	const chosen = byId(s.themeId);
	if (chosen) return chosen;
	if (s.theme === 'system') return pairTheme(s, prefersDark, all, fallback);
	return s.theme === 'light' ? fallback.light : fallback.dark;
}

/** The half of the System pair for this appearance. */
export function pairTheme(
	s: Pick<ThemeChoice, 'themeLight' | 'themeDark'>,
	dark: boolean,
	all: Theme[],
	fallback: { light: Theme; dark: Theme }
): Theme {
	const id = dark ? s.themeDark : s.themeLight;
	return (id && all.find((t) => t.id === id)) || (dark ? fallback.dark : fallback.light);
}
