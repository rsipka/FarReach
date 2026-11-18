#!/usr/bin/env node
// Cuts a Reacher CM release or dev pre-release. It does the steps in
// .github/dev-release.md; release.yml does the rest once the tag is pushed.
//
//   npm run release -- dev                next dev build (0.9.2-dev.4 -> 0.9.2-dev.5,
//                                         0.9.2 -> 0.9.3-dev.1)
//   npm run release -- stable             stable release (0.9.2-dev.4 -> 0.9.2,
//                                         0.9.2 -> 0.9.3)
//   npm run release -- 1.0.0-dev.1        any version
//   npm run release -- tag                after the release PR is merged: tag main
//                                         and push the tag, which starts the builds
//
// Options:
//   --minor, --major   with dev or stable from a stable version: bump that part
//   --direct           commit the bump straight to main, push it and tag it
//                      (no pull request; needs main to accept pushes)
//   --dry-run          show what would change and which commands would run,
//                      change nothing
//   --yes              do not ask before committing or pushing
//   --no-edit          do not offer to edit the release notes
//   --skip-ci-check    tag even without a green full Build check on main
//   --run-ci           (tag) start the full Build check on main and stop
//   --remote <name>    git remote to use (default origin)
//
// The bump step changes the version in package.json, package-lock.json,
// src-tauri/Cargo.toml, src-tauri/Cargo.lock and src-tauri/tauri.conf.json,
// sets the Windows (wix) and Android version numbers, adds the release to the
// Linux metainfo, drafts .github/release-notes.md from the CHANGELOG entries
// added since the last tag, and for a stable release dates the CHANGELOG
// section. It commits that on release/vX.Y.Z and opens a pull request.

import { execFileSync, spawnSync } from 'node:child_process';
import { readFileSync, realpathSync, writeFileSync } from 'node:fs';
import { createInterface } from 'node:readline/promises';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');

export const FILES = {
	packageJson: 'package.json',
	packageLock: 'package-lock.json',
	cargoToml: 'src-tauri/Cargo.toml',
	cargoLock: 'src-tauri/Cargo.lock',
	tauriConf: 'src-tauri/tauri.conf.json',
	metainfo: 'src-tauri/linux/com.reachercm.client.metainfo.xml',
	changelog: 'CHANGELOG.md',
	notes: '.github/release-notes.md',
};
// A green full Build check on an earlier commit still holds when only these
// changed since.
const RELEASE_ONLY = new Set(Object.values(FILES));

// Stable releases take the last build number, so they install over every dev
// build of the same version and under the next version's dev builds.
export const STABLE_BUILD = 999;

const VERSION_RE = /^(\d+)\.(\d+)\.(\d+)(?:-dev\.([1-9]\d*))?$/;

export function parseVersion(v) {
	const m = VERSION_RE.exec(v);
	if (!m) throw new Error(`${v} is not X.Y.Z or X.Y.Z-dev.N`);
	const [major, minor, patch] = [m[1], m[2], m[3]].map(Number);
	const dev = m[4] ? Number(m[4]) : null;
	if (dev !== null && dev >= STABLE_BUILD) throw new Error(`dev number must be below ${STABLE_BUILD}`);
	if (minor > 99 || patch > 99) throw new Error('minor and patch must be below 100 (Android versionCode)');
	return { major, minor, patch, dev };
}

const base = ({ major, minor, patch }) => `${major}.${minor}.${patch}`;

export function nextVersion(current, kind, bump = 'patch') {
	const v = parseVersion(current);
	const next = () => {
		if (bump === 'major') return `${v.major + 1}.0.0`;
		if (bump === 'minor') return `${v.major}.${v.minor + 1}.0`;
		return `${v.major}.${v.minor}.${v.patch + 1}`;
	};
	if (kind === 'dev') {
		if (v.dev !== null && bump === 'patch') return `${base(v)}-dev.${v.dev + 1}`;
		return `${next()}-dev.1`;
	}
	if (kind === 'stable') {
		if (v.dev !== null && bump === 'patch') return base(v);
		return next();
	}
	parseVersion(kind);
	return kind;
}

/** Versions compared as release.yml and the updaters see them. */
export function compareVersions(a, b) {
	const x = parseVersion(a);
	const y = parseVersion(b);
	for (const k of ['major', 'minor', 'patch']) if (x[k] !== y[k]) return x[k] - y[k];
	return (x.dev ?? STABLE_BUILD) - (y.dev ?? STABLE_BUILD);
}

