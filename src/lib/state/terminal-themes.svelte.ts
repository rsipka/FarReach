/**
 * Terminal colours, chosen apart from the app's theme.
 *
 * A terminal theme is a palette with an id:
 *   - `app:<theme id>`  the palette an app theme carries (Reach Dark, an
 *                       installed marketplace theme...);
 *   - `scheme:<name>`   one of the schemes bundled from iTerm2-Color-Schemes
 *                       (#lib/data/terminal-schemes.json);
 *   - `custom:<uuid>`   one the user made: a copy of another, edited, or a
 *                       scheme file they imported or downloaded.
 *
 * The default terminal theme is a setting of this device (`terminalThemeId`,
 * empty: follow the app theme). A session can name its own
 * (`terminal_theme`), which syncs with it; custom themes are kept in the
 * settings vault, so they sync too and a session's custom theme is found on
 * every device. A theme that cannot be found (deleted, not synced yet) falls
 * back to the default, and that to the app theme.
 */

import schemes from '#lib/data/terminal-schemes.json';
import { getSettings } from '#lib/state/settings.svelte.js';
import { themeState, type TerminalPalette } from '#lib/state/theme.svelte.js';
import * as settingsIpc from '#lib/ipc/settings.js';
import { invoke } from '@tauri-apps/api/core';
import {
	PALETTE_ORDER,
	isHexColour,
	isLightPalette,
	isSafeSchemeName,
	parseItermColors,
	parseSchemeFile,
	unpackPalette,
	type ParsedScheme
} from '#lib/terminal/color-schemes.js';

export type TerminalThemeKind = 'app' | 'scheme' | 'custom';

export interface TerminalTheme {
	id: string;
	name: string;
	kind: TerminalThemeKind;
	palette: TerminalPalette;
	light: boolean;
}

/** What is stored for a custom theme. */
export interface CustomTerminalTheme {
	id: string;
	name: string;
	palette: TerminalPalette;
}

/** The settings vault key the custom themes are stored under, as JSON. */
const STORE_KEY = 'terminal_themes';

/** Where the bundled schemes come from, and the ones only listed by name. */
export const SCHEMES_SOURCE: string = schemes.source;
export const DOWNLOADABLE_SCHEMES: string[] = schemes.more;

const BUNDLED: TerminalTheme[] = schemes.schemes.flatMap(([name, packed]) => {
	try {
		const palette = unpackPalette(packed);
		return [{ id: `scheme:${name}`, name, kind: 'scheme' as const, palette, light: isLightPalette(palette) }];
	} catch {
		return [];
	}
});

class TerminalThemeState {
	custom = $state<CustomTerminalTheme[]>([]);
	/** Read from the vault since it last opened. Saving before would overwrite
	 *  the stored list with an empty one. */
	loaded = $state(false);
}

export const terminalThemes = new TerminalThemeState();

function fromCustom(c: CustomTerminalTheme): TerminalTheme {
	return { id: c.id, name: c.name, kind: 'custom', palette: c.palette, light: isLightPalette(c.palette) };
}

function fromApp(id: string): TerminalTheme | undefined {
	const theme = themeState.all.find((t) => t.id === id);
	if (!theme) return undefined;
	return { id: `app:${theme.id}`, name: theme.name, kind: 'app', palette: theme.terminal, light: theme.appearance === 'light' };
}

/** Every terminal theme: the user's own first, then the app themes', then the bundled schemes. */
export function allTerminalThemes(): TerminalTheme[] {
	return [
		...terminalThemes.custom.map(fromCustom),
		...themeState.all.map((t) => fromApp(t.id)!).filter(Boolean),
		...BUNDLED
	];
}

/** A terminal theme by id, or undefined when there is none such here. */
export function findTerminalTheme(id: string | null | undefined): TerminalTheme | undefined {
	if (!id) return undefined;
	if (id.startsWith('custom:')) {
		const c = terminalThemes.custom.find((t) => t.id === id);
		return c ? fromCustom(c) : undefined;
	}
	if (id.startsWith('app:')) return fromApp(id.slice(4));
	return BUNDLED.find((t) => t.id === id);
}

