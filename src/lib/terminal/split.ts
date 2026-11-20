/**
 * Tabs side by side, split any way (issue #106, nested): the rules, kept
 * apart from the state so they can be tested as they run.
 *
 * A layout is a tree. A leaf is a pane showing one tab; a split node puts two
 * subtrees side by side (`row`: `a` left, `b` right) or one above the other
 * (`column`: `a` on top, `b` below), with `ratio` the share `a` gets. A
 * layout always holds at least two panes; one pane left is no split at all
 * (`null`). Any tab can be in it (terminals, remote desktops), each at most
 * once.
 *
 * The active tab is the one with focus, always one of the panes while a split
 * is open. Choosing another tab in the bar puts it in the focused pane, so the
 * others stay where they were; closing a tab in it takes its pane out, and the
 * pane beside it gets the room.
 */

/** The two ways the keyboard and menus split: beside or below. */
export type Direction = 'right' | 'down';
export type Side = 'left' | 'right' | 'top' | 'bottom';
/** Where a dragged tab lands on a pane: one of its sides, or in it. */
export type Zone = Side | 'center';

export interface Pane {
	kind: 'pane';
	id: string;
}

export interface Branch {
	kind: 'split';
	dir: 'row' | 'column';
	/** The share of `a`, 0..1. */
	ratio: number;
	a: Node;
	b: Node;
}

export type Node = Pane | Branch;
/** A split view, or `null` for one tab at a time. */
export type Layout = Branch | null;

/** Below this window width only the focused pane is shown; the split comes back with the room. */
export const SPLIT_MIN_WIDTH = 900;

/** The divider stays where both sides are still usable. */
export const MIN_RATIO = 0.1;
export const MAX_RATIO = 0.9;

/** A pane is not split further below this size, in pixels, so each half stays usable. */
export const MIN_PANE_WIDTH = 220;
export const MIN_PANE_HEIGHT = 120;

export function clampRatio(r: number): number {
	if (!Number.isFinite(r)) return 0.5;
	return Math.min(MAX_RATIO, Math.max(MIN_RATIO, r));
}

export function sideOf(direction: Direction): Side {
	return direction === 'right' ? 'right' : 'bottom';
}

const pane = (id: string): Pane => ({ kind: 'pane', id });

/** The tabs in a layout, left to right and top to bottom. */
export function leaves(node: Node | null): string[] {
	if (!node) return [];
	if (node.kind === 'pane') return [node.id];
	return [...leaves(node.a), ...leaves(node.b)];
}

export function contains(node: Node | null, id: string): boolean {
	if (!node) return false;
	if (node.kind === 'pane') return node.id === id;
	return contains(node.a, id) || contains(node.b, id);
}

/** Two nodes side by side: `incoming` on `side` of `existing`. */
function pair(existing: Node, incoming: Node, side: Side, ratio = 0.5): Branch {
	const dir = side === 'left' || side === 'right' ? 'row' : 'column';
	const first = side === 'left' || side === 'top';
	return { kind: 'split', dir, ratio: clampRatio(ratio), a: first ? incoming : existing, b: first ? existing : incoming };
}

/** The layout without `id`'s pane: its sibling takes the room. */
function without(node: Node, id: string): Node | null {
	if (node.kind === 'pane') return node.id === id ? null : node;
	const a = without(node.a, id);
	const b = without(node.b, id);
	if (!a) return b;
	if (!b) return a;
	if (a === node.a && b === node.b) return node;
	return { ...node, a, b };
}

function asLayout(node: Node | null): Layout {
	return node && node.kind === 'split' ? node : null;
}

/** The first pane of a subtree: the one that takes focus when its sibling goes. */
function firstLeaf(node: Node): string {
	return node.kind === 'pane' ? node.id : firstLeaf(node.a);
}

/** The node `id`'s pane is a child of, and on which side. */
function parentOf(node: Node, id: string): { parent: Branch; side: 'a' | 'b' } | null {
	if (node.kind === 'pane') return null;
	if (node.a.kind === 'pane' && node.a.id === id) return { parent: node, side: 'a' };
	if (node.b.kind === 'pane' && node.b.id === id) return { parent: node, side: 'b' };
	return parentOf(node.a, id) ?? parentOf(node.b, id);
}

/** A copy with `fn` applied to the pane of `id`. */
function mapPane(node: Node, id: string, fn: (p: Pane) => Node): Node {
	if (node.kind === 'pane') return node.id === id ? fn(node) : node;
	const a = mapPane(node.a, id, fn);
	const b = mapPane(node.b, id, fn);
	return a === node.a && b === node.b ? node : { ...node, a, b };
}

/**
 * Put `tabId` on `side` of the pane showing `target`. With no split yet,
 * `target` is the tab on screen. A tab already in the layout moves (its old
 * place closes up). Nothing to split with itself.
 */
