/**
 * Tests which of a session's addresses a connect uses (#lib/sessions/addresses):
 * their order, which go through the session's route, and when a failure moves
 * on to the next one.
 *
 * Run: node --import ./scripts/resolve-lib.mjs scripts/addresses-test.mjs
 */

import { addressKey, autoOrder, checksAddresses, plainOrder, connectMode, hasAltAddresses, isUnreachable, sessionAddresses } from '../src/lib/sessions/addresses.ts';

let failures = 0;
const check = (name, cond, extra = '') => {
	if (cond) console.log(`  PASS  ${name}`);
	else {
		failures++;
		console.log(`  FAIL  ${name} ${extra}`);
	}
};

const plain = { host: 'server.example.com', port: 22 };
check('no other addresses: main only', !hasAltAddresses(plain) && connectMode(plain) === 'main' && sessionAddresses(plain).length === 1);

const jump = [{ host: 'gw.example.com', port: 22, username: 'j', auth_method: { type: 'Agent' } }];
const s = {
	host: 'server.example.com',
	port: 2222,
	host_label: 'Internet',
	jump_chain: jump,
	alt_addresses: [
		{ label: 'Office', host: '10.0.0.5', port: 22, direct: true },
		{ host: '2001:db8::5', direct: true },
		{ label: 'Blank', host: '  ' },
	],
};
const all = sessionAddresses(s);
check('blank addresses are left out', all.length === 3, JSON.stringify(all));
check('main first, through the jump host', all[0].host === 'server.example.com' && all[0].routed && all[0].label === 'Internet');
check('a direct address skips the route', !all[1].routed && all[1].port === 22);
check("an address without a port takes the session's", all[2].port === 2222);
check('mode defaults to auto', connectMode(s) === 'auto');
check('ask is kept', connectMode({ ...s, connect_mode: 'ask' }) === 'ask');
check('other kinds ignore other addresses', !hasAltAddresses({ ...s, kind: 'rdp' }));
check('IPv6 keys are bracketed', addressKey(all[2]) === '[2001:db8::5]:2222');

// The office answers: it goes first, then the routed main, then the silent one.
let order = autoOrder(all, [undefined, 3, null]);
check('answering direct first', order.map((a) => a.index).join() === '1,0,2', order.map((a) => a.index).join());
// Away from the office: the routed main first.
order = autoOrder(all, [undefined, null, null]);
check('routed before the silent ones', order.map((a) => a.index).join() === '0,1,2', order.map((a) => a.index).join());
// Both direct ones answer, IPv6 worked last time.
order = autoOrder(all, [undefined, 3, 4], '[2001:db8::5]:2222');
check('last good goes first within its group', order.map((a) => a.index).join() === '2,1,0', order.map((a) => a.index).join());
// The last good one is routed while the office answers: the office still wins.
order = autoOrder(all, [undefined, 3, null], 'server.example.com:2222');
check('last good never beats an answering direct one', order[0].index === 1);

// Silent mode: in priority order, the last good one first.
check('silent keeps priority order', plainOrder(all).map((a) => a.index).join() === '0,1,2');
check('silent puts the last good first', plainOrder(all, '10.0.0.5:22').map((a) => a.index).join() === '1,0,2');
check('the session wins over the setting', checksAddresses({ address_check: false }, true) === false && checksAddresses({ address_check: true }, false) === true);
check('unset follows the setting', checksAddresses({}, false) === false && checksAddresses({ address_check: null }, true) === true);

check('a timeout moves on', isUnreachable('Connection failed: Connection timed out'));
check('a refused port moves on', isUnreachable('Connection failed: could not reach 10.0.0.5:22: connection refused'));
check('a refused login does not', !isUnreachable('Authentication rejected — server did not accept the public key'));
check('a key error does not', !isUnreachable('Connection failed: Auth error: bad passphrase'));
check('an unknown server key turned down does not', !isUnreachable('Connection failed: Unknown server key'));
check('a name that does not resolve moves on', isUnreachable('Connection failed: failed to lookup address information: Name or service not known'));
check('a rejected host key does not', !isUnreachable('Connection failed: Host key rejected by user'));

if (failures) {
	console.log(`\n${failures} failed`);
	process.exit(1);
}
console.log('\nall passed');
