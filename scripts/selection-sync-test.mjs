/**
 * Tests the deferred Linux primary-selection copy (#lib/terminal/selection-sync).
 *
 * The selection service double records what xterm's `refresh(isLinux)` would
 * do: queue a redraw always, and copy the text to the primary selection only
 * when the flag is set (xterm 6.0.0 SelectionService.refresh).
 *
 * Run: node scripts/selection-sync-test.mjs
 */

import { deferLinuxSelectionSync } from '../src/lib/terminal/selection-sync.ts';

let failures = 0;
const check = (name, cond, extra = '') => {
	if (cond) console.log(`  PASS  ${name}`);
	else {
		failures++;
		console.log(`  FAIL  ${name}${extra ? '\n        ' + extra : ''}`);
	}
};

function target() {
	const listeners = new Map();
	return {
		addEventListener: (t, fn) => listeners.set(t, fn),
		removeEventListener: (t, fn) => listeners.get(t) === fn && listeners.delete(t),
		fire: (t, e) => listeners.get(t)?.(e),
		has: (t) => listeners.has(t)
	};
}

function setup() {
	const log = { redraws: 0, primary: 0 };
	const service = {
		refresh(isLinux) {
			log.redraws++;
			if (isLinux) log.primary++;
		}
	};
	const term = { _core: { _selectionService: service } };
	const host = target();
	const win = target();
	const queued = [];
	const undo = deferLinuxSelectionSync(term, { host, win, later: (fn) => queued.push(fn) });
	const flush = () => queued.splice(0).forEach((fn) => fn());
	return { log, service, host, win, undo, flush };
}

console.log('\nA drag');
{
	const { log, service, host, win, flush } = setup();
	host.fire('mousedown', { button: 0 });
	service.refresh(true); // xterm's own mousedown
	for (let i = 0; i < 50; i++) service.refresh(true); // mouse moves
	check('the highlight is redrawn on every move', log.redraws === 51, `redraws ${log.redraws}`);
	check('nothing goes to the primary selection while dragging', log.primary === 0, `primary ${log.primary}`);
	win.fire('mouseup', { button: 0 });
	flush();
	check('the primary selection gets the text once, on release', log.primary === 1, `primary ${log.primary}`);
}

console.log('\nOutside a drag');
{
	const { log, service, flush } = setup();
	service.refresh(true); // a right click selecting a word
	flush();
	check('a copy outside a drag goes at once', log.primary === 1);
	service.refresh();
	check('a plain redraw is untouched', log.redraws === 2 && log.primary === 1);
}

console.log('\nOther buttons and a click with no move');
{
	const { log, service, host, win, flush } = setup();
	host.fire('mousedown', { button: 2 });
	service.refresh(true);
	check('a right button press is not a drag', log.primary === 1);
	host.fire('mousedown', { button: 0 });
	win.fire('mouseup', { button: 0 });
	flush();
	check('a click that never asked for a copy adds none', log.primary === 1);
}

console.log('\nUndo');
{
	const { service, host, win, undo } = setup();
	const wrapped = service.refresh;
	undo();
	check('refresh is put back', service.refresh !== wrapped);
	check('listeners are removed', !host.has('mousedown') && !win.has('mouseup'));
}

console.log('\nUnknown xterm');
{
	let warned = '';
	const undo = deferLinuxSelectionSync({}, { host: target(), win: target(), warn: (m) => (warned = m) });
	check('warns and does nothing', warned.includes('unrecognised') && typeof undo === 'function');
}

console.log(failures ? `\n${failures} failed` : '\nAll passed');
process.exit(failures ? 1 : 0);
