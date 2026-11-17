// Runs every `npm run <name>:test` script in package.json, one after another,
// so a new test script is picked up by CI and `npm run verify` without being
// listed anywhere else. Each takes a second or so.
//
//   node scripts/script-tests.mjs            all of them
//   node scripts/script-tests.mjs paste ime  only these

import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';

// Not about the app: the Discord tooling is Reach's own.
const SKIP = new Set(['discord:test']);

const { scripts } = JSON.parse(readFileSync(new URL('../package.json', import.meta.url), 'utf8'));
const only = process.argv.slice(2).map((n) => (n.endsWith(':test') ? n : `${n}:test`));
const names = Object.keys(scripts).filter(
	(n) => n.endsWith(':test') && !SKIP.has(n) && (only.length === 0 || only.includes(n)),
);

const failed = [];
for (const name of names) {
	const started = Date.now();
	const run = spawnSync('npm', ['run', '-s', name], { stdio: 'inherit', shell: process.platform === 'win32' });
	const secs = ((Date.now() - started) / 1000).toFixed(1);
	if (run.status === 0) {
		console.log(`ok   ${name} (${secs}s)`);
	} else {
		console.log(`FAIL ${name} (${secs}s)`);
		failed.push(name);
	}
}

console.log(`\n${names.length - failed.length} of ${names.length} script tests passed`);
if (failed.length) {
	console.log(`Failed: ${failed.join(', ')}`);
	process.exit(1);
}
