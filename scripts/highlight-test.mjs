/**
 * Tests what the terminal highlights, in Node, with no terminal.
 * Addresses here are documentation and private ranges only.
 *
 * Run: node scripts/highlight-test.mjs
 */

import {
	findHighlights,
	isIpv6,
	highlightKinds,
	highlightColors,
	hexColor,
	HIGHLIGHT_KINDS,
	DEFAULT_HIGHLIGHT_KINDS
} from '../src/lib/terminal/highlight.ts';

let failures = 0;
const check = (name, cond, extra = '') => {
	if (cond) console.log(`  PASS  ${name}`);
	else {
		failures++;
		console.log(`  FAIL  ${name}${extra ? '\n        ' + extra : ''}`);
	}
};
const section = (s) => console.log(`\n${s}`);
const ALL = HIGHLIGHT_KINDS;
/** The highlighted text and its kind, as `kind:text`. */
const hl = (line, kinds = ALL) => findHighlights(line, kinds).map((h) => `${h.kind}:${line.slice(h.start, h.end)}`);
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const expect = (name, line, want, kinds) => {
	const got = hl(line, kinds);
	check(name, same(got, want), `got ${JSON.stringify(got)}`);
};

section('words');
expect('an error word, any case', 'ERROR: disk full', ['error:ERROR']);
expect('failed and denied', 'Login failed: permission denied', ['error:failed', 'error:permission denied']);
expect('not inside a longer word', 'errorlevel=0 terror failsafe', []);
expect('not inside a dashed name', 'no-error-handler', []);
expect('a warning', 'WARNING: deprecated option', ['warning:WARNING', 'warning:deprecated']);
expect('a timeout', 'Connection timed out', ['warning:timed out']);
expect('success words', '[  OK  ] Started nginx', ['success:OK']);
expect('success off by default list', '[  OK  ] Started nginx', [], DEFAULT_HIGHLIGHT_KINDS);
expect('a kind turned off is not shown', 'error at 10.0.0.1', ['ipv4:10.0.0.1'], ['ipv4']);

section('IPv4');
expect('an address', 'inet 10.0.0.5 netmask 255.255.255.0', ['ipv4:10.0.0.5', 'ipv4:255.255.255.0']);
expect('with a prefix', 'route 192.0.2.0/24 via 10.0.0.1', ['ipv4:192.0.2.0/24', 'ipv4:10.0.0.1']);
expect('with a port, the address only', 'connect to 10.0.0.7:22', ['ipv4:10.0.0.7']);
expect('at the end of a sentence', 'Reply from 10.0.0.9.', ['ipv4:10.0.0.9']);
expect('not a version with five parts', 'kernel 1.2.3.4.5', []);
expect('not an octet over 255', 'value 10.0.0.256', []);
expect('not inside a word', 'v10.0.0.1x', []);
expect('a prefix over 32 is left off', 'net 10.0.0.0/33', []);

section('IPv6');
check('full form', isIpv6('2001:0db8:0000:0000:0000:ff00:0042:8329'));
check('compressed', isIpv6('2001:db8::1') && isIpv6('::1') && isIpv6('fe80::'));
check('IPv4 tail', isIpv6('::ffff:192.0.2.1'));
check('zone and prefix', isIpv6('fe80::1%eth0') && isIpv6('2001:db8::/32'));
check('not a time', !isIpv6('12:34:56'));
check('not a MAC', !isIpv6('00:11:22:33:44:55'));
check('not two ::', !isIpv6('2001::db8::1'));
check('not nine groups', !isIpv6('1:2:3:4:5:6:7:8:9'));
check('not a group over four digits', !isIpv6('2001:db8::12345'));
check('not a prefix over 128', !isIpv6('2001:db8::/129'));
check('not a bare ::', !isIpv6('::'));
expect('in ip output', 'inet6 fe80::a00:27ff:fe4e:66a1/64 scope link', ['ipv6:fe80::a00:27ff:fe4e:66a1/64']);
expect('a mapped address is one IPv6, not an IPv4 inside', 'from ::ffff:192.0.2.1 port', ['ipv6:::ffff:192.0.2.1']);
expect('a time is not an address', 'Nov 19 12:34:56 host sshd', []);
expect('C++ scope is not an address', 'std::vector<int>', []);

section('MAC');
expect('colon form', 'link/ether 00:1a:2b:3c:4d:5e brd ff:ff:ff:ff:ff:ff', ['mac:00:1a:2b:3c:4d:5e', 'mac:ff:ff:ff:ff:ff:ff']);
expect('dash form, upper case', 'Physical Address. . . : 00-1A-2B-3C-4D-5E', ['mac:00-1A-2B-3C-4D-5E']);
expect('Cisco form', '  1    001a.2b3c.4d5e    DYNAMIC     Gi0/1', ['mac:001a.2b3c.4d5e']);
expect('not mixed separators', '00:1a-2b:3c-4d:5e', []);
expect('not seven pairs', '00:1a:2b:3c:4d:5e:6f', []);

section('overlaps and order');
expect('in order of the line', 'failed to reach 10.0.0.1 (00:1a:2b:3c:4d:5e): timeout', [
	'error:failed',
	'ipv4:10.0.0.1',
	'mac:00:1a:2b:3c:4d:5e',
	'warning:timeout'
]);
expect('nothing to find in an empty line', '   ', []);
expect('nothing with no kinds', 'error 10.0.0.1', [], []);

section('stored settings');
check('the default when nothing is stored', same(highlightKinds(undefined), [...DEFAULT_HIGHLIGHT_KINDS]));
check('unknown kinds are dropped', same(highlightKinds(['mac', 'urls', 'error']), ['error', 'mac']));
check('an empty list stays empty', same(highlightKinds([]), []));

section('colours');
check('#rrggbb is kept', hexColor('#AABBCC') === '#aabbcc');
check('#rgb grows', hexColor('#abc') === '#aabbcc');
check('alpha is dropped', hexColor('#aabbcc80') === '#aabbcc');
check('rgb() turns into hex', hexColor('rgb(255, 0, 16)') === '#ff0010');
check('a name is refused', hexColor('red') === null);
const colors = highlightColors({ brightRed: '#ff0000', brightCyan: 'not a colour' });
check('a theme colour is used', colors.error === '#ff0000');
check('an unusable one falls back', /^#[0-9a-f]{6}$/.test(colors.ipv4) && colors.ipv4 !== 'not a colour');
check('every kind has a colour', ALL.every((k) => /^#[0-9a-f]{6}$/.test(colors[k])));

console.log(failures ? `\n${failures} failed` : '\nAll passed');
process.exit(failures ? 1 : 0);
