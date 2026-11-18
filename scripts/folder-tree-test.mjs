/**
 * Tests the session list's folder rules (#lib/sessions/folder-tree): each
 * vault keeps its own folders, and a session goes only into a folder of
 * its own vault.
 *
 * Run: node --import ./scripts/resolve-lib.mjs scripts/folder-tree-test.mjs
 */

import { buildFolderTree, commonVault, foldersInVault, vaultOf } from '../src/lib/sessions/folder-tree.ts';

let failures = 0;
const check = (name, cond, extra = '') => {
	if (cond) console.log(`  PASS  ${name}`);
	else {
		failures++;
		console.log(`  FAIL  ${name} ${extra}`);
	}
};

const folder = (id, vault_id, parent_id = null) => ({ id, name: id, parent_id, vault_id });
const session = (id, vault_id, folder_id) => ({ id, name: id, host: 'h', port: 22, username: 'u', folder_id, tags: [], vault_id });

const folders = [folder('home', null), folder('servers', 'team'), folder('web', 'team', 'servers')];

check('Private is null, missing too', vaultOf({}) === null && vaultOf({ vault_id: null }) === null && vaultOf({ vault_id: 'team' }) === 'team');
check('Private folders only under Private', foldersInVault(folders, null).map((f) => f.id).join() === 'home');
check('a vault shows its own folders', foldersInVault(folders, 'team').map((f) => f.id).join() === 'servers,web');
check('a vault no folder is in', foldersInVault(folders, 'other').length === 0);

check('one vault', commonVault([session('a', 'team'), session('b', 'team')]) === 'team');
check('Private', commonVault([session('a', null), session('b', undefined)]) === null);
check('several vaults', commonVault([session('a', 'team'), session('b', null)]) === undefined);
check('no sessions', commonVault([]) === undefined);

{
	// A session an older version filed in another vault's folder.
	const tree = buildFolderTree(folders, [session('a', 'team', 'web'), session('b', null, 'servers')]);
	check('a session goes in its own vault\'s folder', tree.roots.length === 1 && tree.roots[0].children[0].sessions.map((s) => s.id).join() === 'a');
	check('one filed across vaults shows ungrouped', tree.ungrouped.map((s) => s.id).join() === 'b');
}
{
	const tree = buildFolderTree(foldersInVault(folders, null), [], () => true);
	check('an empty Private folder shows under Private', tree.roots.map((n) => n.folder.id).join() === 'home');
	const team = buildFolderTree(foldersInVault(folders, 'team'), [], () => true);
	check('and not under a vault', !team.roots.some((n) => n.folder.id === 'home'));
}

if (failures) {
	console.log(`\n${failures} failed`);
	process.exit(1);
}
console.log('\nAll folder tree tests passed');
