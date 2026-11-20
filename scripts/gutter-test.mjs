/**
 * Tests the terminal scrollback setting and the line-number/timestamp gutter
 * in Node, with a pretend terminal buffer instead of xterm.js.
 *
 * Run: node scripts/gutter-test.mjs
 */

import { LineClock, walkLines, gutterTime, numberWidth } from '../src/lib/terminal/gutter.ts';
import {
	scrollbackSetting,
	scrollbackLines,
	DEFAULT_SCROLLBACK,
	UNLIMITED_LINES,
	UNLIMITED_LINES_MOBILE
} from '../src/lib/terminal/scrollback.ts';

let failures = 0;
const check = (name, cond, extra = '') => {
	if (cond) console.log(`  PASS  ${name}`);
	else {
		failures++;
		console.log(`  FAIL  ${name}${extra ? '\n        ' + extra : ''}`);
	}
};
const section = (s) => console.log(`\n${s}`);

/**
 * A pretend xterm.js: rows (each just a wrap flag), a screen of `rows`, a
 * scrollback limit, markers that move when the top is trimmed and die when
 * their row goes.
 */
function fakeTerm(screenRows, scrollback) {
	const lines = [];
	for (let i = 0; i < screenRows; i++) lines.push({ isWrapped: false });
	const markers = new Set();
	const buf = {
		type: 'normal',
		cursorY: 0,
		viewportY: 0,
		get length() {
			return lines.length;
		},
		get baseY() {
			return lines.length - screenRows;
		},
		getLine: (r) => lines[r]
	};
	function trim() {
		while (lines.length > screenRows + scrollback) {
			lines.shift();
			for (const m of [...markers]) {
				m.line--;
				if (m.line < 0) m.dispose();
			}
		}
	}
	const term = {
		rows: screenRows,
		buffer: { active: buf },
		registerMarker(offset = 0) {
			const m = {
				line: buf.baseY + buf.cursorY + offset,
				isDisposed: false,
				dispose() {
					this.isDisposed = true;
					this.line = -1;
					markers.delete(this);
				}
			};
			markers.add(m);
			return m;
		},
		markerCount: () => markers.size,
		/** Moves to a new row, as a line feed (or a wrap) does. */
		feed(isWrapped = false) {
			if (buf.cursorY < screenRows - 1) {
				buf.cursorY++;
				lines[buf.baseY + buf.cursorY] = { isWrapped };
			} else {
				const follow = buf.viewportY === buf.baseY;
				lines.push({ isWrapped });
				trim();
				if (follow) buf.viewportY = buf.baseY;
			}
		},
		lines
	};
	return term;
}

section('scrollback setting');
check('the default is Unlimited', DEFAULT_SCROLLBACK === 0 && scrollbackSetting(undefined) === 0);
check('a stored number is kept', scrollbackSetting(10000) === 10000);
check('junk falls back to the default', scrollbackSetting('lots') === 0 && scrollbackSetting(-5) === 0 && scrollbackSetting(NaN) === 0);
check('fractions are cut to whole lines', scrollbackSetting(1500.7) === 1500);
check('more than Unlimited is Unlimited', scrollbackSetting(10_000_000) === UNLIMITED_LINES);
check('Unlimited on a computer is 250,000 lines', scrollbackLines(0, false) === 250_000);
check('Unlimited on a phone is 50,000 lines', scrollbackLines(0, true) === UNLIMITED_LINES_MOBILE && UNLIMITED_LINES_MOBILE === 50_000);
check('a set number is used as is', scrollbackLines(1000, false) === 1000 && scrollbackLines(1000, true) === 1000);
check('a phone caps a large number', scrollbackLines(100_000, true) === 50_000);

