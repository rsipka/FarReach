/**
 * Tests how a custom editor command from Settings is split into a program
 * and its arguments. Node strips the TypeScript types on import.
 *
 * Run: node scripts/editor-command-test.mjs
 */

import { splitCommand } from '../src/lib/explorer/command-line.ts';

let failures = 0;
const eq = (name, got, want) => {
	const ok = JSON.stringify(got) === JSON.stringify(want);
	if (ok) console.log(`  PASS  ${name}`);
	else {
		failures++;
		console.log(`  FAIL  ${name}\n        got ${JSON.stringify(got)}, want ${JSON.stringify(want)}`);
	}
};

eq('plain words', splitCommand('subl -w'), ['subl', '-w']);
eq('extra spaces', splitCommand('  code   --wait  '), ['code', '--wait']);
eq('double quotes keep spaces', splitCommand('"C:\\Program Files\\Ed\\ed.exe" --new'), ['C:\\Program Files\\Ed\\ed.exe', '--new']);
eq('single quotes', splitCommand("emacsclient -a '' -c"), ['emacsclient', '-a', '', '-c']);
eq('empty', splitCommand('   '), []);

console.log(failures ? `\n${failures} failed` : '\nAll passed');
process.exit(failures ? 1 : 0);
