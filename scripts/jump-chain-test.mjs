/**
 * Tests the jump chain the session editor saves (#lib/utils/route): taking the
 * jump hosts away must clear them, not keep the chain the session had.
 *
 * Run: node --import ./scripts/resolve-lib.mjs scripts/jump-chain-test.mjs
 */

import { jumpChainToSave } from '../src/lib/utils/route.ts';

let failures = 0;
const check = (name, cond, extra = '') => {
	if (cond) console.log(`  PASS  ${name}`);
	else {
		failures++;
		console.log(`  FAIL  ${name} ${extra}`);
	}
};

const hops = [{ host: 'gw.example.com', port: 22, username: 'j', auth_method: { type: 'Agent' } }];

check('hops kept while jump host is on', jumpChainToSave(false, true, hops) === hops);
check('every hop removed: saved as none', jumpChainToSave(false, true, []) === null);
check('jump host switched off: saved as none', jumpChainToSave(false, false, hops) === null);
check('desktop session: never a jump chain', jumpChainToSave(true, true, hops) === null);

if (failures) {
	console.log(`\n${failures} failed`);
	process.exit(1);
}
console.log('\nall passed');