export function platformNumbers(version) {
	const v = parseVersion(version);
	const build = v.dev ?? STABLE_BUILD;
	return {
		wix: `${base(v)}.${build}`,
		versionCode: v.major * 10000000 + v.minor * 100000 + v.patch * 1000 + build,
	};
}

function replaceOnce(text, re, replacement, what) {
	if (!re.test(text)) throw new Error(`could not find ${what}`);
	return text.replace(re, replacement);
}

export const edit = {
	packageJson: (t, v) => replaceOnce(t, /^(  "version": ")[^"]+(")/m, `$1${v}$2`, 'version in package.json'),
	packageLock: (t, v) => {
		const out = replaceOnce(t, /^(  "version": ")[^"]+(")/m, `$1${v}$2`, 'version in package-lock.json');
		return replaceOnce(out, /("packages": \{\s*"": \{\s*"name": "[^"]+",\s*"version": ")[^"]+(")/, `$1${v}$2`, 'root package in package-lock.json');
	},
	cargoToml: (t, v) => replaceOnce(t, /^(\[package\][^[]*?\nversion = ")[^"]+(")/, `$1${v}$2`, '[package] version in Cargo.toml'),
	cargoLock: (t, v) => replaceOnce(t, /(\nname = "reacher-cm"\nversion = ")[^"]+(")/, `$1${v}$2`, 'reacher-cm in Cargo.lock'),
	tauriConf: (t, v) => {
		const { wix, versionCode } = platformNumbers(v);
		let out = replaceOnce(t, /^(  "version": ")[^"]+(")/m, `$1${v}$2`, 'version in tauri.conf.json');
		out = replaceOnce(out, /("wix": \{\s*"version": ")[^"]+(")/, `$1${wix}$2`, 'bundle.windows.wix.version in tauri.conf.json');
		return replaceOnce(out, /("versionCode": )\d+/, `$1${versionCode}`, 'bundle.android.versionCode in tauri.conf.json');
	},
	metainfo: (t, v, { date, repo }) => {
		if (t.includes(`<release version="${v}"`)) {
			throw new Error(`the metainfo already lists ${v} (a release of that number exists, maybe Reach's own); pick a newer version, e.g. npm run release -- stable --minor or -- X.Y.Z`);
		}
		const type = parseVersion(v).dev !== null ? ' type="development"' : '';
		const entry =
			`    <release version="${v}" date="${date}"${type}>\n` +
			`      <url type="details">https://github.com/${repo}/releases/tag/v${v}</url>\n` +
			`    </release>\n`;
		return replaceOnce(t, /(<releases>\n)/, `$1${entry}`, '<releases> in the metainfo');
	},
	/** Stable only: the unreleased section gets the version and date, and a new empty one starts above it. */
	changelog: (t, v, { date }) => {
		const re = /^## Reacher CM \(unreleased(?:, ([^)]*))?\)$/m;
		const m = re.exec(t);
		if (!m) throw new Error('could not find "## Reacher CM (unreleased...)" in CHANGELOG.md');
		const heading = `## Reacher CM v${v} (${date}${m[1] ? `, ${m[1]}` : ''})`;
		return t.replace(re, `## Reacher CM (unreleased)\n\n${heading}`);
	},
};

/** Every place the version lives, read from file contents. */
export function readVersions(read) {
	const tauri = JSON.parse(read(FILES.tauriConf));
	const lock = JSON.parse(read(FILES.packageLock));
	return {
		'package.json': JSON.parse(read(FILES.packageJson)).version,
		'package-lock.json': lock.version,
		'package-lock.json (packages[""])': lock.packages[''].version,
		'Cargo.toml': /^\[package\][^[]*?\nversion = "([^"]+)"/.exec(read(FILES.cargoToml))?.[1],
		'Cargo.lock': /\nname = "reacher-cm"\nversion = "([^"]+)"/.exec(read(FILES.cargoLock))?.[1],
		'tauri.conf.json': tauri.version,
		wix: tauri.bundle?.windows?.wix?.version,
		versionCode: tauri.bundle?.android?.versionCode,
	};
}

/** Problems that would make release.yml, the Rust tests or the installers refuse the version. */
export function checkVersions(read) {
	const found = readVersions(read);
	const version = found['tauri.conf.json'];
	const problems = [];
	for (const [where, v] of Object.entries(found)) {
		if (where === 'wix' || where === 'versionCode') continue;
		if (v !== version) problems.push(`${where} has ${v}, tauri.conf.json has ${version}`);
	}
	try {
		const want = platformNumbers(version);
		if (found.wix !== want.wix) problems.push(`bundle.windows.wix.version is ${found.wix}, should be ${want.wix}`);
		if (found.versionCode !== want.versionCode) problems.push(`bundle.android.versionCode is ${found.versionCode}, should be ${want.versionCode}`);
	} catch (e) {
		problems.push(e.message);
	}
	if (!read(FILES.metainfo).includes(`<release version="${version}"`)) problems.push(`the metainfo has no <release version="${version}">`);
	return { version, problems };
}

/** Titles of the CHANGELOG entries a diff adds (lines like "+- **Title** — ..."). */
export function addedEntryTitles(diff) {
	const titles = [];
	for (const line of diff.split('\n')) {
		const m = /^\+- \*\*(.+?)\*\*/.exec(line);
		if (m) titles.push(m[1]);
	}
	return titles;
}

export function draftNotes(version, previousTag, titles) {
	const dev = parseVersion(version).dev !== null;
	const lines = [dev ? `Reacher CM Dev ${version}, a pre-release built from the fork's main.` : `Reacher CM ${version}.`, ''];
	if (titles.length) {
		lines.push(previousTag ? `New since ${previousTag}:` : 'New in this release:', '');
		for (const t of titles) lines.push(`- ${t}`);
	} else {
		lines.push('Fixes and small changes; see CHANGELOG.md.');
	}
	lines.push(
		'',
		'iPhone and iPad: add https://raw.githubusercontent.com/rsipka/ReacherCM/update-feed/apps.json as a source in LiveContainer to get updates.',
		'Full list of changes: https://github.com/rsipka/ReacherCM/blob/main/CHANGELOG.md',
	);
	return lines.join('\n') + '\n';
}

// ---------------------------------------------------------------------------
// The command line part.

class Abort extends Error {}

function parseArgs(argv) {
	const opts = { remote: 'origin', bump: 'patch' };
	const rest = [];
	for (let i = 0; i < argv.length; i++) {
		const a = argv[i];
		if (a === '--dry-run' || a === '-n') opts.dryRun = true;
		else if (a === '--yes' || a === '-y') opts.yes = true;
		else if (a === '--direct') opts.direct = true;
		else if (a === '--no-edit') opts.noEdit = true;
		else if (a === '--skip-ci-check') opts.skipCi = true;
		else if (a === '--run-ci') opts.runCi = true;
		else if (a === '--minor' || a === '--major') opts.bump = a.slice(2);
		else if (a === '--remote') opts.remote = argv[++i];
		else if (a === '--help' || a === '-h') opts.help = true;
		else if (a.startsWith('-')) throw new Abort(`unknown option ${a}`);
		else rest.push(a);
	}
	if (rest.length !== 1 && !opts.help) throw new Abort('say what to release: dev, stable, X.Y.Z[-dev.N] or tag (--help for more)');
	opts.what = rest[0];
	return opts;
}

function git(args, { allowFail = false } = {}) {
	try {
		return execFileSync('git', args, { cwd: ROOT, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }).trim();
	} catch (e) {
		if (allowFail) return null;
		throw new Abort(`git ${args.join(' ')} failed:\n${e.stderr || e.message}`);
	}
}

function hasGh() {
	return spawnSync('gh', ['--version'], { stdio: 'ignore', shell: process.platform === 'win32' }).status === 0;
}

function gh(args, { allowFail = false } = {}) {
	const r = spawnSync('gh', args, { cwd: ROOT, encoding: 'utf8', shell: process.platform === 'win32' });
	if (r.status !== 0) {
		if (allowFail) return null;
		throw new Abort(`gh ${args.join(' ')} failed:\n${r.stderr}`);
	}
	return r.stdout.trim();
}

function repoSlug(remote) {
	const url = git(['config', '--get', `remote.${remote}.url`]);
	const m = /github\.com[:/]([^/]+\/[^/]+?)(?:\.git)?\/?$/.exec(url);
	if (!m) throw new Abort(`${remote} (${url}) is not a GitHub remote`);
	return m[1];
}

function today() {
	const d = new Date();
	return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
}

async function confirm(opts, question) {
	if (opts.yes || opts.dryRun) return true;
	if (!process.stdin.isTTY) throw new Abort(`${question} Run with --yes to go ahead without asking.`);
	const rl = createInterface({ input: process.stdin, output: process.stdout });
	const answer = (await rl.question(`${question} [y/N] `)).trim().toLowerCase();
	rl.close();
	return answer === 'y' || answer === 'yes';
}

/** Runs a command that changes something, or only prints it in a dry run. */
function act(opts, cmd, args) {
	console.log(`${opts.dryRun ? '[dry run] would run' : '$'} ${cmd} ${args.join(' ')}`);
	if (opts.dryRun) return '';
	return cmd === 'git' ? git(args) : gh(args);
}

function tagExists(remote, tag) {
	return !!git(['ls-remote', '--tags', remote, `refs/tags/${tag}`]) || git(['tag', '--list', tag]) !== '';
}

function showChange(file, before, after) {
	const a = before.split('\n');
	const b = after.split('\n');
	console.log(`\n--- ${file}`);
	// The edits only replace or insert lines near each other, so a plain
	// line-by-line walk with resync is enough to show them.
	let i = 0;
	let j = 0;
	let shown = 0;
	while ((i < a.length || j < b.length) && shown < 40) {
		if (a[i] === b[j]) {
			i++;
			j++;
			continue;
		}
		const k = b.indexOf(a[i], j);
		if (k !== -1 && k - j <= 6) {
			while (j < k) console.log(`+ ${b[j++]}`), shown++;
			continue;
		}
		if (i < a.length) console.log(`- ${a[i++]}`), shown++;
		if (j < b.length) console.log(`+ ${b[j++]}`), shown++;
	}
}

function openEditor(path) {
	const editor = process.env.VISUAL || process.env.EDITOR || (process.platform === 'win32' ? 'notepad' : 'vi');
	const r = spawnSync(editor, [path], { stdio: 'inherit', shell: true });
	if (r.status !== 0) console.log(`(${editor} exited with ${r.status}; keeping the notes as they are)`);
}

async function bump(opts) {
	const { remote } = opts;
	const read = (f) => readFileSync(join(ROOT, f), 'utf8');
	const { version: current, problems } = checkVersions(read);
	const version = nextVersion(current, opts.what, opts.bump);
	const tag = `v${version}`;
	if (compareVersions(version, current) <= 0) throw new Abort(`${version} is not newer than the current ${current}`);
	const stable = parseVersion(version).dev === null;
	console.log(`Release ${current} -> ${version} (${stable ? 'stable release' : 'dev pre-release'})`);
	if (problems.length) console.log(`Note, the current version files disagree (fixed by this bump):\n  ${problems.join('\n  ')}`);

	// Start from an up-to-date, clean main.
	// A dry run only warns, so it can be tried from any branch.
	const need = (ok, msg) => {
		if (ok) return;
		if (!opts.dryRun) throw new Abort(msg);
		console.log(`[dry run] a real run would stop here: ${msg}`);
	};
	const branch = git(['rev-parse', '--abbrev-ref', 'HEAD']);
	need(branch === 'main', `switch to main first (on ${branch})`);
	need(!git(['status', '--porcelain']), 'the working tree has uncommitted changes; commit or stash them first');
	git(['fetch', '--quiet', '--tags', remote, 'main']);
	need(git(['rev-parse', 'HEAD']) === git(['rev-parse', `${remote}/main`]), `main is not the same as ${remote}/main; run git pull (or push) first`);
	if (tagExists(remote, tag)) throw new Abort(`tag ${tag} already exists`);
	const releaseBranch = `release/${tag}`;
	if (!opts.direct && git(['ls-remote', '--heads', remote, releaseBranch])) throw new Abort(`branch ${releaseBranch} already exists on ${remote}`);

	const repo = repoSlug(remote);
	const ctx = { date: today(), repo };
	const changes = new Map();
	for (const key of ['packageJson', 'packageLock', 'cargoToml', 'cargoLock', 'tauriConf', 'metainfo']) {
		changes.set(FILES[key], edit[key](read(FILES[key]), version, ctx));
	}
	if (stable) changes.set(FILES.changelog, edit.changelog(read(FILES.changelog), version, ctx));

	const previousTag = git(['describe', '--tags', '--abbrev=0', '--match', 'v*'], { allowFail: true });
	const diff = previousTag ? git(['diff', `${previousTag}..HEAD`, '--', FILES.changelog]) : '';
	changes.set(FILES.notes, draftNotes(version, previousTag, addedEntryTitles(diff)));

	// Same checks release.yml and the Rust metainfo test make.
	const after = checkVersions((f) => changes.get(f) ?? read(f));
	if (after.problems.length) throw new Abort(`the bump left problems:\n  ${after.problems.join('\n  ')}`);

	for (const [file, text] of changes) {
		if (file === FILES.notes) console.log(`\n--- ${file} (new draft)\n${text.replace(/^/gm, '  ')}`);
		else showChange(file, read(file), text);
	}
	console.log('');

	if (opts.dryRun) {
		console.log('[dry run] no files written.');
	} else {
		for (const [file, text] of changes) writeFileSync(join(ROOT, file), text);
		console.log(`Release notes drafted in ${FILES.notes} (the release description and what the updater shows; keep it short, plain text).`);
		if (!opts.noEdit && !opts.yes && process.stdin.isTTY && (await confirm({}, 'Edit the release notes now?'))) openEditor(join(ROOT, FILES.notes));
	}

	const files = [...changes.keys()];
	const message = stable ? `Release Reacher CM ${tag}` : `Release Reacher CM Dev ${tag}`;
	if (opts.direct) {
		// Before main changes, so a stale check does not leave main bumped but untagged.
		if (!opts.dryRun) await ciCheck(opts, repo, git(['rev-parse', 'HEAD']));
		if (!(await confirm(opts, `Commit ${tag} on main, push main and the tag to ${remote}?`))) return restore(files);
		act(opts, 'git', ['add', ...files]);
		act(opts, 'git', ['commit', '-m', message]);
		act(opts, 'git', ['push', remote, 'main']);
		return tagAndPush(opts, { expect: version });
	}

	if (!(await confirm(opts, `Commit on ${releaseBranch}, push it and open a pull request?`))) return restore(files);
	act(opts, 'git', ['switch', '-c', releaseBranch]);
	act(opts, 'git', ['add', ...files]);
	act(opts, 'git', ['commit', '-m', message]);
	act(opts, 'git', ['push', '-u', remote, releaseBranch]);
	const body = `Bumps the version to ${version} and drafts the release notes. After merging, run \`npm run release -- tag\` to tag main, which starts release.yml.`;
	if (hasGh()) {
		const url = act(opts, 'gh', ['pr', 'create', '--repo', repo, '--base', 'main', '--head', releaseBranch, '--title', message, '--body', body]);
		if (url) console.log(url);
	} else {
		console.log(`Open the pull request: https://github.com/${repo}/compare/main...${releaseBranch}?expand=1`);
	}
	if (!opts.dryRun) act(opts, 'git', ['switch', 'main']);
	console.log(`\nNext: merge the pull request, then run  npm run release -- tag`);
}

function restore(files) {
	git(['checkout', '--', ...files]);
	console.log('Stopped; the files are back as they were.');
}

/** The newest successful full Build check on main (nightly or Run workflow). */
function lastFullBuildCheck(repo) {
	const out = gh(['run', 'list', '--repo', repo, '--workflow', 'build-check.yml', '--branch', 'main', '--limit', '30', '--json', 'event,headSha,conclusion,status,url,createdAt'], { allowFail: true });
	if (out === null) return null;
	const runs = JSON.parse(out).filter((r) => r.event === 'schedule' || r.event === 'workflow_dispatch');
	return { green: runs.find((r) => r.conclusion === 'success'), running: runs.find((r) => r.status !== 'completed') };
}

async function ciCheck(opts, repo, sha) {
	if (opts.skipCi) return console.log('Skipping the Build check check (--skip-ci-check).');
	if (!hasGh()) throw new Abort('gh (GitHub CLI) is needed to check that the full Build check is green on main. Install it and run gh auth login, or pass --skip-ci-check if you checked it yourself.');
	const runs = lastFullBuildCheck(repo);
	if (!runs) throw new Abort('could not list Build check runs (gh auth login?); pass --skip-ci-check to tag anyway');
	const green = runs.green;
	if (green && git(['cat-file', '-e', `${green.headSha}^{commit}`], { allowFail: true }) !== null) {
		const changed = git(['diff', '--name-only', green.headSha, sha]).split('\n').filter(Boolean);
		const other = changed.filter((f) => !RELEASE_ONLY.has(f));
		if (other.length === 0) return console.log(`Full Build check is green for this code: ${green.url}`);
		console.log(`The last green full Build check (${green.url}) is older than ${other.length} changed file(s), e.g. ${other.slice(0, 3).join(', ')}.`);
	} else {
		console.log('No green full Build check (nightly or Run workflow) on main found.');
	}
	if (runs.running) throw new Abort(`a full Build check is running now: ${runs.running.url}\nTag once it is green.`);
	if (opts.runCi) {
		act(opts, 'gh', ['workflow', 'run', 'build-check.yml', '--repo', repo, '--ref', 'main']);
		throw new Abort(`started the full Build check on main: https://github.com/${repo}/actions/workflows/build-check.yml\nRun npm run release -- tag again once it is green (about 45 minutes).`);
	}
	throw new Abort('run it first with  npm run release -- tag --run-ci  (or pass --skip-ci-check)');
}

async function tagAndPush(opts, { expect } = {}) {
	const { remote } = opts;
	const repo = repoSlug(remote);
	let ref = `${remote}/main`;
	if (opts.dryRun && expect) {
		ref = 'HEAD';
		console.log('[dry run] checking the working tree as it would be after the bump is skipped; the bump above already passed the same checks.');
	} else {
		git(['fetch', '--quiet', '--tags', remote, 'main']);
	}
	const sha = git(['rev-parse', ref]);
	if (!(opts.dryRun && expect)) {
		const { version, problems } = checkVersions((f) => git(['show', `${ref}:${f}`]));
		if (problems.length) throw new Abort(`${ref} is not ready to tag:\n  ${problems.join('\n  ')}\nMerge the release pull request first.`);
		if (expect && version !== expect) throw new Abort(`${ref} has ${version}, expected ${expect}`);
		expect = version;
	}
	const tag = `v${expect}`;
	if (tagExists(remote, tag)) throw new Abort(`tag ${tag} already exists. Merge the release pull request first, or bump with  npm run release -- dev  (or stable).`);
	const notes = git(['show', `${ref}:${FILES.notes}`], { allowFail: true }) ?? '';
	console.log(`\nTag ${tag} on ${ref} (${sha.slice(0, 10)}), ${parseVersion(expect).dev !== null ? 'dev pre-release' : 'stable release'}.`);
	if (notes && !opts.dryRun) console.log(`Release notes:\n${notes.replace(/^/gm, '  ')}`);
	await ciCheck(opts, repo, sha);
	if (!(await confirm(opts, `Push tag ${tag}? This starts the release builds.`))) return console.log('Stopped; nothing tagged.');
	act(opts, 'git', ['tag', '-a', tag, '-m', `Reacher CM ${tag}`, sha]);
	act(opts, 'git', ['push', remote, `refs/tags/${tag}`]);
	if (opts.dryRun) return console.log('\n[dry run] nothing tagged or pushed.');
	console.log(`\nBuilds started: https://github.com/${repo}/actions/workflows/release.yml`);
	console.log('The release is published when every platform has built; a failed job leaves it as a draft, re-run that job.');
}

async function main() {
	const opts = parseArgs(process.argv.slice(2));
	if (opts.help) {
		const head = readFileSync(fileURLToPath(import.meta.url), 'utf8').split('\n').slice(1);
		console.log(head.slice(0, head.findIndex((l) => !l.startsWith('//'))).map((l) => l.replace(/^\/\/ ?/, '')).join('\n'));
		return;
	}
	if (opts.what === 'tag') return tagAndPush(opts);
	if (opts.what !== 'dev' && opts.what !== 'stable' && !VERSION_RE.test(opts.what)) throw new Abort(`unknown release ${opts.what}: use dev, stable, X.Y.Z[-dev.N] or tag`);
	return bump(opts);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === realpathSync(process.argv[1])) {
	main().catch((e) => {
		console.error(`release: ${e instanceof Abort || !process.env.DEBUG ? e.message : e.stack}`);
		process.exit(1);
	});
}
