/**
 * Tests how the What's new dialog reads release notes (#lib/updater/release-notes).
 *
 * Run: node --import ./scripts/resolve-lib.mjs scripts/release-notes-test.mjs
 */

import { parseReleaseNotes, splitLinks } from '../src/lib/updater/release-notes.ts';

let failures = 0;
const check = (name, cond, extra = '') => {
	if (cond) console.log(`  PASS  ${name}`);
	else {
		failures++;
		console.log(`  FAIL  ${name} ${extra}`);
	}
};
const eq = (a, b) => JSON.stringify(a) === JSON.stringify(b);

// The shape the release workflow publishes (.github/release-notes.md).
const notes = `Reacher CM Dev 26.11.1-dev.4, a pre-release
built from the fork's main.

New since v26.11.1-dev.3:

- Face ID on iPhone
- MikroTik and UniFi devices get their own icon

Full list of changes: https://example.com/owner/repo/blob/main/CHANGELOG.md`;

const blocks = parseReleaseNotes(notes);
check('kinds in order', eq(blocks.map((b) => b.kind), ['para', 'para', 'item', 'item', 'para']), JSON.stringify(blocks));
check('paragraph lines joined', eq(blocks[0].parts, [{ text: "Reacher CM Dev 26.11.1-dev.4, a pre-release built from the fork's main." }]));
check('list item text without the dash', eq(blocks[2].parts, [{ text: 'Face ID on iPhone' }]));
check('link picked out', eq(blocks[4].parts, [{ text: 'Full list of changes: ' }, { url: 'https://example.com/owner/repo/blob/main/CHANGELOG.md' }]));

check('empty notes', eq(parseReleaseNotes(null), []) && eq(parseReleaseNotes('  \n\n'), []));
check('heading', eq(parseReleaseNotes('## Fixes')[0], { kind: 'heading', parts: [{ text: 'Fixes' }] }));
check('numbered and starred items', eq(parseReleaseNotes('1. one\n* two').map((b) => b.kind), ['item', 'item']));
check('CRLF', parseReleaseNotes('a\r\n\r\nb').length === 2);

check('trailing full stop not in link', eq(splitLinks('See https://example.com/x.'), [{ text: 'See ' }, { url: 'https://example.com/x' }, { text: '.' }]));
check('link in brackets', eq(splitLinks('(https://example.com/a)'), [{ text: '(' }, { url: 'https://example.com/a' }, { text: ')' }]));
check('markdown link keeps label', eq(splitLinks('[docs](https://example.com/d)'), [{ text: 'docs (' }, { url: 'https://example.com/d' }, { text: ')' }]));
check('bold and code markers dropped', eq(splitLinks('**New** `npm run x`'), [{ text: 'New npm run x' }]));
check('only http(s) is a link', eq(splitLinks('javascript:alert(1) file:///etc/passwd'), [{ text: 'javascript:alert(1) file:///etc/passwd' }]));

if (failures) {
	console.log(`\n${failures} failed`);
	process.exit(1);
}
console.log('\nall passed');
