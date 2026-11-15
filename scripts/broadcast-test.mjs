/**
 * Tests the rules of typing into several terminals at once.
 *
 * Node 24 strips TypeScript types on import, so the exact module the webview
 * runs is what is under test here.
 *
 * Run: node scripts/broadcast-test.mjs
 */

import { live, isOn, recipients, toggle, add, isTerminalReply } from '../src/lib/terminal/broadcast.ts';

let failures = 0;
const check = (name, cond, extra = '') => {
	if (cond) console.log(`  PASS  ${name}`);
	else {
		failures++;
		console.log(`  FAIL  ${name}${extra ? '\n        ' + extra : ''}`);
	}
};
const eq = (a, b) => JSON.stringify(a) === JSON.stringify(b);

const open = ['a', 'b', 'c', 'd'];

console.log('\nchoosing');
check('a terminal goes in', eq(toggle([], 'a'), ['a']));
check('and comes out again', eq(toggle(['a', 'b'], 'a'), ['b']));
check('adding keeps the order and skips those already in', eq(add(['b'], ['a', 'b', 'c']), ['b', 'a', 'c']));
check('closed terminals drop out', eq(live(['a', 'x', 'c'], open), ['a', 'c']));

console.log('\non or off');
check('off with nothing chosen', !isOn([], open));
check('off with one: nobody to send to', !isOn(['a'], open));
check('on with two', isOn(['a', 'b'], open));
check('off when the other one closed', !isOn(['a', 'x'], open));

console.log('\nwhere typing goes');
check('from a chosen one to the other chosen', eq(recipients(['a', 'b', 'c'], 'b', open), ['a', 'c']));
check('never back to itself', !recipients(['a', 'b'], 'a', open).includes('a'));
check('a terminal left out sends nowhere else', eq(recipients(['a', 'b'], 'c', open), []));
check('not to a closed one', eq(recipients(['a', 'x'], 'a', open), []));

console.log('\nwhat counts as typed');
for (const [name, data] of [
	['a letter', 'l'],
	['Enter', '\r'],
	['Ctrl+C', '\x03'],
	['an arrow', '\x1b[A'],
	['an application-mode arrow', '\x1bOA'],
	['Ctrl+Right', '\x1b[1;5C'],
	['F5', '\x1b[15~'],
	['Shift+Tab', '\x1b[Z'],
	['Ctrl+F3', '\x1b[1;5R'],
	['a bracketed paste', '\x1b[200~ls -la\x1b[201~'],
	['a pasted line', 'echo hi\r']
]) {
	check(`${name} is typed`, !isTerminalReply(data));
}
for (const [name, data] of [
	['a cursor position', '\x1b[24;80R'],
	['device attributes', '\x1b[?1;2c'],
	['secondary device attributes', '\x1b[>0;276;0c'],
	['focus in', '\x1b[I'],
	['focus out', '\x1b[O'],
	['a status report', '\x1b[0n'],
	['a mode report', '\x1b[?2004;1$y'],
	['a window size report', '\x1b[8;24;80t'],
	['a colour answer', '\x1b]11;rgb:0000/0000/0000\x1b\\'],
	['a colour answer ended by BEL', '\x1b]10;rgb:ffff/ffff/ffff\x07'],
	['a DCS answer', '\x1bP1$r0m\x1b\\'],
	['an SGR mouse click', '\x1b[<0;12;5M'],
	['an X10 mouse click', '\x1b[M !!']
]) {
	check(`${name} is not typed`, isTerminalReply(data));
}

console.log(failures ? `\n${failures} failed\n` : '\nall passed\n');
process.exit(failures ? 1 : 0);
