/**
 * Tests the LiveContainer source (apps.json) release.yml builds.
 *
 * Run: node scripts/ios-source-test.mjs
 */

import { buildSource, privacyFrom, notesOf, MAX_VERSIONS } from './ios-source.mjs';

let failures = 0;
const eq = (name, got, want) => {
	const g = JSON.stringify(got);
	const w = JSON.stringify(want);
	if (g === w) console.log(`  PASS  ${name}`);
	else {
		failures++;
		console.log(`  FAIL  ${name}\n        got  ${g}\n        want ${w}`);
	}
};
const throws = (name, fn) => {
	try {
		fn();
		failures++;
		console.log(`  FAIL  ${name} (no error)`);
	} catch {
		console.log(`  PASS  ${name}`);
	}
};

const rel = (tag, published, assets, extra = {}) => ({
	tag_name: tag,
	published_at: published,
	created_at: published ?? '2026-12-01T00:00:00Z',
	body: `Notes for ${tag}`,
	draft: false,
	prerelease: false,
	assets,
	...extra,
});
const ipa = (name, size) => ({ name, size, browser_download_url: `https://x/untagged-1/${name}` });

const releases = [
	rel('v1.1.0', null, [ipa('FarReach_1.1.0_ios.ipa', 300), { name: 'latest.json', size: 1 }], { draft: true }),
	rel('v1.0.0', '2026-11-20T10:00:00Z', [ipa('FarReach_1.0.0_ios.ipa', 200)]),
	rel('v1.0.1-beta', '2026-11-25T10:00:00Z', [ipa('FarReach_1.0.1-beta_ios.ipa', 250)], { prerelease: true }),
	rel('v0.9.2', '2026-11-10T10:00:00Z', [{ name: 'FarReach_0.9.2_amd64.deb', size: 5 }]),
	rel('v0.9.9', '2026-11-18T10:00:00Z', [ipa('FarReach_0.9.9_ios.ipa', 150)]),
	rel('v0.0.1', null, [ipa('old.ipa', 1)], { draft: true }),
];
const now = new Date('2026-12-01T12:00:00Z');
const src = buildSource({ releases, tag: 'v1.1.0', repo: 'rsipka/FarReach', now, privacy: { NSLocalNetworkUsageDescription: 'x' } });
const app = src.apps[0];

console.log('\nversions');
eq('newest first, then by publish date; pre-releases in, other drafts and IPA-less releases out',
	app.versions.map((v) => v.marketingVersion), ['1.1.0', '1.0.1-beta', '1.0.0', '0.9.9']);
eq('download URL by tag, not the draft asset URL', app.versions[0].downloadURL,
	'https://github.com/rsipka/FarReach/releases/download/v1.1.0/FarReach_1.1.0_ios.ipa');
eq('size in bytes from the asset', app.versions.map((v) => v.size), [300, 250, 200, 150]);
eq('the release being made is dated now', app.versions[0].date, '2026-12-01T12:00:00.000Z');
eq('older ones keep their publish date', app.versions[2].date, '2026-11-20T10:00:00.000Z');
eq('stable build number', app.versions[0].buildVersion, '1000');
const dev = buildSource({ releases: [rel('v0.9.2-dev.3', null, [ipa('d.ipa', 9)], { draft: true })], tag: 'v0.9.2-dev.3', repo: 'a/b', now }).apps[0].versions[0];
eq('a dev tag: numeric version, dev number as build, full tag shown', [dev.version, dev.buildVersion, dev.marketingVersion], ['0.9.2', '3', '0.9.2-dev.3']);
eq('notes carried', app.versions[2].localizedDescription, 'Notes for v1.0.0');

console.log('\nlegacy fields mirror the newest version');
eq('version', app.version, '1.1.0');
eq('downloadURL', app.downloadURL, app.versions[0].downloadURL);
eq('size', app.size, 300);

console.log('\nsource');
eq('bundle id', app.bundleIdentifier, 'com.farreach.client');
eq('privacy passed through', app.appPermissions, { entitlements: [], privacy: { NSLocalNetworkUsageDescription: 'x' } });
throws('a release with no IPA is refused', () => buildSource({ releases, tag: 'v0.9.2', repo: 'a/b' }));

const many = Array.from({ length: 15 }, (_, i) =>
	rel(`v1.${i}.0`, `2026-01-${String(i + 1).padStart(2, '0')}T00:00:00Z`, [ipa(`a${i}.ipa`, i)]));
eq(`at most ${MAX_VERSIONS} versions`, buildSource({ releases: many, tag: 'v1.14.0', repo: 'a/b' }).apps[0].versions.length, MAX_VERSIONS);

console.log('\nhelpers');
eq('privacy from Info.plist', privacyFrom('<key>NSLocalNetworkUsageDescription</key>\n\t<string>Local net.</string><key>Other</key><string>no</string>'),
	{ NSLocalNetworkUsageDescription: 'Local net.' });
eq('notes lose Markdown links and bold', notesOf('**Fix** see [docs](https://x)\r\n'), 'Fix see docs');

console.log(failures ? `\n${failures} failure(s)` : '\nAll passed');
process.exit(failures ? 1 : 0);
