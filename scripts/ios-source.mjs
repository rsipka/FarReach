/**
 * Builds apps.json, the AltStore-style source LiveContainer (and SideStore,
 * AltStore Classic) reads to offer FarReach's unsigned IPA and its updates.
 *
 * release.yml runs it for each tagged release and attaches the result to
 * that release, so the source is always served from one stable address:
 *   https://github.com/<owner>/<repo>/releases/latest/download/apps.json
 *
 * Versions come from the repository's releases that carry an IPA, newest
 * first; the release being made (still a draft) is put first.
 *
 * Run: node scripts/ios-source.mjs --releases releases.json --tag v1.2.3 \
 *        --repo owner/name --info src-tauri/Info.ios.plist --out apps.json
 *   releases.json is `gh api repos/<repo>/releases --paginate --slurp`
 *   (one array, or an array of pages).
 * Test: node scripts/ios-source-test.mjs
 */

import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

export const BUNDLE_ID = 'com.farreach.client';
// Older versions stay listed so one can go back; the newest is what updates.
export const MAX_VERSIONS = 10;
// Matches tauri.ios.conf.json's minimumSystemVersion.
export const MIN_OS = '15.0';

const IPA = /\.ipa$/i;

/** The version an app reports (CFBundleShortVersionString) for a tag: v0.9.2-dev.3 → 0.9.2. */
export const versionOf = (tag) => tag.replace(/^v/i, '').replace(/-.*$/, '');

/**
 * The CFBundleVersion release.yml gives the IPA: the dev number of a dev
 * tag, 1000 for a stable one (so it sorts after that version's dev builds).
 */
export const buildOf = (tag) => tag.match(/-dev\.(\d+)$/)?.[1] ?? '1000';

/** NS…UsageDescription strings from an Info.plist fragment, for appPermissions.privacy. */
export function privacyFrom(plist) {
	const privacy = {};
	const re = /<key>(NS\w+UsageDescription)<\/key>\s*<string>([^<]*)<\/string>/g;
	for (const [, key, text] of plist.matchAll(re)) privacy[key] = text;
	return privacy;
}

/** Release notes as plain text a source can show: no Markdown links or emphasis. */
export function notesOf(body) {
	return (body ?? '')
		.replace(/\r\n/g, '\n')
		.replace(/\[([^\]]+)\]\([^)]+\)/g, '$1')
		.replace(/\*\*([^*]+)\*\*/g, '$1')
		.trim();
}

/**
 * @param {object} o
 * @param {object[]} o.releases GitHub release objects (drafts included).
 * @param {string} o.tag The tag being released now.
 * @param {string} o.repo owner/name
 * @param {Record<string,string>} [o.privacy]
 * @param {Date} [o.now] Date of the release being made.
 */
export function buildSource({ releases, tag, repo, privacy = {}, now = new Date() }) {
	const base = `https://github.com/${repo}`;
	const raw = `https://raw.githubusercontent.com/${repo}/main`;

	const picked = releases
		.filter((r) => r.tag_name === tag || (!r.draft && !r.prerelease))
		.map((r) => ({ r, ipa: (r.assets ?? []).find((a) => IPA.test(a.name)) }))
		.filter(({ ipa }) => ipa);

	const current = picked.find(({ r }) => r.tag_name === tag);
	if (!current) throw new Error(`Release ${tag} has no .ipa asset`);

	const older = picked
		.filter(({ r }) => r.tag_name !== tag)
		.sort((a, b) => Date.parse(b.r.published_at) - Date.parse(a.r.published_at));

	const versions = [current, ...older].slice(0, MAX_VERSIONS).map(({ r, ipa }) => {
		const version = versionOf(r.tag_name);
		const date = r.tag_name === tag ? now : new Date(r.published_at ?? r.created_at);
		return {
			version,
			buildVersion: buildOf(r.tag_name),
			// What is shown: the whole tag, e.g. 0.9.2-dev.3.
			marketingVersion: r.tag_name.replace(/^v/i, ''),
			date: date.toISOString(),
			localizedDescription: notesOf(r.body),
			// Built by tag, not the asset's own URL: a draft's asset URL
			// names an "untagged-…" release that stops working once published.
			downloadURL: `${base}/releases/download/${encodeURIComponent(r.tag_name)}/${encodeURIComponent(ipa.name)}`,
			size: ipa.size,
			minOSVersion: MIN_OS,
		};
	});

	const latest = versions[0];
	const icon = `${raw}/src-tauri/icons/ios/AppIcon-512@2x.png`;

	return {
		name: 'FarReach',
		identifier: BUNDLE_ID,
		subtitle: 'Unsigned FarReach builds for LiveContainer and sideloading',
		description:
			'Unsigned iOS builds of FarReach, a fork of the Reach SSH, SFTP, RDP and VNC client, made by GitHub Actions for each release.',
		iconURL: icon,
		website: base,
		tintColor: '#43E6B0',
		nsfw: false,
		featuredApps: [BUNDLE_ID],
		apps: [
			{
				name: 'FarReach',
				bundleIdentifier: BUNDLE_ID,
				developerName: 'FarReach',
				subtitle: 'SSH, SFTP, RDP and VNC client',
				localizedDescription:
					'FarReach is a fork of Reach: SSH, SFTP, RDP, VNC, Telnet and serial sessions, an encrypted vault and sync. This is an early, unsigned iOS build, meant for LiveContainer or for signing yourself.',
				iconURL: icon,
				tintColor: '#43E6B0',
				category: 'developer',
				versions,
				// The same as the newest entry of versions, for clients that
				// predate the versions list.
				version: latest.version,
				versionDate: latest.date,
				versionDescription: latest.localizedDescription,
				downloadURL: latest.downloadURL,
				size: latest.size,
				appPermissions: { entitlements: [], privacy },
			},
		],
		news: [],
	};
}

function args(argv) {
	const out = {};
	for (let i = 0; i < argv.length; i += 2) {
		if (!argv[i].startsWith('--')) throw new Error(`Unexpected argument ${argv[i]}`);
		out[argv[i].slice(2)] = argv[i + 1];
	}
	return out;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
	const a = args(process.argv.slice(2));
	for (const k of ['releases', 'tag', 'repo', 'out']) {
		if (!a[k]) throw new Error(`--${k} is required`);
	}
	// --paginate --slurp gives an array of pages.
	const releases = JSON.parse(readFileSync(a.releases, 'utf8')).flat();
	const privacy = a.info ? privacyFrom(readFileSync(a.info, 'utf8')) : {};
	const source = buildSource({ releases, tag: a.tag, repo: a.repo, privacy });
	writeFileSync(a.out, JSON.stringify(source, null, 2) + '\n');
	const v = source.apps[0].versions;
	console.log(`apps.json: ${v.length} version(s), newest ${v[0].version} (${v[0].size} bytes)`);
}
