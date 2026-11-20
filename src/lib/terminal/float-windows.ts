/**
 * Floating session windows: the rules, kept apart from the Tauri glue in
 * #lib/state/windows so they can be tested as they run.
 *
 * A tab moves from one window to another without its connection noticing:
 * the connection lives in the backend, whose output reaches every window. The
 * terminal on screen does not: its lines live in the page. So the window
 * taking a tab starts listening first, holding what arrives; then it asks the
 * window giving it up for the terminal as it stands (its screen and
 * scrollback, colours kept), which stops listening at that moment. What the
 * new window holds can begin a little before that moment, so the chunks both
 * saw are dropped once (see overlap).
 *
 * Window memory: where a window was is kept as the monitor it was on (by
 * name and size) and its place on that monitor, so it comes back on the same
 * screen, or on the main one when that screen is gone.
 */

import { leaves, remove, type Layout } from '#lib/terminal/split.js';
import { leafOf, savedLeaves, type SavedLeaf, type SavedNode, type TabInfo } from '#lib/terminal/layouts.js';

/** How many recent output chunks a terminal keeps to match against. */
export const TAIL_CHUNKS = 64;

/** A chunk of terminal output as an event carries it. */
export type Chunk = number[] | Uint8Array | string;

/** A chunk as text that compares equal whenever the chunks are the same. */
export function chunkKey(c: Chunk): string {
	if (typeof c === 'string') return 's' + c;
	let out = 'b';
	const bytes = c instanceof Uint8Array ? c : Uint8Array.from(c);
	for (let i = 0; i < bytes.length; i += 4096) {
		out += String.fromCharCode(...bytes.subarray(i, i + 4096));
	}
	return out;
}

/**
 * How many chunks at the start of `held` the giving window already showed:
 * the longest run that ends `tail` and begins `held`. Both windows get the
 * same events in the same order, so that run is the part both saw.
 */
export function overlap(tail: string[], held: string[]): number {
	const max = Math.min(tail.length, held.length);
	for (let k = max; k > 0; k--) {
		let same = true;
		for (let i = 0; i < k; i++) {
			if (tail[tail.length - k + i] !== held[i]) {
				same = false;
				break;
			}
		}
		if (same) return k;
	}
	return 0;
}

/** The split shown in the window a set of tabs moves to: their panes, kept as they were. */
export function subLayout(layout: Layout, ids: string[]): Layout {
	const keep = new Set(ids);
	let out = layout;
	for (const id of leaves(layout)) if (!keep.has(id)) out = remove(out, id).layout;
	return out;
}

/** The split left behind once some tabs have gone. */
export function withoutTabs(layout: Layout, ids: string[]): Layout {
	let out = layout;
	for (const id of ids) out = remove(out, id).layout;
	return out;
}

// ---- Window memory -------------------------------------------------------

export interface Rect {
	x: number;
	y: number;
	width: number;
	height: number;
}

/** A screen, in physical pixels as Tauri gives it. */
export interface MonitorInfo {
	name: string | null;
	position: { x: number; y: number };
	size: { width: number; height: number };
	scaleFactor: number;
}

/** Where a window was: its monitor, and its place on it (physical pixels). */
export interface Placement {
	monitor: { name: string | null; width: number; height: number } | null;
	/** Relative to the monitor's top left corner. */
	x: number;
	y: number;
	width: number;
	height: number;
	maximized: boolean;
}

/** The smallest window kept or opened, in physical pixels. */
export const MIN_WIDTH = 480;
export const MIN_HEIGHT = 320;

function within(r: Rect, m: MonitorInfo): boolean {
	const cx = r.x + r.width / 2;
	const cy = r.y + r.height / 2;
	return cx >= m.position.x && cx < m.position.x + m.size.width && cy >= m.position.y && cy < m.position.y + m.size.height;
}

/** A window's place as it is kept: on the monitor its middle is on. */
export function toPlacement(window: Rect, maximized: boolean, monitors: MonitorInfo[]): Placement {
	const m = monitors.find((x) => within(window, x)) ?? null;
	return {
		monitor: m ? { name: m.name, width: m.size.width, height: m.size.height } : null,
		x: m ? window.x - m.position.x : window.x,
		y: m ? window.y - m.position.y : window.y,
		width: Math.round(window.width),
		height: Math.round(window.height),
		maximized
	};
}

/** The monitor a kept place belongs on: the same name and size, the same name, else the first (the main one). */
export function monitorFor(p: Placement, monitors: MonitorInfo[]): MonitorInfo | null {
	if (monitors.length === 0) return null;
	const want = p.monitor;
	if (want) {
		const exact = monitors.find((m) => m.name === want.name && m.size.width === want.width && m.size.height === want.height);
		if (exact) return exact;
		const named = want.name ? monitors.find((m) => m.name === want.name) : undefined;
		if (named) return named;
	}
	return monitors[0];
}

/**
 * Where to open a window kept at `p`: on its monitor, inside it. A monitor
 * that changed size scales the place with it; a window bigger than the
 * screen is made to fit.
 */
