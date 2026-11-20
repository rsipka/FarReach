/**
 * Tests the full-pane file manager's Compare: folders by listing, text
 * files line by line. Node strips the TypeScript types on import.
 *
 * Run: node scripts/compare-test.mjs
 */

import { compareFolders, diffLines, countChanges } from '../src/lib/explorer/compare.ts';

let failures = 0;
const check = (name, cond, extra = '') => {
	if (cond) console.log(`  PASS  ${name}`);
	else {
		failures++;
		console.log(`  FAIL  ${name}${extra ? '\n        ' + extra : ''}`);
	}
};
const f = (name, size, modified, dir = false) => ({ name, path: `/${name}`, isDirectory: dir, size, modified });

console.log('\nFolders');
{
	const r = compareFolders(
		[f('same', 10, 100), f('newer-left', 10, 500), f('only-left', 1, 1), f('size', 5, 100), f('dir', 0, 0, true)],
		[f('same', 10, 101), f('newer-left', 12, 200), f('only-right', 1, 1), f('size', 6, 100), f('dir2', 0, 0, true)]
	);
	check('same size, time within slack counts as same', r.same === 1, JSON.stringify(r));
	check('left marks: newer, missing on right, size differs', JSON.stringify(r.left.sort()) === JSON.stringify(['/newer-left', '/only-left', '/size']), JSON.stringify(r.left));
	check('right marks: missing on left, size differs', JSON.stringify(r.right.sort()) === JSON.stringify(['/only-right', '/size']), JSON.stringify(r.right));
	check('folders are left alone', !r.left.includes('/dir') && !r.right.includes('/dir2'));
}

console.log('\nLines');
{
	const rows = diffLines('a\nb\nc\nd\n', 'a\nB\nc\nd\ne\n', null);
	const kinds = rows.map((r) => r.kind).join(',');
	check('changed and added lines found', kinds === 'same,changed,same,same,added', kinds);
	check('line numbers kept', rows[4].rightNo === 5 && rows[4].leftNo === undefined);
	check('equal files have no changes', countChanges(diffLines('x\ny', 'x\r\ny\n')) === 0);
	const long = Array.from({ length: 50 }, (_, i) => `line ${i}`);
	const changed = [...long];
	changed[25] = 'CHANGED';
	const folded = diffLines(long.join('\n'), changed.join('\n'));
	check('long unchanged runs fold into gaps', folded.filter((r) => r.kind === 'gap').length === 2 && folded.length === 9, folded.map((r) => r.kind).join(','));
	const removed = diffLines('a\nb\nc', 'a\nc', null);
	check('a removed line', removed.map((r) => r.kind).join(',') === 'same,removed,same');
}

console.log(failures ? `\n${failures} failed` : '\nAll passed');
process.exit(failures ? 1 : 0);