section('walking rows to line numbers');
{
	const rows = [false, false, true, true, false, true, false].map((w) => ({ isWrapped: w }));
	const buf = { getLine: (r) => rows[r] };
	check('forward over wrapped rows', walkLines(buf, 0, 1, 6) === 4);
	check('backward over wrapped rows', walkLines(buf, 6, 4, 0) === 1);
	check('a wrapped row has its line number', walkLines(buf, 0, 1, 3) === 2);
	check('the same row', walkLines(buf, 4, 3, 4) === 3);
}

section('numbering and times');
{
	const term = fakeTerm(5, 20);
	const clock = new LineClock(term, 20);
	clock.update(1000); // the prompt on line 1
	let rows = clock.rows();
	check('the first line is 1', rows[0]?.line === 1 && rows[0]?.time === 1000);
	check('rows below the cursor show nothing', rows.slice(1).every((r) => r === null));

	term.feed(); // line 2
	term.feed(true); // line 2 wraps
	term.feed(); // line 3
	clock.update(2000);
	rows = clock.rows();
	check('line 2 has its number and time', rows[1]?.line === 2 && rows[1]?.time === 2000);
	check('a wrapped row shows nothing', rows[2] === null);
	check('line 3 follows', rows[3]?.line === 3);
	check('an earlier line keeps its time', rows[0]?.time === 1000);
	check('one marker follows the cursor', term.markerCount() === 1);
}

section('trimmed scrollback');
{
	const term = fakeTerm(4, 10);
	const clock = new LineClock(term, 10);
	clock.update(0);
	for (let i = 0; i < 60; i++) {
		term.feed();
		clock.update(i + 1);
	}
	check('the buffer is full', term.lines.length === 14);
	const rows = clock.rows();
	check('the bottom row is line 61', rows[3]?.line === 61, JSON.stringify(rows));
	check('numbers keep counting past the trim', rows[0]?.line === 58);
	check('times stay with their lines', rows[0]?.time === 57 && rows[3]?.time === 60);
	check('lastLine is the newest line', clock.lastLine === 61);
}

section('scrolling back');
{
	const term = fakeTerm(4, 100);
	const clock = new LineClock(term, 100);
	clock.update(0);
	for (let i = 0; i < 50; i++) {
		term.feed(i % 3 === 0);
		clock.update(i + 1);
	}
	const expectAt = (row) => walkLines(term.buffer.active, 0, 1, row);
	term.buffer.active.viewportY = 10;
	let rows = clock.rows();
	const first = rows.find((r) => r !== null);
	check('the scrolled-back view numbers like a count from the top', first && first.line === expectAt(rows.indexOf(first) + 10));
	check('a view marker is kept while scrolled back', term.markerCount() === 2);
	term.buffer.active.viewportY = 11;
	rows = clock.rows();
	const r11 = rows.find((r) => r !== null);
	check('one row further is still right', r11 && r11.line === expectAt(rows.indexOf(r11) + 11));
	term.buffer.active.viewportY = term.buffer.active.baseY;
	clock.rows();
	check('the view marker goes at the bottom', term.markerCount() === 1);
}

section('alternate screen and lost markers');
{
	const term = fakeTerm(4, 10);
	const clock = new LineClock(term, 10);
	clock.update(0);
	term.feed();
	clock.update(5);
	term.buffer.active.type = 'alternate';
	check('the alternate screen shows nothing', clock.rows().every((r) => r === null));
	clock.update(9);
	term.buffer.active.type = 'normal';
	check('output on it gives no times', clock.lastLine === 2);
	clock.dispose();
	check('dispose drops the markers', term.markerCount() === 0);
	clock.update(10);
	check('a lost marker carries on from the last line', clock.rows()[1]?.line === 2);
}

section('formatting');
{
	const t = new Date(2026, 10, 19, 7, 5, 3).getTime();
	check('time is HH:MM:SS', gutterTime(t) === '07:05:03');
	check('number column is at least 5 wide', numberWidth(12) === 5 && numberWidth(0) === 5);
	check('number column grows past 99,999', numberWidth(123456) === 6);
}

console.log(failures ? `\n${failures} failed` : '\nAll passed');
process.exit(failures ? 1 : 0);