/**
 * The palette a terminal paints with: its session's theme, else the default
 * terminal theme, else the app theme's own. Reactive: read inside an effect
 * or $derived, it follows each of the three.
 */
export function terminalPalette(sessionThemeId?: string | null): TerminalPalette {
	return (
		findTerminalTheme(sessionThemeId)?.palette ??
		findTerminalTheme(getSettings().terminalThemeId)?.palette ??
		themeState.current.terminal
	);
}

function validCustom(x: unknown): x is CustomTerminalTheme {
	if (!x || typeof x !== 'object') return false;
	const c = x as CustomTerminalTheme;
	return (
		typeof c.id === 'string' &&
		c.id.startsWith('custom:') &&
		typeof c.name === 'string' &&
		!!c.palette &&
		PALETTE_ORDER.every((k) => isHexColour(c.palette[k]))
	);
}

/** Read the custom themes from the settings vault. Call once it is open. */
export async function loadCustomTerminalThemes(): Promise<void> {
	try {
		const raw = await settingsIpc.get(STORE_KEY);
		const list: unknown = raw ? JSON.parse(raw) : [];
		terminalThemes.custom = Array.isArray(list) ? list.filter(validCustom) : [];
		terminalThemes.loaded = true;
	} catch (err) {
		console.error('Could not read the custom terminal themes:', err);
	}
}

async function store(next: CustomTerminalTheme[]): Promise<void> {
	if (!terminalThemes.loaded) throw new Error('Unlock the vault first');
	await settingsIpc.set(STORE_KEY, JSON.stringify(next));
	terminalThemes.custom = next;
}

/** Add or replace a custom theme. */
export async function saveCustomTerminalTheme(theme: CustomTerminalTheme): Promise<void> {
	if (!validCustom(theme)) throw new Error('Every colour must be #rrggbb');
	const clean: CustomTerminalTheme = {
		id: theme.id,
		name: theme.name.trim() || 'Custom',
		palette: Object.fromEntries(PALETTE_ORDER.map((k) => [k, theme.palette[k].toLowerCase()])) as unknown as TerminalPalette
	};
	const i = terminalThemes.custom.findIndex((t) => t.id === theme.id);
	const next = [...terminalThemes.custom];
	if (i === -1) next.unshift(clean);
	else next[i] = clean;
	await store(next);
}

/** Remove a custom theme. Sessions that use it fall back to the default. */
export async function deleteCustomTerminalTheme(id: string): Promise<void> {
	await store(terminalThemes.custom.filter((t) => t.id !== id));
}

/** A name not taken by another custom theme ("Homebrew", "Homebrew 2"...). */
function freeName(name: string): string {
	const taken = new Set(terminalThemes.custom.map((t) => t.name));
	if (!taken.has(name)) return name;
	let n = 2;
	while (taken.has(`${name} ${n}`)) n++;
	return `${name} ${n}`;
}

/** A new, unsaved custom theme with another theme's colours. */
export function copyOfTerminalTheme(from: Pick<TerminalTheme, 'name' | 'palette'>, name?: string): CustomTerminalTheme {
	return { id: `custom:${crypto.randomUUID()}`, name: freeName(name ?? from.name), palette: { ...from.palette } };
}

/** Save the schemes of a picked file as custom themes; returns them. */
export async function importTerminalSchemes(text: string, fileName: string): Promise<CustomTerminalTheme[]> {
	const added = parseSchemeFile(text, fileName).map((s: ParsedScheme) => copyOfTerminalTheme(s));
	if (added.length === 0) return [];
	await store([...added, ...terminalThemes.custom]);
	return added;
}

/** Download one of the collection's other schemes by name and save it as a custom theme. */
export async function downloadTerminalScheme(name: string): Promise<CustomTerminalTheme> {
	if (!isSafeSchemeName(name)) throw new Error('Not a colour scheme name');
	const text = await invoke<string>('terminal_scheme_download', { name });
	const theme = copyOfTerminalTheme({ name, palette: parseItermColors(text) });
	await store([theme, ...terminalThemes.custom]);
	return theme;
}
