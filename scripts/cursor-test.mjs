/**
 * Tests the terminal cursor rules in Node, with no terminal.
 *
 * Run: node scripts/cursor-test.mjs
 */

import {
	cursorOptions,
	cursorStyle,
	inactiveCursorStyle,
	cursorWidth,
	DEFAULT_CURSOR_STYLE,
	DEFAULT_INACTIVE_CURSOR
} from '../src/lib/terminal/cursor.ts';

let failures = 0;
const check = (name, cond, extra = '') => {
	if (cond) console.log(`  PASS  ${name}`);
	else {
		failures++;
		console.log(`  FAIL  ${name}${extra ? '\n        ' + extra : ''}`);
	}
};
const section = (s) => console.log(`\n${s}`);

section('stored settings');
check('the default shape is the bar Reach always drew', DEFAULT_CURSOR_STYLE === 'bar');
check('a known shape is kept', cursorStyle('block') === 'block' && cursorStyle('underline') === 'underline');
check('an unknown shape falls back', cursorStyle('beam') === 'bar' && cursorStyle(undefined) === 'bar');
check('the inactive default is xterm.js own outline', DEFAULT_INACTIVE_CURSOR === 'outline');
check('a known inactive shape is kept', inactiveCursorStyle('none') === 'none');
check('an unknown inactive shape falls back', inactiveCursorStyle('hidden') === 'outline');
check('a bar width is kept', cursorWidth(3) === 3);
check('an odd width falls back', cursorWidth(9) === 1 && cursorWidth('2') === 1);

const settings = { terminalCursorStyle: 'block', terminalCursorBlink: false, terminalCursorInactive: 'none', terminalCursorWidth: 2 };

section('what a terminal draws');
const plain = cursorOptions(settings);
check('no session choice: Settings', plain.cursorStyle === 'block' && plain.cursorBlink === false);
check('inactive shape and width come from Settings', plain.cursorInactiveStyle === 'none' && plain.cursorWidth === 2);
const own = cursorOptions(settings, { style: 'underline', blink: true });
check('a session shape wins', own.cursorStyle === 'underline');
check('a session blink wins, even over off', own.cursorBlink === true);
const unset = cursorOptions(settings, { style: null, blink: null });
check('unset session choices follow Settings', unset.cursorStyle === 'block' && unset.cursorBlink === false);
const odd = cursorOptions(settings, { style: 'beam' });
check('an unknown session shape follows Settings', odd.cursorStyle === 'block');
const empty = cursorOptions({});
check('nothing stored: the old look', empty.cursorStyle === 'bar' && empty.cursorBlink === true && empty.cursorWidth === 1);

if (failures) {
	console.log(`\n${failures} failed`);
	process.exit(1);
}
console.log('\nAll cursor tests passed');
