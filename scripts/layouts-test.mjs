/**
 * Tests saved split layouts (#lib/terminal/layouts).
 *
 * Run: node scripts/layouts-test.mjs
 */

import { toSaved, savedLeaves, fill, claim, parseLayouts, leafOf } from '../src/lib/terminal/layouts.ts';

let failures = 0;
const check = (name, cond, extra = '') => {
	if (cond) console.log(`  PASS  ${name}`);
	else {
		failures++;
		console.log(`  FAIL  ${name}${extra ? '\n        ' + extra : ''}`);
	}
};
const eq = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const P = (id) => ({ kind: 'pane', id });
const S = (dir, a, b, ratio = 0.5) => ({ kind: 'split', dir, ratio, a, b });

const tabs = [
	{ id: 't1', type: 'ssh', sessionId: 's1', sessionName: 'web1' },
	{ id: 't2', type: 'local' },
	{ id: 't3', type: 'ssh' }, // Quick Connect: no saved session
	{ id: 't4', type: 'rdp', sessionId: 's4' },
	{ id: 't5', type: 'local', wslDistro: 'Ubuntu', localCommand: { program: 'wsl.exe', args: ['-d', 'Ubuntu'] } }
];

console.log('\nwhat a pane keeps');
check('a saved session: its id and name', eq(leafOf(tabs[0]), { session: 's1', name: 'web1' }));
check('a local shell', eq(leafOf(tabs[1]), { local: true }));
check('a WSL shell keeps its distro and program', eq(leafOf(tabs[4]), { local: true, wslDistro: 'Ubuntu', command: { program: 'wsl.exe', args: ['-d', 'Ubuntu'] } }));
check('Quick Connect cannot open again', leafOf(tabs[2]) === null);

console.log('\nsaving');
const layout = S('row', P('t1'), S('column', P('t2'), P('t4'), 0.3), 0.6);
const saved = toSaved(layout, tabs);
check('shape and ratios kept', eq(saved, S('row', { kind: 'pane', leaf: { session: 's1', name: 'web1' } }, S('column', { kind: 'pane', leaf: { local: true } }, { kind: 'pane', leaf: { session: 's4' } }, 0.3), 0.6)));
check('Quick Connect panes left out, the rest close up', eq(toSaved(S('row', P('t3'), S('column', P('t1'), P('t2'))), tabs), S('column', { kind: 'pane', leaf: { session: 's1', name: 'web1' } }, { kind: 'pane', leaf: { local: true } })));
check('fewer than two panes left: nothing to save', toSaved(S('row', P('t3'), P('t1')), tabs) === null && toSaved(null, tabs) === null);

console.log('\nopening');
const leaves = savedLeaves(saved);
check('panes in order', leaves.length === 3 && leaves[0].session === 's1' && leaves[1].local && leaves[2].session === 's4');
check('all filled: the saved shape', eq(fill(saved, ['a', 'b', 'c']), S('row', P('a'), S('column', P('b'), P('c'), 0.3), 0.6)));
check('one missing: the others take its room', eq(fill(saved, ['a', null, 'c']), S('row', P('a'), P('c'), 0.6)));
check('only one there: no split yet', fill(saved, [null, 'b', null]) === null);
check('a new tab claims its session\'s slot', claim(leaves, [null, 'b', null], 's4') === 2);
check('the same session twice fills the next free slot', claim([{ session: 'x' }, { session: 'x' }], ['a', null], 'x') === 1);
check('another session claims nothing', claim(leaves, [null, null, null], 'zz') === -1 && claim(leaves, [null, null, null], undefined) === -1);

console.log('\nreading back');
check('a good list survives', parseLayouts([{ id: '1', name: 'A', tree: saved }]).length === 1);
check('broken entries are dropped', parseLayouts([{ id: '1', name: 'A', tree: { kind: 'split', dir: 'x' } }, null, 5, { id: '2', name: 'B', tree: { kind: 'pane', leaf: {} } }]).length === 0);
check('not a list: empty', eq(parseLayouts({}), []));

console.log(failures ? `\n${failures} failed` : '\nall passed');
process.exit(failures ? 1 : 0);
