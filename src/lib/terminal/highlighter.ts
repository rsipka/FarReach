/**
 * Draws the highlights of #lib/terminal/highlight in a live terminal.
 *
 * Only the rows on screen are looked at, again whenever they change (output,
 * a scroll, a resize), so a long scrollback costs nothing. Each highlight is
 * an xterm decoration that recolours its cells; the text itself is never
 * touched, so a copy, a search or a log has it as the server sent it.
 *
 * A line the terminal wrapped is read whole, so an address split over two
 * rows is still found. Programs that draw a full screen of their own (vim,
 * htop, mc) run in the alternate buffer, which is left alone: they colour
 * their own text.
 */

import type { IBufferCell, IDecoration, IDisposable, IMarker, Terminal } from '@xterm/xterm';
import { findHighlights, type HighlightKind } from './highlight.js';

export interface HighlighterOptions {
	enabled: boolean;
	kinds: readonly HighlightKind[];
	colors: Record<HighlightKind, string>;
}

interface Shown {
	marker: IMarker;
	decoration: IDecoration;
}

/** A logical line: its text, and the row and column of each character. */
interface Logical {
	text: string;
	rows: number[];
	cols: number[];
	widths: number[];
}

export class Highlighter {
	private options: HighlighterOptions = { enabled: false, kinds: [], colors: {} as Record<HighlightKind, string> };
	private shown = new Map<string, Shown>();
	private listeners: IDisposable[] = [];
	private frame = 0;
	/** What the screen held at the last scan; the same screen is not scanned again. */
	private lastScan = '';
	private disposed = false;

	constructor(private term: Terminal) {
		const schedule = () => this.schedule();
		this.listeners.push(
			term.onWriteParsed(schedule),
			term.onScroll(schedule),
			term.onResize(schedule),
			term.buffer.onBufferChange(schedule),
			// A scroll with the wheel moves the viewport without onScroll in
			// some xterm versions; a render follows every one. The scan
			// stops at once when the screen is unchanged, so the render its
			// own decorations cause does not loop.
			term.onRender(schedule)
		);
	}

	setOptions(options: HighlighterOptions): void {
		this.options = options;
		this.lastScan = '';
		this.schedule();
	}

	dispose(): void {
		this.disposed = true;
		if (this.frame) cancelAnimationFrame(this.frame);
		this.listeners.forEach((l) => l.dispose());
		this.listeners = [];
		this.clear();
	}

	private schedule(): void {
		if (this.disposed || this.frame) return;
		this.frame = requestAnimationFrame(() => {
			this.frame = 0;
			try {
				this.scan();
			} catch (e) {
				// A terminal disposed or resized mid-frame; the next change scans again.
				console.debug('[Highlighter] scan skipped:', e);
			}
		});
	}

	private clear(): void {
		for (const s of this.shown.values()) {
			s.decoration.dispose();
			s.marker.dispose();
		}
		this.shown.clear();
		this.lastScan = '';
	}

	private scan(): void {
		const { enabled, kinds, colors } = this.options;
		const buffer = this.term.buffer.active;
		if (!enabled || kinds.length === 0 || buffer.type !== 'normal') {
			if (this.shown.size) this.clear();
			return;
		}
		const top = buffer.viewportY;
		const bottom = Math.min(top + this.term.rows, buffer.length);

		// The screen as text, to tell whether anything changed.
		const rowsText: string[] = [];
		for (let y = top; y < bottom; y++) rowsText.push(buffer.getLine(y)?.translateToString(true) ?? '');
		const key = `${top}|${buffer.baseY}|${this.term.cols}|${rowsText.join('\n')}`;
		if (key === this.lastScan) return;
		this.lastScan = key;

		// Every logical line with a row on screen, from its first row.
		let y = top;
		while (y > 0 && buffer.getLine(y)?.isWrapped) y--;
		const wanted = new Map<string, { row: number; col: number; width: number; color: string }>();
		const cell: IBufferCell | undefined = buffer.getNullCell();
		while (y < bottom) {
			const line = this.readLogical(y, cell);
			y = line.rows.length ? line.rows[line.rows.length - 1] + 1 : y + 1;
			for (const h of findHighlights(line.text, kinds)) {
				// One decoration per row the match covers.
				let i = h.start;
				while (i < h.end) {
					const row = line.rows[i];
					const col = line.cols[i];
					let j = i;
					let width = 0;
					while (j < h.end && line.rows[j] === row) {
						width += line.widths[j];
						j++;
					}
					if (row >= top && row < bottom && width > 0) {
						const color = colors[h.kind];
						wanted.set(`${row}:${col}:${width}:${color}`, { row, col, width, color });
					}
					i = j;
				}
			}
		}

		// Keep what is still right, drop the rest, add what is new.
		for (const [k, s] of this.shown) {
			if (!wanted.has(k) || s.marker.isDisposed || s.marker.line !== Number(k.split(':')[0])) {
				s.decoration.dispose();
				s.marker.dispose();
				this.shown.delete(k);
			}
		}
		const cursorLine = buffer.baseY + buffer.cursorY;
		for (const [k, w] of wanted) {
			if (this.shown.has(k)) continue;
			const marker = this.term.registerMarker(w.row - cursorLine);
			if (!marker) continue;
			const decoration = this.term.registerDecoration({
				marker,
				x: w.col,
				width: w.width,
				foregroundColor: w.color,
				layer: 'bottom'
			});
			if (!decoration) {
				marker.dispose();
				continue;
			}
			this.shown.set(k, { marker, decoration });
		}
	}

	/**
	 * The logical line starting at row `y`: that row and the rows the
	 * terminal wrapped it onto.
	 */
	private readLogical(y: number, cell: IBufferCell | undefined): Logical {
		const buffer = this.term.buffer.active;
		const out: Logical = { text: '', rows: [], cols: [], widths: [] };
		let row = y;
		// A very long wrapped line (a minified file) is read no further than
		// a few screens; nothing past that is on screen anyway.
		const limit = y + this.term.rows * 3;
		do {
			const line = buffer.getLine(row);
			if (!line) break;
			for (let x = 0; x < line.length; x++) {
				const c = line.getCell(x, cell);
				if (!c) break;
				const w = c.getWidth();
				if (w === 0) continue; // the second half of a wide character
				const chars = c.getChars() || ' ';
				for (let k = 0; k < chars.length; k++) {
					out.text += chars[k];
					out.rows.push(row);
					out.cols.push(x);
					// The cells belong to the first code unit of the character.
					out.widths.push(k === 0 ? w : 0);
				}
			}
			row++;
		} while (row < limit && buffer.getLine(row)?.isWrapped);
		return out;
	}
}
