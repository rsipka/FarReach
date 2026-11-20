/**
 * Tests the rules of floating session windows (#lib/terminal/float-windows):
 * handing a terminal over without losing or repeating output, the split a
 * moved group keeps, and window memory across monitors.
 *
 * Run: node --import ./scripts/resolve-lib.mjs scripts/float-windows-test.mjs
 */

import {
	chunkKey, overlap, subLayout, withoutTabs, toPlacement, monitorFor, restoreRect, rectAt,
	keepWindow, windowLeaves, parseKept, parsePlacement, MIN_WIDTH
} from '../src/lib/terminal/float-windows.ts';

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

console.log('\nchunks');
check('bytes and arrays of the same bytes match', chunkKey(new Uint8Array([104, 105])) === chunkKey([104, 105]));
check('text and bytes never match', chunkKey('hi') !== chunkKey([104, 105]));
const big = new Uint8Array(20000).map((_, i) => i % 251);
check('a large chunk keys whole', chunkKey(big).length === 20001);

console.log('\nhandover overlap');
// The giving window showed a..e; the taking one started listening at d.
check('the shared run is dropped once', overlap(['a', 'b', 'c', 'd', 'e'], ['d', 'e', 'f']) === 2);
check('nothing shared: nothing dropped', overlap(['a', 'b'], ['c', 'd']) === 0);
check('the taking window held nothing', overlap(['a', 'b'], []) === 0);
check('the giving window showed nothing yet', overlap([], ['a']) === 0);
check('all it held was already shown', overlap(['a', 'b', 'c'], ['b', 'c']) === 2);
// A prompt repeated: the longest run that fits wins, not the first match.
check('a repeated chunk takes the longest run', overlap(['$ ', 'x', '$ '], ['$ ', 'x', '$ ', 'y']) === 3);
check('a run must end the shown tail', overlap(['a', 'b', 'c'], ['a', 'b']) === 0);

console.log('\nmoving a split');
const tree = S('row', P('a'), S('column', P('b'), P('c')));
check('a pane taken alone is no split', subLayout(tree, ['a']) === null);
check('two panes keep their places', eq(subLayout(tree, ['b', 'c']), S('column', P('b'), P('c'))));
check('the whole split moves as it is', eq(subLayout(tree, ['a', 'b', 'c']), tree));
check('tabs not in the split move without one', subLayout(null, ['x']) === null);
check('the split left behind closes up', eq(withoutTabs(tree, ['b']), S('row', P('a'), P('c'))));

console.log('\nwindow memory');
const left = { name: 'DP-1', position: { x: 0, y: 0 }, size: { width: 2560, height: 1440 }, scaleFactor: 1 };
const right = { name: 'HDMI-1', position: { x: 2560, y: 0 }, size: { width: 1920, height: 1080 }, scaleFactor: 1 };
const p = toPlacement({ x: 2660, y: 100, width: 1200, height: 800 }, false, [left, right]);
check('kept on the monitor its middle is on', eq(p.monitor, { name: 'HDMI-1', width: 1920, height: 1080 }));
check('kept relative to that monitor', p.x === 100 && p.y === 100);
check('back on the same monitor', eq(restoreRect(p, [left, right]), { x: 2660, y: 100, width: 1200, height: 800, maximized: false }));
check('the same monitor found when the order changed', eq(restoreRect(p, [right, left]), { x: 2660, y: 100, width: 1200, height: 800, maximized: false }));
const moved = { ...right, position: { x: -1920, y: 0 } };
check('the same monitor found where it moved to', restoreRect(p, [left, moved]).x === -1820);
const laptop = { name: 'eDP-1', position: { x: 0, y: 0 }, size: { width: 1440, height: 900 }, scaleFactor: 2 };
const alone = restoreRect(p, [laptop]);
check('monitor gone: on the main one', monitorFor(p, [laptop]) === laptop);
check('monitor gone: scaled and inside it', alone.x >= 0 && alone.y >= 0 && alone.x + alone.width <= 1440 && alone.y + alone.height <= 900, JSON.stringify(alone));
const huge = toPlacement({ x: 0, y: 0, width: 2560, height: 1440 }, true, [left]);
const fit = restoreRect(huge, [laptop]);
check('a window larger than the screen is made to fit', fit.width <= 1440 && fit.height <= 900 && fit.maximized);
const renamed = { ...right, name: 'HDMI-1', size: { width: 3840, height: 2160 } };
check('same name, new size: still that monitor', monitorFor(p, [left, renamed]) === renamed);
check('no monitors known: kept as it was', restoreRect(p, []).width === 1200);
const tiny = restoreRect({ ...p, width: 100, height: 50 }, [left, right]);
check('never smaller than the minimum', tiny.width === MIN_WIDTH);

console.log('\ntearing a tab off');
const r = rectAt({ x: 3000, y: 500 }, { width: 1000, height: 700 }, [left, right]);
check('opens under the pointer', r.x < 3000 && r.x + r.width > 3000 && r.y < 500);
const edge = rectAt({ x: 4470, y: 1070 }, { width: 1000, height: 700 }, [left, right]);
check('kept on the screen it is let go on', edge.x + edge.width <= 4480 && edge.y + edge.height <= 1080 && edge.x >= 2560, JSON.stringify(edge));

console.log('\nreopening at start');
const tabs = [
	{ id: '1', type: 'ssh', sessionId: 's1', sessionName: 'web' },
	{ id: '2', type: 'ssh' }, // Quick Connect: cannot open again
	{ id: '3', type: 'local' },
	{ id: '4', type: 'ssh', sessionId: 's2' }
];
const saved = S('row', { kind: 'pane', leaf: { session: 's2' } }, { kind: 'pane', leaf: { local: true } });
const kept = keepWindow(tabs, saved, p);
check('only tabs that open again are kept', eq(kept.tabs, [{ session: 's1', name: 'web' }, { local: true }, { session: 's2' }]));
check('nothing to keep: nothing kept', keepWindow([{ id: '2', type: 'ssh' }], null, p) === null);
const wl = windowLeaves(kept);
check('the split panes open first, the rest after, each once', eq(wl.leaves, [{ session: 's2' }, { local: true }, { session: 's1', name: 'web' }]) && wl.inTree === 2);
check('kept windows read back', eq(parseKept({ 'float-2': kept }), [kept]));
check('junk is dropped', eq(parseKept({ a: 1, b: { tabs: [{ nope: 1 }] }, c: null }), []));
check('not an object: nothing', eq(parseKept('x'), []));
check('a bad placement is dropped, the window kept', parseKept([{ ...kept, placement: { x: 'a' } }])[0].placement === null);
check('a placement reads back', eq(parsePlacement(JSON.parse(JSON.stringify(p))), p));

console.log(`\n${failures === 0 ? 'All passed' : `${failures} failed`}`);
process.exit(failures === 0 ? 0 : 1);
