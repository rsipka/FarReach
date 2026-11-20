/**
 * Saved split layouts: the shape of a split view and what each pane showed,
 * to open again later. The rules, kept apart from the state so they can be
 * tested as they run.
 *
 * A pane keeps only what opens it again: a saved session's id, or a local
 * shell (with its WSL distro or program). Nothing about a session (host,
 * user, secret) is kept here. A tab that came from neither (Quick Connect,
 * say) cannot be opened again, so its pane is left out.
 */

import type { Branch, Layout, Node } from './split.js';

export type SavedLeaf =
	| { session: string; name?: string }
	| { local: true; wslDistro?: string; command?: { program: string; args: string[] } };

export type SavedNode = { kind: 'pane'; leaf: SavedLeaf } | { kind: 'split'; dir: 'row' | 'column'; ratio: number; a: SavedNode; b: SavedNode };

export interface SavedLayout {
	id: string;
	name: string;
	tree: SavedNode;
}

/** What a tab needs to say for it to be saved. */
export interface TabInfo {
	id: string;
	type: string;
	sessionId?: string;
	sessionName?: string;
	wslDistro?: string;
	localCommand?: { program: string; args: string[] };
}

export function leafOf(tab: TabInfo | undefined): SavedLeaf | null {
	if (!tab) return null;
	if (tab.sessionId) return { session: tab.sessionId, ...(tab.sessionName ? { name: tab.sessionName } : {}) };
	if (tab.type === 'local') {
		return { local: true, ...(tab.wslDistro ? { wslDistro: tab.wslDistro } : {}), ...(tab.localCommand ? { command: tab.localCommand } : {}) };
	}
	return null;
}

/** A split as it can be saved, its panes that cannot open again left out; null with fewer than two left. */
export function toSaved(layout: Layout, tabs: TabInfo[]): SavedNode | null {
	if (!layout) return null;
	const byId = new Map(tabs.map((t) => [t.id, t]));
	const walk = (node: Node): SavedNode | null => {
		if (node.kind === 'pane') {
			const leaf = leafOf(byId.get(node.id));
			return leaf ? { kind: 'pane', leaf } : null;
		}
		const a = walk(node.a);
		const b = walk(node.b);
		if (!a) return b;
		if (!b) return a;
		return { kind: 'split', dir: node.dir, ratio: node.ratio, a, b };
	};
	const tree = walk(layout);
	return tree && tree.kind === 'split' ? tree : null;
}

/** The panes of a saved layout, in order: slot i is the i-th pane. */
export function savedLeaves(tree: SavedNode): SavedLeaf[] {
	return tree.kind === 'pane' ? [tree.leaf] : [...savedLeaves(tree.a), ...savedLeaves(tree.b)];
}

/**
 * The split to show while a layout opens: slot i in the pane of tab
 * `slots[i]`. A slot not filled yet (a session still connecting, or one that
 * failed) is left out, and the panes beside it take its room.
 */
export function fill(tree: SavedNode, slots: (string | null)[]): Layout {
	let i = 0;
	const walk = (node: SavedNode): Node | null => {
		if (node.kind === 'pane') {
			const id = slots[i++];
			return id ? { kind: 'pane', id } : null;
		}
		const a = walk(node.a);
		const b = walk(node.b);
		if (!a) return b;
		if (!b) return a;
		return { kind: 'split', dir: node.dir, ratio: node.ratio, a, b } satisfies Branch;
	};
	const n = walk(tree);
	return n && n.kind === 'split' ? n : null;
}

/** The first empty slot a new tab from saved session `sessionId` fills, or -1. */
export function claim(leaves: SavedLeaf[], slots: (string | null)[], sessionId: string | undefined): number {
	if (!sessionId) return -1;
	return leaves.findIndex((l, i) => slots[i] === null && 'session' in l && l.session === sessionId);
}

/** A saved list read back: anything not shaped like a layout is dropped. */
export function parseLayouts(raw: unknown): SavedLayout[] {
	if (!Array.isArray(raw)) return [];
	const node = (n: unknown): n is SavedNode => {
		if (!n || typeof n !== 'object') return false;
		const o = n as Record<string, unknown>;
		if (o.kind === 'pane') {
			const l = o.leaf as Record<string, unknown> | undefined;
			return !!l && (typeof l.session === 'string' || l.local === true);
		}
		return o.kind === 'split' && (o.dir === 'row' || o.dir === 'column') && typeof o.ratio === 'number' && node(o.a) && node(o.b);
	};
	return raw.filter(
		(x): x is SavedLayout => !!x && typeof x === 'object' && typeof (x as SavedLayout).id === 'string' && typeof (x as SavedLayout).name === 'string' && node((x as SavedLayout).tree)
	);
}
