// Tests for scripts/release.mjs: version math and the file edits, run on the
// repository's own files so a format change there shows up here.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import {
	FILES,
	addedEntryTitles,
	checkVersions,
	compareVersions,
	draftNotes,
	edit,
	nextVersion,
	platformNumbers,
	monthOf,
} from './release.mjs';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');
const read = (f) => readFileSync(join(ROOT, f), 'utf8');
let n = 0;
const test = (name, fn) => {
	fn();
	n++;
};

test('next versions count up within the month', () => {
	const nov = new Date(2026, 10, 18);
	const dec = new Date(2026, 11, 1);
	assert.equal(nextVersion('0.9.2-dev.6', 'dev', nov), '26.11.1-dev.1');
	assert.equal(nextVersion('0.9.2-dev.6', 'stable', nov), '26.11.1');
	assert.equal(nextVersion('26.11.1-dev.1', 'dev', nov), '26.11.1-dev.2');
	assert.equal(nextVersion('26.11.1-dev.2', 'stable', nov), '26.11.1');
	assert.equal(nextVersion('26.11.1', 'dev', nov), '26.11.2-dev.1');
	assert.equal(nextVersion('26.11.1', 'stable', nov), '26.11.2');
	// A new month starts again at 1, also from an unreleased dev line.
	assert.equal(nextVersion('26.11.2-dev.3', 'dev', dec), '26.12.1-dev.1');
	assert.equal(nextVersion('26.11.2-dev.3', 'stable', dec), '26.12.1');
	assert.equal(nextVersion('26.12.4', 'stable', new Date(2027, 0, 5)), '27.1.1');
	assert.throws(() => nextVersion('26.12.1', 'dev', nov), /later month/);
	assert.equal(monthOf(new Date(2030, 0, 1)), '30.1');
	assert.equal(nextVersion('26.11.1', '26.11.5-dev.1'), '26.11.5-dev.1');
	assert.throws(() => nextVersion('0.9.2', '1.0'));
	assert.throws(() => nextVersion('0.9.2', '1.0.0-beta.1'));
	assert.throws(() => nextVersion('0.9.2', '2026.11.1'), /209/);
});

test('ordering', () => {
	assert.ok(compareVersions('0.9.2-dev.4', '0.9.2-dev.5') < 0);
	assert.ok(compareVersions('0.9.2-dev.9', '0.9.2') < 0);
	assert.ok(compareVersions('0.9.2', '0.9.3-dev.1') < 0);
	assert.equal(compareVersions('1.0.0', '1.0.0'), 0);
	// The first date version installs over every 0.9.x build.
	assert.ok(compareVersions('0.9.2-dev.6', '26.11.1-dev.1') < 0);
	assert.ok(compareVersions('26.11.1-dev.9', '26.11.1') < 0);
	assert.ok(compareVersions('26.11.1', '26.11.2-dev.1') < 0);
	assert.ok(compareVersions('26.11.12', '26.12.1-dev.1') < 0);
});

test('Windows and Android numbers match release.yml and keep going up', () => {
	assert.deepEqual(platformNumbers('0.9.2-dev.4'), { wix: '0.9.2.4', versionCode: 902004 });
	assert.deepEqual(platformNumbers('0.9.2'), { wix: '0.9.2.999', versionCode: 902999 });
	assert.deepEqual(platformNumbers('26.11.2-dev.3'), { wix: '26.11.2.3', versionCode: 261102003 });
	assert.deepEqual(platformNumbers('26.11.2'), { wix: '26.11.2.999', versionCode: 261102999 });
	const order = ['0.9.2-dev.4', '0.9.2-dev.5', '0.9.2', '0.9.3-dev.1', '0.10.0', '26.11.1-dev.1', '26.11.1', '26.11.2-dev.1', '26.12.1', '27.1.1-dev.1', '99.12.99'];
	for (let i = 1; i < order.length; i++) {
		assert.ok(platformNumbers(order[i]).versionCode > platformNumbers(order[i - 1]).versionCode, order[i]);
	}
});

test('the repository is consistent now', () => {
	const { problems } = checkVersions(read);
	assert.deepEqual(problems, []);
});

test('a bump edits every file and passes the same checks', () => {
	// Bump from whatever the repository has now, so this keeps passing on a
	// release branch, where the next version is already in the metainfo.
	const current = checkVersions(read).version;
	const v = nextVersion(current, 'dev');
	const stable = nextVersion(current, 'stable', new Date(2099, 0, 1));
	const ctx = { date: '2026-11-18', repo: 'rsipka/ReacherCM' };
	const out = new Map();
	for (const key of ['packageJson', 'packageLock', 'cargoToml', 'cargoLock', 'tauriConf', 'metainfo']) {
		out.set(FILES[key], edit[key](read(FILES[key]), v, ctx));
	}
	const after = checkVersions((f) => out.get(f) ?? read(f));
	assert.equal(after.version, v);
	assert.deepEqual(after.problems, []);
	// Nothing else in the lock files moved.
	for (const f of [FILES.packageLock, FILES.cargoLock]) {
		const a = read(f).split('\n');
		const b = out.get(f).split('\n');
		assert.equal(a.length, b.length);
		assert.equal(a.filter((l, i) => l !== b[i]).length, f === FILES.packageLock ? 2 : 1, f);
	}
	const esc = (x) => x.replace(/\./g, '\\.');
	assert.match(
		out.get(FILES.metainfo),
		new RegExp(`<releases>\n    <release version="${esc(v)}" date="2026-11-18" type="development">\n      <url type="details">https://github\\.com/rsipka/ReacherCM/releases/tag/v${esc(v)}</url>`),
	);
	// A stable release is not marked as development, and a listed one is refused.
	assert.match(edit.metainfo(read(FILES.metainfo), stable, ctx), new RegExp(`<release version="${esc(stable)}" date="2026-11-18">`));
	assert.throws(() => edit.metainfo(read(FILES.metainfo), current, ctx), /already lists/);
});

test('a stable release dates the CHANGELOG section', () => {
	const t = '# Changelog\n\n## Reacher CM (unreleased, based on Reach v0.9.2)\n- **A** — a\n';
	assert.equal(
		edit.changelog(t, '0.9.3', { date: '2026-11-18' }),
		'# Changelog\n\n## Reacher CM (unreleased)\n\n## Reacher CM v0.9.3 (2026-11-18, based on Reach v0.9.2)\n- **A** — a\n',
	);
	assert.match(edit.changelog(read(FILES.changelog), '9.9.9', { date: '2026-11-18' }), /^## Reacher CM v9\.9\.9 \(2026-11-18/m);
});

test('release notes draft', () => {
	const diff = '+- **Serial port sessions** — New\n- **Old** — x\n+text\n+- **tmux windows as tabs** — Each';
	const titles = addedEntryTitles(diff);
	assert.deepEqual(titles, ['Serial port sessions', 'tmux windows as tabs']);
	const notes = draftNotes('0.9.2-dev.5', 'v0.9.2-dev.4', titles);
	assert.match(notes, /^Reacher CM Dev 0\.9\.2-dev\.5/);
	assert.match(notes, /New since v0\.9\.2-dev\.4:\n\n- Serial port sessions\n- tmux windows as tabs\n/);
	assert.match(draftNotes('0.9.3', null, []), /^Reacher CM 0\.9\.3\./);
});

console.log(`release: ${n} tests passed`);