export function restoreRect(p: Placement, monitors: MonitorInfo[]): Rect & { maximized: boolean } {
	const m = monitorFor(p, monitors);
	if (!m) return { x: p.x, y: p.y, width: Math.max(MIN_WIDTH, p.width), height: Math.max(MIN_HEIGHT, p.height), maximized: p.maximized };
	const sameSize = p.monitor && p.monitor.width === m.size.width && p.monitor.height === m.size.height;
	const sx = !sameSize && p.monitor && p.monitor.width > 0 ? m.size.width / p.monitor.width : 1;
	const sy = !sameSize && p.monitor && p.monitor.height > 0 ? m.size.height / p.monitor.height : 1;
	const width = Math.min(m.size.width, Math.max(MIN_WIDTH, Math.round(p.width * sx)));
	const height = Math.min(m.size.height, Math.max(MIN_HEIGHT, Math.round(p.height * sy)));
	const x = Math.min(m.size.width - width, Math.max(0, Math.round(p.x * sx)));
	const y = Math.min(m.size.height - height, Math.max(0, Math.round(p.y * sy)));
	return { x: m.position.x + x, y: m.position.y + y, width, height, maximized: p.maximized };
}

/**
 * A new window opened by dragging a tab out: under the pointer (physical
 * screen pixels), the size the last one had, kept on the screen it lands on.
 */
export function rectAt(point: { x: number; y: number }, size: { width: number; height: number }, monitors: MonitorInfo[]): Rect {
	const width = Math.max(MIN_WIDTH, size.width);
	const height = Math.max(MIN_HEIGHT, size.height);
	// The tab bar under the pointer, as if the window were picked up by it.
	let x = Math.round(point.x - Math.min(160, width / 4));
	let y = Math.round(point.y - 50);
	const m = monitors.find((mm) => within({ x: point.x, y: point.y, width: 0, height: 0 }, mm));
	if (m) {
		x = Math.min(m.position.x + m.size.width - Math.min(width, m.size.width), Math.max(m.position.x, x));
		y = Math.min(m.position.y + m.size.height - Math.min(height, m.size.height), Math.max(m.position.y, y));
	}
	return { x, y, width, height };
}

// ---- Reopening windows at start -----------------------------------------

/** A floating window as kept for the next start: where it was and what it showed. */
export interface KeptWindow {
	placement: Placement | null;
	/** Its tabs that can open again, in bar order. */
	tabs: SavedLeaf[];
	/** Its split, if it had one. */
	tree: SavedNode | null;
}

/** What a window keeps of its tabs: only those that can open again (see leafOf). */
export function keepWindow(tabs: TabInfo[], split: SavedNode | null, placement: Placement | null): KeptWindow | null {
	const inSplit = split ? savedLeaves(split).length : 0;
	const loose: SavedLeaf[] = [];
	for (const t of tabs) {
		const leaf = leafOf(t);
		if (leaf) loose.push(leaf);
	}
	if (loose.length === 0 && inSplit === 0) return null;
	return { placement, tabs: loose, tree: split };
}

/**
 * The tabs a kept window opens: its split's panes first (in their order), then
 * any other tab it had. A tab in the split is also in `tabs`; each is opened once.
 */
export function windowLeaves(w: KeptWindow): { leaves: SavedLeaf[]; inTree: number } {
	const tree = w.tree ? savedLeaves(w.tree) : [];
	const rest = [...w.tabs];
	for (const leaf of tree) {
		const i = rest.findIndex((x) => sameLeaf(x, leaf));
		if (i !== -1) rest.splice(i, 1);
	}
	return { leaves: [...tree, ...rest], inTree: tree.length };
}

function sameLeaf(a: SavedLeaf, b: SavedLeaf): boolean {
	if ('session' in a && 'session' in b) return a.session === b.session;
	if ('local' in a && 'local' in b) return a.wslDistro === b.wslDistro && JSON.stringify(a.command ?? null) === JSON.stringify(b.command ?? null);
	return false;
}

function isLeaf(l: unknown): l is SavedLeaf {
	if (!l || typeof l !== 'object') return false;
	const o = l as Record<string, unknown>;
	return typeof o.session === 'string' || o.local === true;
}

function isNode(n: unknown): n is SavedNode {
	if (!n || typeof n !== 'object') return false;
	const o = n as Record<string, unknown>;
	if (o.kind === 'pane') return isLeaf(o.leaf);
	return o.kind === 'split' && (o.dir === 'row' || o.dir === 'column') && typeof o.ratio === 'number' && isNode(o.a) && isNode(o.b);
}

function isPlacement(p: unknown): p is Placement {
	if (!p || typeof p !== 'object') return false;
	const o = p as Record<string, unknown>;
	const num = (v: unknown) => typeof v === 'number' && Number.isFinite(v);
	const m = o.monitor as Record<string, unknown> | null | undefined;
	const monitorOk = m === null || (!!m && typeof m === 'object' && (m.name === null || typeof m.name === 'string') && num(m.width) && num(m.height));
	return monitorOk && num(o.x) && num(o.y) && num(o.width) && num(o.height) && typeof o.maximized === 'boolean';
}

export function parsePlacement(raw: unknown): Placement | null {
	return isPlacement(raw) ? raw : null;
}

/** Kept windows read back: anything not shaped like one is dropped. */
export function parseKept(raw: unknown): KeptWindow[] {
	if (!raw || typeof raw !== 'object') return [];
	const list = Array.isArray(raw) ? raw : Object.values(raw as Record<string, unknown>);
	const out: KeptWindow[] = [];
	for (const x of list) {
		if (!x || typeof x !== 'object') continue;
		const o = x as Record<string, unknown>;
		const tabs = Array.isArray(o.tabs) ? o.tabs.filter(isLeaf) : [];
		const tree = isNode(o.tree) ? o.tree : null;
		if (tabs.length === 0 && !tree) continue;
		out.push({ placement: parsePlacement(o.placement), tabs, tree });
	}
	return out;
}
