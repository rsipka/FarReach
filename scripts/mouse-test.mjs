/**
 * Tests the terminal's mouse rules in Node, with no terminal.
 *
 * Run: node scripts/mouse-test.mjs
 */

import {
	rightClickAction,
	rightClickMode,
	wordSeparator,
	DEFAULT_RIGHT_CLICK,
	DEFAULT_WORD_SEPARATORS
} from '../src/lib/terminal/mouse.ts';

let failures = 0;
const check = (name, cond, extra = '') => {
	if (cond) console.log(`  PASS  ${name}`);
	else {
		failures++;
		console.log(`  FAIL  ${name}${extra ? '\n        ' + extra : ''}`);
	}
};
const section = (s) => console.log(`\n${s}`);

const click = (over) =>
	rightClickAction({ mode: 'menu', hasSelection: false, mouseTracking: false, shift: false, ctrl: false, ...over });

section('stored setting');
check('the default is a menu', DEFAULT_RIGHT_CLICK === 'menu');
check('a known mode is kept', rightClickMode('paste') === 'paste' && rightClickMode('copy-paste') === 'copy-paste');
check('nothing stored is the default', rightClickMode(undefined) === 'menu');
check('an unknown value is the default', rightClickMode('explode') === 'menu');

section('right click: menu');
check('opens the menu', click({}) === 'menu');
check('with a selection too', click({ hasSelection: true }) === 'menu');

section('right click: paste (PuTTY)');
check('pastes', click({ mode: 'paste' }) === 'paste');
check('pastes over a selection as well', click({ mode: 'paste', hasSelection: true }) === 'paste');
check('Ctrl opens the menu', click({ mode: 'paste', ctrl: true }) === 'menu');

section('right click: copy or paste');
check('copies a selection', click({ mode: 'copy-paste', hasSelection: true }) === 'copy');
check('pastes without one', click({ mode: 'copy-paste' }) === 'paste');
check('Ctrl opens the menu', click({ mode: 'copy-paste', hasSelection: true, ctrl: true }) === 'menu');

section('a program that tracks the mouse');
for (const mode of ['menu', 'paste', 'copy-paste']) {
	check(`${mode}: the click is the program's`, click({ mode, mouseTracking: true }) === 'none');
	check(`${mode}: Ctrl does not take it back`, click({ mode, mouseTracking: true, ctrl: true }) === 'none');
}
check('Shift takes the mouse back: menu', click({ mouseTracking: true, shift: true }) === 'menu');
check('Shift takes the mouse back: paste', click({ mode: 'paste', mouseTracking: true, shift: true }) === 'paste');

section('word separators');
check('the default starts with a space', wordSeparator(DEFAULT_WORD_SEPARATORS) === ' ' + DEFAULT_WORD_SEPARATORS);
check('nothing stored is the default', wordSeparator(undefined) === ' ' + DEFAULT_WORD_SEPARATORS);
check('an empty field leaves only the space', wordSeparator('') === ' ');
check('typed blanks are not doubled', wordSeparator(' :\t; ') === ' :;');
check('a repeated character counts once', wordSeparator('::/') === ' :/');
check('a path stays whole by default', !wordSeparator(DEFAULT_WORD_SEPARATORS).includes('/'));

console.log(failures === 0 ? '\nAll passed.' : `\n${failures} failed.`);
process.exit(failures === 0 ? 0 : 1);
