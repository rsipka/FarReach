/**
 * One side of the full-pane file manager: a folder, its rows, the row the
 * cursor is on and the rows marked for an action. The same for this
 * computer's files and the server's; only how a folder is listed differs.
 */
import { SvelteSet } from 'svelte/reactivity';
import { compareEntries, type FileEntry, type SortKey } from '#lib/state/explorer.svelte.js';

export type PaneKind = 'local' | 'remote';

export interface Listing {
	path: string;
	/** None at a root. */
	parent: string | null;
	entries: FileEntry[];
}

/** A row: an entry, or the `..` row that goes up a folder. */
export type Row = FileEntry & { up?: true };

/** A server folder's parent, or null at `/`. */
export function remoteParent(path: string): string | null {
	if (path === '/' || path === '') return null;
	return path.replace(/\/[^/]+\/?$/, '') || '/';
}

export function remoteJoin(dir: string, name: string): string {
	return dir.endsWith('/') ? dir + name : `${dir}/${name}`;
}

/** Folders first, then by `key`; the `..` row always on top. */
export function sortRows(entries: FileEntry[], key: SortKey, desc: boolean, parent: string | null): Row[] {
	const sorted: Row[] = [...entries].sort((a, b) => compareEntries(a, b, key, desc));
	if (parent === null) return sorted;
	const up: Row = { name: '..', path: parent, isDirectory: true, size: 0, modified: 0, up: true };
	return [up, ...sorted];
}

/** The row to go to for typed letters: the first name that starts with
 *  them, else the first that has them, case ignored. -1 when none. */
export function findByPrefix(rows: Row[], typed: string): number {
	const q = typed.toLowerCase();
	const starts = rows.findIndex((r) => !r.up && r.name.toLowerCase().startsWith(q));
	return starts >= 0 ? starts : rows.findIndex((r) => !r.up && r.name.toLowerCase().includes(q));
}

export class Pane {
	readonly kind: PaneKind;
	path = $state('');
	parent = $state<string | null>(null);
	entries = $state<FileEntry[]>([]);
	cursor = $state(0);
	/** Paths of the marked rows. */
	readonly marked = new SvelteSet<string>();
	loading = $state(false);
	error = $state<string | undefined>();
	sort = $state<SortKey>('name');
	desc = $state(false);

	rows = $derived(sortRows(this.entries, this.sort, this.desc, this.parent));
	current = $derived<Row | undefined>(this.rows[this.cursor]);

	#list: (path: string) => Promise<Listing>;
	/** Only the newest listing lands: a slow one asked for first must not
	 *  replace the folder the person went to after it. */
	#generation = 0;

	constructor(kind: PaneKind, list: (path: string) => Promise<Listing>) {
		this.kind = kind;
		this.#list = list;
	}

	/** Open `path`, the cursor on `focus` (by name) when it is there: going
	 *  up lands on the folder you came out of. Resolves false on failure,
	 *  leaving the folder shown as it was. */
	async load(path: string, focus?: string): Promise<boolean> {
		const gen = ++this.#generation;
		this.loading = true;
		this.error = undefined;
		try {
			const listing = await this.#list(path);
			if (gen !== this.#generation) return false;
			const changed = listing.path !== this.path;
			this.path = listing.path;
			this.parent = listing.parent;
			this.entries = listing.entries;
			if (changed) this.marked.clear();
			else for (const p of [...this.marked]) if (!listing.entries.some((e) => e.path === p)) this.marked.delete(p);
			const at = focus === undefined ? -1 : this.rows.findIndex((r) => !r.up && r.name === focus);
			this.cursor = at >= 0 ? at : changed ? 0 : Math.min(this.cursor, Math.max(0, this.rows.length - 1));
			return true;
		} catch (err) {
			if (gen === this.#generation) this.error = err instanceof Error ? err.message : String(err);
			return false;
		} finally {
			if (gen === this.#generation) this.loading = false;
		}
	}

	refresh(): Promise<boolean> {
		return this.load(this.path, this.current?.up ? undefined : this.current?.name);
	}

	up(): void {
		if (this.parent === null) return;
		const from = this.path.replace(/[\\/]+$/, '').split(/[\\/]/).pop();
		void this.load(this.parent, from);
	}

	/** Go into a folder row (or up for `..`). False for a file. */
	enter(row: Row | undefined = this.current): boolean {
		if (!row?.isDirectory) return false;
		if (row.up) this.up();
		else void this.load(row.path);
		return true;
	}

	move(to: number): void {
		this.cursor = Math.max(0, Math.min(this.rows.length - 1, to));
	}

	toggleMark(row: Row | undefined = this.current): void {
		if (!row || row.up) return;
		if (this.marked.has(row.path)) this.marked.delete(row.path);
		else this.marked.add(row.path);
	}

	markAll(): void {
		const all = this.entries.every((e) => this.marked.has(e.path));
		this.marked.clear();
		if (!all) for (const e of this.entries) this.marked.add(e.path);
	}

	/** What an action works on: the marked rows, or else the cursor's. */
	selection(): FileEntry[] {
		if (this.marked.size > 0) return this.entries.filter((e) => this.marked.has(e.path));
		const row = this.current;
		return row && !row.up ? [row] : [];
	}

	sortBy(key: SortKey): void {
		const keep = this.current;
		if (this.sort === key) this.desc = !this.desc;
		else {
			this.sort = key;
			this.desc = false;
		}
		if (keep) this.cursor = Math.max(0, this.rows.findIndex((r) => r.path === keep.path && !!r.up === !!keep.up));
	}
}
