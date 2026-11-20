/**
 * Saved split layouts (see #lib/terminal/layouts): kept on this device, in
 * local storage, as recent sessions are. Only session ids and the split's
 * shape are stored; nothing of it syncs.
 *
 * Opening one connects its saved sessions the way a click in the session
 * list does (password prompts and all) and opens its local shells, then puts
 * each new tab in its pane as it appears. A session that does not open (it
 * was deleted, or its connect was cancelled) leaves its pane out.
 */

import { untrack } from 'svelte';
import { getTabs, getSplit, createTab, setLayout, unsplit, activateTab } from '#lib/state/tabs.svelte.js';
import { requestSessionAction } from '#lib/state/palette.svelte.js';
import { isMobile } from '#lib/platform.js';
import { toSaved, savedLeaves, fill, claim, parseLayouts, type SavedLayout } from '#lib/terminal/layouts.js';
import { windowLeaves, type KeptWindow } from '#lib/terminal/float-windows.js';
import { reopenAround } from '#lib/state/windows.svelte.js';

const KEY = 'reach-split-layouts';
/** How long an opening layout waits for its sessions' tabs. */
const WAIT_MS = 120_000;

function load(): SavedLayout[] {
	try {
		return parseLayouts(JSON.parse(localStorage.getItem(KEY) ?? '[]'));
	} catch {
		return [];
	}
}

let layouts = $state<SavedLayout[]>(typeof localStorage === 'undefined' ? [] : load());

function store(): void {
	try {
		localStorage.setItem(KEY, JSON.stringify(layouts));
	} catch {
		// Storage full or unavailable: the list just is not remembered.
	}
}

export function getLayouts(): SavedLayout[] {
	return layouts;
}

/** Can the split on screen be saved (two panes or more that can open again)? */
export function canSaveLayout(): boolean {
	return toSaved(getSplit(), getTabs()) !== null;
}

/** Save the split on screen under `name`; a layout of that name is replaced. */
export function saveLayout(name: string): boolean {
	const tree = toSaved(getSplit(), getTabs());
	const n = name.trim();
	if (!tree || !n) return false;
	const same = layouts.find((l) => l.name.toLowerCase() === n.toLowerCase());
	const entry: SavedLayout = { id: same?.id ?? crypto.randomUUID(), name: n, tree };
	layouts = same ? layouts.map((l) => (l.id === same.id ? entry : l)) : [...layouts, entry];
	store();
	return true;
}

export function deleteLayout(id: string): void {
	layouts = layouts.filter((l) => l.id !== id);
	store();
}

let stopWaiting: (() => void) | null = null;

/** Open a saved layout: its sessions connect and fill their panes as they come. */
export function openLayout(id: string): void {
	const saved = layouts.find((l) => l.id === id);
	if (!saved) return;
	stopWaiting?.();
	const leaves = savedLeaves(saved.tree);
	const slots: (string | null)[] = leaves.map(() => null);
	const before = new Set(getTabs().map((t) => t.id));
	unsplit();

	// Local shells open at once; not on a phone or tablet, which has none.
	leaves.forEach((leaf, i) => {
		if ('local' in leaf && !isMobile()) {
			const tab = createTab('local', undefined, undefined, undefined, undefined, leaf.command, leaf.wslDistro);
			before.add(tab.id);
			slots[i] = tab.id;
		}
	});
	const show = () => {
		setLayout(fill(saved.tree, slots));
		const last = getTabs().find((t) => t.active);
		if (last) activateTab(last.id);
	};
	show();

	const sessions = leaves.flatMap((l) => ('session' in l ? [l.session] : []));
	if (sessions.length === 0) return;

	// New tabs from the saved sessions take their slots as they appear.
	const stop = $effect.root(() => {
		$effect(() => {
			let changed = false;
			for (const tab of getTabs()) {
				const sid = tab.sessionId;
				if (before.has(tab.id) || !sid) continue;
				const i = claim(leaves, slots, sid);
				before.add(tab.id);
				if (i < 0) continue;
				slots[i] = tab.id;
				changed = true;
			}
			if (changed) untrack(show);
			if (slots.every((s) => s !== null)) queueMicrotask(() => stopWaiting?.());
		});
	});
	const timer = setTimeout(() => stopWaiting?.(), WAIT_MS);
	stopWaiting = () => {
		clearTimeout(timer);
		stop();
		stopWaiting = null;
	};
	requestSessionAction({ kind: 'connect-many', sessionIds: sessions });
}

/**
 * A floating window kept from the last run (#lib/state/windows), opened
 * again: its sessions connect here, as a layout's do, then move to a window
 * of their own where it was. Resolves once that window is asked for.
 */
export function openKept(kept: KeptWindow): Promise<void> {
	const { leaves, inTree } = windowLeaves(kept);
	const slots: (string | null)[] = leaves.map(() => null);
	const before = new Set(getTabs().map((t) => t.id));
	const keep = getSplit();
	leaves.forEach((leaf, i) => {
		if ('local' in leaf) {
			const tab = createTab('local', undefined, undefined, undefined, undefined, leaf.command, leaf.wslDistro);
			before.add(tab.id);
			slots[i] = tab.id;
		}
	});
	// New tabs must not take the panes of a split here.
	setLayout(keep);
	const sessions = leaves.flatMap((l) => ('session' in l ? [l.session] : []));

	return new Promise((resolve) => {
		let done = false;
		const finish = () => {
			if (done) return;
			done = true;
			clearTimeout(timer);
			stop();
			const ids = slots.filter((s): s is string => s !== null);
			const layout = kept.tree ? fill(kept.tree, slots.slice(0, inTree)) : null;
			void reopenAround(ids, kept, layout).finally(resolve);
		};
		const stop = $effect.root(() => {
			$effect(() => {
				for (const tab of getTabs()) {
					const sid = tab.sessionId;
					if (before.has(tab.id) || !sid) continue;
					before.add(tab.id);
					const i = claim(leaves, slots, sid);
					if (i >= 0) slots[i] = tab.id;
				}
				if (slots.every((s) => s !== null)) queueMicrotask(finish);
			});
		});
		const timer = setTimeout(finish, WAIT_MS);
		if (sessions.length) requestSessionAction({ kind: 'connect-many', sessionIds: sessions });
		else queueMicrotask(finish);
	});
}
