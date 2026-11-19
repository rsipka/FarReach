/**
 * Tests the session editor's SSH algorithms choice over ssh_config lines.
 *
 * Run: node scripts/ssh-algorithms-test.mjs
 */

import {
	COMPATIBLE_LINES,
	algorithmMode,
	assemble,
	enabledIn,
	pubkeySha1,
	setEnabled,
	setFlag,
	flagOn,
	syncAccepted,
	withoutAlgorithmLines
} from '../src/lib/utils/sshAlgorithms.ts';

let failures = 0;
const check = (name, cond, extra = '') => {
	if (cond) console.log(`  PASS  ${name}`);
	else {
		failures++;
		console.log(`  FAIL  ${name} ${extra}`);
	}
};
const eq = (a, b) => JSON.stringify(a) === JSON.stringify(b);

const kex = {
	keyword: 'KexAlgorithms',
	algorithms: [
		{ name: 'curve25519-sha256', default: true, weak: null },
		{ name: 'diffie-hellman-group14-sha256', default: true, weak: null },
		{ name: 'diffie-hellman-group-exchange-sha1', default: false, weak: 'sha1' },
		{ name: 'diffie-hellman-group14-sha1', default: false, weak: 'sha1' },
		{ name: 'diffie-hellman-group1-sha1', default: false, weak: 'sha1' }
	]
};
const ciphers = {
	keyword: 'Ciphers',
	algorithms: [
		{ name: 'aes256-ctr', default: true, weak: null },
		{ name: 'aes128-cbc', default: false, weak: 'cbc' },
		{ name: 'aes192-cbc', default: false, weak: 'cbc' },
		{ name: 'aes256-cbc', default: false, weak: 'cbc' },
		{ name: '3des-cbc', default: false, weak: 'cbc' }
	]
};
const macs = {
	keyword: 'MACs',
	algorithms: [
		{ name: 'hmac-sha2-256', default: true, weak: null },
		{ name: 'hmac-sha1', default: false, weak: 'sha1' },
		{ name: 'hmac-sha1-96', default: false, weak: 'short' },
		{ name: 'hmac-md5', default: false, weak: 'md5' }
	]
};
const catalog = [kex, ciphers, macs];

console.log('mode');
check('no lines is default', algorithmMode(['Compression yes', '# note']) === 'default');
check('compatible lines are compatible', algorithmMode(['Compression yes', ...COMPATIBLE_LINES]) === 'compatible');
check('compatible, other case and =', algorithmMode(COMPATIBLE_LINES.map((l) => l.toLowerCase().replace(' ', '='))) === 'compatible');
check('anything else is custom', algorithmMode(['Ciphers +aes128-cbc']) === 'custom');
check('dropping algorithm lines keeps the rest', eq(withoutAlgorithmLines(['Compression yes', ...COMPATIBLE_LINES]), ['Compression yes']));

console.log('lists');
check('assemble +', eq(assemble('+c', ['a', 'b'], ['a', 'b', 'c']), ['a', 'b', 'c']));
check('assemble - with a pattern', eq(assemble('-a*', ['ab', 'b'], ['ab', 'b']), ['b']));
check('assemble ^', eq(assemble('^c', ['a'], ['a', 'c']), ['c', 'a']));
check('defaults without a line', eq(enabledIn([], kex), ['curve25519-sha256', 'diffie-hellman-group14-sha256']));
check('compatible offers group1', enabledIn(COMPATIBLE_LINES, kex).includes('diffie-hellman-group1-sha1'));

console.log('writing');
let l = setEnabled([], kex, ['curve25519-sha256', 'diffie-hellman-group14-sha256', 'diffie-hellman-group1-sha1']);
check('defaults plus one is written as +', eq(l, ['KexAlgorithms +diffie-hellman-group1-sha1']), JSON.stringify(l));
l = setEnabled(l, kex, ['curve25519-sha256', 'diffie-hellman-group14-sha256']);
check('back to the defaults removes the line', eq(l, []), JSON.stringify(l));
l = setEnabled(['Compression yes'], kex, ['diffie-hellman-group1-sha1', 'curve25519-sha256']);
check('without a default: a plain list in engine order', eq(l, ['Compression yes', 'KexAlgorithms curve25519-sha256,diffie-hellman-group1-sha1']), JSON.stringify(l));

console.log('approvals');
const acc = syncAccepted(['StrictHostKeyChecking no', 'Ciphers 3des-cbc'], COMPATIBLE_LINES, catalog);
check('keeps other approvals', acc.includes('StrictHostKeyChecking no'));
check('approves each legacy name', acc.includes('KexAlgorithms diffie-hellman-group1-sha1') && acc.includes('MACs hmac-md5') && acc.includes('Ciphers 3des-cbc'));
check('approves ssh-rsa for key logins', acc.includes('PubkeyAcceptedAlgorithms ssh-rsa') && pubkeySha1(COMPATIBLE_LINES));
const back = syncAccepted(acc, [], catalog);
check('back to secure drops them', eq(back, ['StrictHostKeyChecking no']), JSON.stringify(back));

console.log('forwarding');
l = setFlag(['ForwardAgent no'], 'ForwardAgent', true);
check('agent forwarding on', eq(l, ['ForwardAgent yes']) && flagOn(l, 'ForwardAgent'));
check('agent forwarding off', eq(setFlag(l, 'ForwardAgent', false), []));

if (failures) {
	console.log(`\n${failures} failed`);
	process.exit(1);
}
console.log('\nall passed');