export function insert(layout: Layout, target: string, tabId: string, side: Side): Layout {
	if (target === tabId) return layout;
	if (!layout) return pair(pane(target), pane(tabId), side);
	if (!contains(layout, target)) return layout;
	const rest = without(layout, tabId);
	if (!rest) return layout;
	return asLayout(mapPane(rest, target, (p) => pair(p, pane(tabId), side)));
}

/** Put `tabId` along a whole side of the view, beside everything else. */
export function insertAtEdge(layout: Layout, tabId: string, side: Side): Layout {
	if (!layout) return null;
	const rest = without(layout, tabId);
	if (!rest || rest.kind === 'pane') {
		return rest ? pair(rest, pane(tabId), side) : layout;
	}
	return pair(rest, pane(tabId), side, side === 'left' || side === 'top' ? 0.3 : 0.7);
}

/** A tab closing or taken out: its pane goes; `focus` is who has it next, if it was focused. */
export function remove(layout: Layout, id: string): { layout: Layout; focus?: string } {
	if (!layout || !contains(layout, id)) return { layout };
	const p = parentOf(layout, id);
	const sibling = p ? (p.side === 'a' ? p.parent.b : p.parent.a) : null;
	return { layout: asLayout(without(layout, id)), focus: sibling ? firstLeaf(sibling) : undefined };
}

/** One of the panes gets focus; any other tab replaces the focused pane. */
export function activate(layout: Layout, focusedId: string | undefined, tabId: string): Layout {
	if (!layout || contains(layout, tabId) || !focusedId || !contains(layout, focusedId)) return layout;
	return asLayout(mapPane(layout, focusedId, () => pane(tabId)));
}

/** `tabId` shown in `target`'s pane: two panes trade places, or a tab not shown takes it. */
export function place(layout: Layout, target: string, tabId: string): Layout {
	if (!layout || target === tabId || !contains(layout, target)) return layout;
	if (!contains(layout, tabId)) return asLayout(mapPane(layout, target, () => pane(tabId)));
	const swapped = mapPane(mapPane(layout, target, () => pane('\u0000')), tabId, () => pane(target));
	return asLayout(mapPane(swapped, '\u0000', () => pane(tabId)));
}

/** A split node by its path from the root (`a`/`b` steps), for its divider. */
export function setRatio(layout: Layout, path: string, ratio: number): Layout {
	if (!layout) return layout;
	const go = (node: Node, rest: string): Node => {
		if (node.kind === 'pane') return node;
		if (!rest) return { ...node, ratio: clampRatio(ratio) };
		return rest[0] === 'a' ? { ...node, a: go(node.a, rest.slice(1)) } : { ...node, b: go(node.b, rest.slice(1)) };
	};
	return asLayout(go(layout, path));
}

/** A box inside the terminal area, in fractions of it. */
export interface Rect {
	x: number;
	y: number;
	w: number;
	h: number;
}

export interface PaneRect extends Rect {
	id: string;
}

export interface DividerRect {
	/** Which split node it moves (see setRatio). */
	path: string;
	dir: 'row' | 'column';
	ratio: number;
	/** The box of the node it divides, to turn a pointer into a ratio. */
	box: Rect;
}

/** Where every pane and divider sits. */
export function geometry(layout: Layout): { panes: PaneRect[]; dividers: DividerRect[] } {
	const panes: PaneRect[] = [];
	const dividers: DividerRect[] = [];
	const walk = (node: Node, box: Rect, path: string) => {
		if (node.kind === 'pane') {
			panes.push({ id: node.id, ...box });
			return;
		}
		dividers.push({ path, dir: node.dir, ratio: node.ratio, box });
		if (node.dir === 'row') {
			const w = box.w * node.ratio;
			walk(node.a, { x: box.x, y: box.y, w, h: box.h }, path + 'a');
			walk(node.b, { x: box.x + w, y: box.y, w: box.w - w, h: box.h }, path + 'b');
		} else {
			const h = box.h * node.ratio;
			walk(node.a, { x: box.x, y: box.y, w: box.w, h }, path + 'a');
			walk(node.b, { x: box.x, y: box.y + h, w: box.w, h: box.h - h }, path + 'b');
		}
	};
	if (layout) walk(layout, { x: 0, y: 0, w: 1, h: 1 }, '');
	return { panes, dividers };
}

/** Half the divider's thickness, in pixels, kept clear on each side of it. */
const HALF_GAP = 3;
const EPS = 1e-6;
const pct = (f: number) => `${(f * 100).toFixed(3)}%`;

/** Where a pane sits inside the terminal area, as inline CSS: a gap on each side that meets another pane. */
export function paneStyle(r: Rect): string {
	const edge = (at: number, inner: boolean, sign: '+' | '-') => (inner ? `calc(${pct(at)} ${sign} ${HALF_GAP}px)` : pct(at));
	const right = 1 - r.x - r.w;
	const bottom = 1 - r.y - r.h;
	return [
		`left:${edge(r.x, r.x > EPS, '+')}`,
		`top:${edge(r.y, r.y > EPS, '+')}`,
		`right:${edge(right, right > EPS, '+')}`,
		`bottom:${edge(bottom, bottom > EPS, '+')}`,
		'width:auto;height:auto'
	].join(';');
}

