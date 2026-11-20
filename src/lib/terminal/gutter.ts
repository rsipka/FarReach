/**
 * Line numbers and timestamps for the gutter left of a terminal.
 *
 * A line here is a logical line: the rows a long line wraps onto share its
 * number and time, and only its first row shows them. Numbers count from the
 * start of the session and keep counting when the oldest lines leave the
 * scrollback, so line 120,000 stays line 120,000. The time is when the line
 * was first written.
 *
 * Nothing is stored per row. xterm.js moves a marker with its line when the
 * scrollback is trimmed or a resize rewraps the text, so one marker at the
 * cursor's line (and one at the top of the view, for scrolling back) plus
 * the wrap flag of each row is enough to number any row: one more for every
 * row that starts a line. Times are kept in an array by line number, trimmed
 * along with the scrollback.
 *
 * Only the normal screen is numbered; a full-screen program (vim, less, top)
 * draws on the alternate screen, where line numbers mean nothing.
 */

export interface GutterLine {
	readonly isWrapped: boolean;
}

export interface GutterBuffer {
	readonly type: 'normal' | 'alternate';
	readonly length: number;
	readonly baseY: number;
	readonly cursorY: number;
	readonly viewportY: number;
	getLine(row: number): GutterLine | undefined;
}

export interface GutterMarker {
	readonly line: number;
	readonly isDisposed: boolean;
	dispose(): void;
}

/** The part of an xterm.js Terminal the gutter reads. */
export interface GutterTerminal {
	readonly rows: number;
	readonly buffer: { readonly active: GutterBuffer };
	registerMarker(cursorYOffset?: number): GutterMarker | undefined;
}

/** One screen row of the gutter; null where it shows nothing. */
export type GutterRow = { line: number; time: number | undefined } | null;

interface Anchor {
	marker: GutterMarker;
	logical: number;
}

function wrapped(buf: GutterBuffer, row: number): boolean {
	return buf.getLine(row)?.isWrapped ?? false;
}

/** The logical line number of `to`, counting from a row whose number is known. */
export function walkLines(buf: GutterBuffer, from: number, logical: number, to: number): number {
	let n = logical;
	if (to >= from) {
		for (let r = from + 1; r <= to; r++) if (!wrapped(buf, r)) n++;
	} else {
		for (let r = from; r > to; r--) if (!wrapped(buf, r)) n--;
	}
	return n;
}

function live(a: Anchor | null): Anchor | null {
	return a && !a.marker.isDisposed && a.marker.line >= 0 ? a : null;
}

export class LineClock {
	private anchor: Anchor | null = null;
	private view: Anchor | null = null;
	/** times[i] is when line timesBase + i was first written. */
	private times: number[] = [];
	private timesBase = 1;
	/** The highest line number that has a time. */
	private stamped = 0;
	private keep = 0;

	private readonly term: GutterTerminal;

	constructor(term: GutterTerminal, keepLines: number) {
		this.term = term;
		this.setKeep(keepLines);
	}

	/** How many lines' times to keep: the scrollback, plus a screen or so. */
	setKeep(lines: number): void {
		this.keep = Math.max(1000, lines + 1000);
	}

	/** After output: give every line up to the cursor's a time, if it has none. */
	update(now: number): void {
		const buf = this.term.buffer.active;
		if (buf.type !== 'normal') return;
		const cursorRow = buf.baseY + buf.cursorY;
		let start = cursorRow;
		while (start > 0 && wrapped(buf, start)) start--;

		const a = live(this.anchor);
		// With no marker left (the very first output, or a clear that took
		// the scrollback and its markers with it) the cursor's line carries
		// on from the last line seen.
		const logical = a ? walkLines(buf, a.marker.line, a.logical, start) : Math.max(1, this.stamped);
		while (this.stamped < logical) {
			this.stamped++;
			this.times.push(now);
		}
		this.prune();

		if (!a || a.marker.line !== start) {
			a?.marker.dispose();
			const marker = this.term.registerMarker(start - cursorRow);
			this.anchor = marker ? { marker, logical } : null;
		}
	}

	/** What each row of the screen shows, top to bottom. */
	rows(): GutterRow[] {
		const buf = this.term.buffer.active;
		const out: GutterRow[] = new Array(this.term.rows).fill(null);
		if (buf.type !== 'normal') return out;
		const top = buf.viewportY;

		// Count from whichever known row is nearer: the cursor's line while
		// following output, the last view while scrolling back.
		const refs = [live(this.anchor), live(this.view)].filter((x): x is Anchor => x !== null);
		if (refs.length === 0) return out;
		const ref = refs.reduce((best, x) => (Math.abs(x.marker.line - top) < Math.abs(best.marker.line - top) ? x : best));
		let logical = walkLines(buf, ref.marker.line, ref.logical, top);

		const a = live(this.anchor);
		if (a && Math.abs(a.marker.line - top) <= this.term.rows * 4) {
			// Following output: the cursor's marker is near enough.
			this.view?.marker.dispose();
			this.view = null;
		} else if (!live(this.view) || this.view!.marker.line !== top) {
			this.view?.marker.dispose();
			const marker = this.term.registerMarker(top - (buf.baseY + buf.cursorY));
			this.view = marker ? { marker, logical } : null;
		}

		for (let i = 0; i < out.length; i++) {
			const row = top + i;
			if (row >= buf.length) break;
			if (i > 0 && !wrapped(buf, row)) logical++;
			if (wrapped(buf, row) || logical < 1 || logical > this.stamped) continue;
			const t = this.times[logical - this.timesBase];
			out[i] = { line: logical, time: t };
		}
		return out;
	}

	/** The newest line number so far. */
	get lastLine(): number {
		return this.stamped;
	}

	dispose(): void {
		this.anchor?.marker.dispose();
		this.view?.marker.dispose();
		this.anchor = this.view = null;
	}

	private prune(): void {
		// In chunks, so a busy terminal does not shift the array every line.
		if (this.times.length <= this.keep + 10_000) return;
		const drop = this.times.length - this.keep;
		this.times.splice(0, drop);
		this.timesBase += drop;
	}
}

const pad = (n: number) => String(n).padStart(2, '0');

/** A line's time as the gutter shows it: 24-hour local time with seconds. */
export function gutterTime(ms: number): string {
	const d = new Date(ms);
	return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

/** Characters the number column needs up to this line: at least 5, so the
 *  gutter (and with it the terminal's width) changes only past 99,999. */
export function numberWidth(lastLine: number): number {
	return Math.max(5, String(Math.max(0, Math.floor(lastLine))).length);
}
