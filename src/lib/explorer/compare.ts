/**
 * Compare for the full-pane file manager: two folders by their listings,
 * as Midnight Commander's Compare directories does, and two text files
 * line by line, shown side by side.
 */

export interface CompareEntry {
	name: string;
	path: string;
	isDirectory: boolean;
	size: number;
	modified: number;
}

export interface FolderDiff {
	/** Paths to mark on each side: files the other side lacks, and of a
	 *  file on both sides that differs, the newer copy (both when their
	 *  times are equal but sizes are not). */
	left: string[];
	right: string[];
	/** Files on both sides that look the same (size and time). */
	same: number;
}

/** Times within this many seconds count as the same: FAT and some servers
 *  keep only every other second, and a copy may round. */
const TIME_SLACK = 2;

/** Compare two folders' files (folders are left alone) by name, size and
 *  modified time. Names compare exactly. */
export function compareFolders(left: CompareEntry[], right: CompareEntry[]): FolderDiff {
	const files = (list: CompareEntry[]) => new Map(list.filter((e) => !e.isDirectory).map((e) => [e.name, e]));
	const l = files(left);
	const r = files(right);
	const out: FolderDiff = { left: [], right: [], same: 0 };
	for (const [name, a] of l) {
		const b = r.get(name);
		if (!b) {
			out.left.push(a.path);
			continue;
		}
		const dt = a.modified - b.modified;
		const sameTime = Math.abs(dt) <= TIME_SLACK || !a.modified || !b.modified;
		if (a.size === b.size && sameTime) {
			out.same++;
		} else if (sameTime) {
			out.left.push(a.path);
			out.right.push(b.path);
		} else if (dt > 0) {
			out.left.push(a.path);
		} else {
			out.right.push(b.path);
		}
	}
	for (const [name, b] of r) if (!l.has(name)) out.right.push(b.path);
	return out;
}

export interface DiffRow {
	kind: 'same' | 'changed' | 'removed' | 'added' | 'gap';
	leftNo?: number;
	left?: string;
	rightNo?: number;
	right?: string;
}

/** Above this many line pairs between the common start and end, the
 *  line-by-line match would take too long; the middle is shown as one
 *  changed block instead. */
const MAX_CELLS = 4_000_000;

/** Unchanged lines kept around each change; longer runs fold to a gap. */
const CONTEXT = 3;

function splitLines(text: string): string[] {
	const lines = text.replace(/\r\n/g, '\n').split('\n');
	if (lines.length > 1 && lines[lines.length - 1] === '') lines.pop();
	return lines;
}

/** Side-by-side rows of a line diff of `a` (left) and `b` (right). With
 *  `context`, long unchanged runs fold into a 'gap' row. */
export function diffLines(a: string, b: string, context: number | null = CONTEXT): DiffRow[] {
	const x = splitLines(a);
	const y = splitLines(b);
	let start = 0;
	while (start < x.length && start < y.length && x[start] === y[start]) start++;
	let endX = x.length;
	let endY = y.length;
	while (endX > start && endY > start && x[endX - 1] === y[endY - 1]) {
		endX--;
		endY--;
	}

	// Edit script of the middle: 's' same, 'd' only left, 'a' only right.
	const ops: ('s' | 'd' | 'a')[] = [];
	const n = endX - start;
	const m = endY - start;
	if (n * m > MAX_CELLS) {
		for (let i = 0; i < n; i++) ops.push('d');
		for (let j = 0; j < m; j++) ops.push('a');
	} else {
		// Longest common subsequence, filled from the end.
		const w = m + 1;
		const table = new Uint32Array((n + 1) * w);
		for (let i = n - 1; i >= 0; i--) {
			for (let j = m - 1; j >= 0; j--) {
				table[i * w + j] =
					x[start + i] === y[start + j] ? table[(i + 1) * w + j + 1] + 1 : Math.max(table[(i + 1) * w + j], table[i * w + j + 1]);
			}
		}
		let i = 0;
		let j = 0;
		while (i < n && j < m) {
			if (x[start + i] === y[start + j]) {
				ops.push('s');
				i++;
				j++;
			} else if (table[(i + 1) * w + j] >= table[i * w + j + 1]) {
				ops.push('d');
				i++;
			} else {
				ops.push('a');
				j++;
			}
		}
		while (i++ < n) ops.push('d');
		while (j++ < m) ops.push('a');
	}

	const rows: DiffRow[] = [];
	for (let k = 0; k < start; k++) rows.push({ kind: 'same', leftNo: k + 1, left: x[k], rightNo: k + 1, right: y[k] });
	let i = start;
	let j = start;
	for (let k = 0; k < ops.length; ) {
		if (ops[k] === 's') {
			rows.push({ kind: 'same', leftNo: i + 1, left: x[i], rightNo: j + 1, right: y[j] });
			i++;
			j++;
			k++;
			continue;
		}
		// A run of removals and additions: paired up as changed lines.
		const dels: number[] = [];
		const adds: number[] = [];
		while (k < ops.length && ops[k] !== 's') {
			if (ops[k] === 'd') dels.push(i++);
			else adds.push(j++);
			k++;
		}
		for (let p = 0; p < Math.max(dels.length, adds.length); p++) {
			const li = dels[p];
			const ri = adds[p];
			rows.push({
				kind: li !== undefined && ri !== undefined ? 'changed' : li !== undefined ? 'removed' : 'added',
				leftNo: li === undefined ? undefined : li + 1,
				left: li === undefined ? undefined : x[li],
				rightNo: ri === undefined ? undefined : ri + 1,
				right: ri === undefined ? undefined : y[ri]
			});
		}
	}
	for (; i < x.length; i++, j++) rows.push({ kind: 'same', leftNo: i + 1, left: x[i], rightNo: j + 1, right: y[j] });
	return context === null ? rows : fold(rows, context);
}

/** Fold unchanged runs longer than twice `context` into one gap row. */
function fold(rows: DiffRow[], context: number): DiffRow[] {
	const out: DiffRow[] = [];
	const near = rows.map(() => false);
	rows.forEach((r, i) => {
		if (r.kind === 'same') return;
		for (let k = Math.max(0, i - context); k <= Math.min(rows.length - 1, i + context); k++) near[k] = true;
	});
	let hidden = 0;
	rows.forEach((r, i) => {
		if (r.kind !== 'same' || near[i]) {
			if (hidden) out.push({ kind: 'gap', leftNo: hidden });
			hidden = 0;
			out.push(r);
		} else hidden++;
	});
	if (hidden) out.push({ kind: 'gap', leftNo: hidden });
	return out;
}

/** How many lines differ, for the header. */
export function countChanges(rows: DiffRow[]): number {
	return rows.filter((r) => r.kind !== 'same' && r.kind !== 'gap').length;
}