/** A divider's place, as inline CSS. */
export function dividerStyle(d: DividerRect): string {
	const { box } = d;
	if (d.dir === 'row') {
		const at = box.x + box.w * d.ratio;
		return `left:calc(${pct(at)} - ${HALF_GAP}px);width:${HALF_GAP * 2}px;top:${pct(box.y)};height:${pct(box.h)}`;
	}
	const at = box.y + box.h * d.ratio;
	return `top:calc(${pct(at)} - ${HALF_GAP}px);height:${HALF_GAP * 2}px;left:${pct(box.x)};width:${pct(box.w)}`;
}

/** The ratio a divider gets with the pointer at (fx, fy), in fractions of the area. */
export function ratioAt(d: DividerRect, fx: number, fy: number): number {
	return clampRatio(d.dir === 'row' ? (fx - d.box.x) / d.box.w : (fy - d.box.y) / d.box.h);
}

/** Within this many pixels of the area's edge, a drop goes along that whole side. */
export const EDGE_BAND = 24;

export interface Drop {
	/** The pane it lands on; `null`: along a whole side of the view. */
	target: string | null;
	zone: Zone;
	/** What lights up while dragging, in fractions of the area. */
	preview: Rect;
}

function half(r: Rect, side: Side): Rect {
	if (side === 'left') return { ...r, w: r.w / 2 };
	if (side === 'right') return { ...r, x: r.x + r.w / 2, w: r.w / 2 };
	if (side === 'top') return { ...r, h: r.h / 2 };
	return { ...r, y: r.y + r.h / 2, h: r.h / 2 };
}

/**
 * Where a tab dragged over the terminal area would land, with the pointer at
 * (x, y) in an area `width` by `height` pixels: near a pane's edge, beside it
 * on that side; in its middle, in it. Near the area's own edge (with a split
 * open), along that whole side. A side whose pane would be too small to
 * halve is not offered; its middle is.
 *
 * With no split, `shown` is the tab on screen, the whole area its pane.
 */
export function dropAt(layout: Layout, shown: string | undefined, x: number, y: number, width: number, height: number): Drop | null {
	if (width <= 0 || height <= 0 || x < 0 || y < 0 || x > width || y > height) return null;
	const fx = x / width;
	const fy = y / height;
	if (layout) {
		const near: [Side, number][] = [
			['left', x],
			['right', width - x],
			['top', y],
			['bottom', height - y]
		];
		const [side, d] = near.reduce((m, c) => (c[1] < m[1] ? c : m));
		if (d <= EDGE_BAND) {
			const across = side === 'left' || side === 'right';
			if (across ? width >= 2 * MIN_PANE_WIDTH : height >= 2 * MIN_PANE_HEIGHT) {
				const share = 0.3;
				const preview: Rect =
					side === 'left' ? { x: 0, y: 0, w: share, h: 1 } : side === 'right' ? { x: 1 - share, y: 0, w: share, h: 1 } : side === 'top' ? { x: 0, y: 0, w: 1, h: share } : { x: 0, y: 1 - share, w: 1, h: share };
				return { target: null, zone: side, preview };
			}
		}
	}
	const panes: PaneRect[] = layout ? geometry(layout).panes : shown ? [{ id: shown, x: 0, y: 0, w: 1, h: 1 }] : [];
	const p = panes.find((r) => fx >= r.x - EPS && fx <= r.x + r.w + EPS && fy >= r.y - EPS && fy <= r.y + r.h + EPS);
	if (!p) return null;
	const px = (fx - p.x) / p.w;
	const py = (fy - p.y) / p.h;
	const canRow = p.w * width >= 2 * MIN_PANE_WIDTH;
	const canColumn = p.h * height >= 2 * MIN_PANE_HEIGHT;
	const sides: [Side, number][] = [];
	if (canRow) sides.push(['left', px], ['right', 1 - px]);
	if (canColumn) sides.push(['top', py], ['bottom', 1 - py]);
	const best = sides.reduce<[Side, number] | null>((m, c) => (!m || c[1] < m[1] ? c : m), null);
	if (best && best[1] < 0.3) return { target: p.id, zone: best[0], preview: half(p, best[0]) };
	return { target: p.id, zone: 'center', preview: { x: p.x, y: p.y, w: p.w, h: p.h } };
}

/** The tab a keyboard split brings in: the next one not on screen, else the one before. */
export function neighbour(ids: string[], focusedId: string | undefined, shown: string[] = []): string | null {
	const i = focusedId ? ids.indexOf(focusedId) : -1;
	if (i < 0) return null;
	const free = (id: string) => id !== focusedId && !shown.includes(id);
	for (let k = i + 1; k < ids.length; k++) if (free(ids[k])) return ids[k];
	for (let k = i - 1; k >= 0; k--) if (free(ids[k])) return ids[k];
	return null;
}
