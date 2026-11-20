/**
 * Tests the rules of tabs side by side, split any way (issue #106).
 *
 * Node 24 strips TypeScript types on import, so the exact module the webview
 * runs is what is under test here.
 *
 * Run: node scripts/split-test.mjs
 */

import {
	insert, insertAtEdge, remove, activate, place, setRatio, leaves, contains, clampRatio,
	geometry, paneStyle, dividerStyle, ratioAt, dropAt, neighbour
} from '../src/lib/terminal/split.ts';

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

console.log('\nopening');
const ab = insert(null, 'a', 'b', 'right');
check('the tab on screen stays, the new one goes right', eq(ab, S('row', P('a'), P('b'))));
check('left puts the new one first', eq(insert(null, 'a', 'b', 'left'), S('row', P('b'), P('a'))));
check('top and bottom stack them', eq(insert(null, 'a', 'b', 'bottom'), S('column', P('a'), P('b'))) && eq(insert(null, 'a', 'b', 'top'), S('column', P('b'), P('a'))));
check('a tab is not split with itself', insert(null, 'a', 'a', 'right') === null);

console.log('\nnesting');
const abc = insert(ab, 'b', 'c', 'bottom');
check('a pane splits again, the other way', eq(abc, S('row', P('a'), S('column', P('b'), P('c')))));
check('every tab is in it once', eq(leaves(abc), ['a', 'b', 'c']));
const moved = insert(abc, 'a', 'c', 'top');
check('a tab already shown moves, its old place closes up', eq(moved, S('row', S('column', P('c'), P('a')), P('b'))));
check('an unknown target changes nothing', insert(abc, 'z', 'd', 'left') === abc);
check('along a whole side', eq(insertAtEdge(abc, 'd', 'bottom'), S('column', abc, P('d'), 0.7)));
check('a shown tab moved to a whole side', eq(insertAtEdge(abc, 'a', 'right'), S('row', S('column', P('b'), P('c')), P('a'), 0.7)));
check('no edge drop without a split', insertAtEdge(null, 'a', 'left') === null);

console.log('\nchoosing tabs');
check('choosing a pane keeps the layout', activate(abc, 'a', 'c') === abc);
check('another tab replaces the focused pane', eq(activate(abc, 'b', 'd'), S('row', P('a'), S('column', P('d'), P('c')))));
check('without a split nothing changes', activate(null, 'a', 'c') === null);
check('a focus outside the split changes nothing', activate(abc, 'z', 'd') === abc);

console.log('\ndropping into a pane');
check('two panes trade places', eq(place(abc, 'a', 'c'), S('row', P('c'), S('column', P('b'), P('a')))));
check('a tab not shown takes the pane', eq(place(abc, 'c', 'd'), S('row', P('a'), S('column', P('b'), P('d')))));

console.log('\nclosing');
const r1 = remove(abc, 'b');
check('a pane goes, its sibling takes the room and the focus', eq(r1, { layout: S('row', P('a'), P('c')), focus: 'c' }));
check('down to one pane is no split', eq(remove(ab, 'a'), { layout: null, focus: 'b' }));
check('the focus goes to the first pane of a sibling subtree', remove(abc, 'a').focus === 'b');
check('a tab not shown leaves it', eq(remove(abc, 'z'), { layout: abc }));

console.log('\ndividers');
check('stay between 10% and 90%', clampRatio(0.01) === 0.1 && clampRatio(0.99) === 0.9 && clampRatio(0.6) === 0.6);
check('a broken value falls back to the middle', clampRatio(NaN) === 0.5);
const r2 = setRatio(abc, 'b', 0.25);
check('each split node has its own', r2.ratio === 0.5 && r2.b.ratio === 0.25);
const g = geometry(r2);
check('panes cover the area', eq(g.panes, [
	{ id: 'a', x: 0, y: 0, w: 0.5, h: 1 },
	{ id: 'b', x: 0.5, y: 0, w: 0.5, h: 0.25 },
	{ id: 'c', x: 0.5, y: 0.25, w: 0.5, h: 0.75 }
]));
check('one divider per split node', eq(g.dividers.map((d) => d.path), ['', 'b']));
check('a pane keeps a gap only where it meets another', paneStyle(g.panes[0]) === 'left:0.000%;top:0.000%;right:calc(50.000% + 3px);bottom:0.000%;width:auto;height:auto');
check('inner edges both ways', paneStyle(g.panes[2]).includes('left:calc(50.000% + 3px)') && paneStyle(g.panes[2]).includes('top:calc(25.000% + 3px)'));
check('a nested divider spans only its node', dividerStyle(g.dividers[1]) === 'top:calc(25.000% - 3px);height:6px;left:50.000%;width:50.000%');
check('the pointer moves a nested divider within its node', Math.abs(ratioAt(g.dividers[1], 0.7, 0.5) - 0.5) < 1e-9);

console.log('\ndragging a tab over the terminal');
const W = 1200, H = 800;
check('no split: the right edge of the one pane', eq(dropAt(null, 'a', 1100, 400, W, H), { target: 'a', zone: 'right', preview: { id: 'a', x: 0.5, y: 0, w: 0.5, h: 1 } }));
check('no split: the left side too', dropAt(null, 'a', 50, 400, W, H).zone === 'left');
check('no split: top and bottom', dropAt(null, 'a', 600, 30, W, H).zone === 'top' && dropAt(null, 'a', 600, 780, W, H).zone === 'bottom');
check('the middle is into the pane', dropAt(null, 'a', 600, 400, W, H).zone === 'center');
check('in a split: a pane\'s own side', eq(dropAt(abc, 'a', 700, 150, W, H), { target: 'b', zone: 'left', preview: { id: 'b', x: 0.5, y: 0, w: 0.25, h: 0.5 } }));
check('near the area\'s edge: along the whole side', eq(dropAt(abc, 'a', 600, 790, W, H), { target: null, zone: 'bottom', preview: { x: 0, y: 0.7, w: 1, h: 0.3 } }));
check('outside the area: nothing', dropAt(abc, 'a', -5, 100, W, H) === null);
check('a pane too narrow to halve offers only top, bottom or into it', dropAt(abc, 'a', 610, 600, 800, H).zone !== 'left');
check('a pane too small either way is into it only', dropAt(null, 'a', 10, 10, 300, 200).zone === 'center');

console.log('\nkeyboard');
check('the next tab comes in', neighbour(['a', 'b', 'c'], 'a') === 'b');
check('from the last tab, the one before', neighbour(['a', 'b', 'c'], 'c') === 'b');
check('a single tab has no neighbour', neighbour(['a'], 'a') === null);
check('tabs already on screen are skipped', neighbour(['a', 'b', 'c', 'd'], 'a', ['a', 'b']) === 'c');
check('all on screen: none', neighbour(['a', 'b'], 'a', ['a', 'b']) === null);
check('contains', contains(abc, 'c') && !contains(abc, 'z') && !contains(null, 'a'));

console.log(failures ? `\n${failures} failed` : '\nall passed');
process.exit(failures ? 1 : 0);
