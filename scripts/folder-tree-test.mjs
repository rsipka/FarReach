/**
 * Tests the session list's folder rules (#lib/sessions/folder-tree): each
 * vault keeps its own folders, and a session goes only into a folder of
 * its own vault.
 *
 * Run: node --import ./scripts/resolve-lib.mjs scripts/folder-tree-test.mjs
 */

import { buildFolderTree, byName, bySavedOrder, commonVault, foldersInVault, moveStep, placeAt, vaultOf } from '../src/lib/sessions/folder-tree.ts';

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


{
	const ids = (l) => l.map((s) => s.id).join();
	const o = (id, order) => ({ id, name: id, order });
	check('saved order first, the rest as read', ids(bySavedOrder([o('x'), o('b', 1), o('y', null), o('a', 0)])) === 'a,b,x,y');
	check('equal places keep their order', ids(bySavedOrder([o('p', 2), o('q', 2), o('r', 1)])) === 'r,p,q');
	check('no order at all keeps the list', ids(bySavedOrder([o('c'), o('a'), o('b')])) === 'c,a,b');
	const l = ['a', 'b', 'c', 'd'];
	check('drop before another', placeAt(l, ['d'], 1).join() === 'a,d,b,c');
	check('drop after itself changes nothing', placeAt(l, ['b'], 2).join() === 'a,b,c,d');
	check('drop at the end', placeAt(l, ['a'], 4).join() === 'b,c,d,a');
	check('several, in the order given', placeAt(l, ['d', 'a'], 2).join() === 'b,d,a,c');
	check('one from another folder', placeAt(l, ['z'], 0).join() === 'z,a,b,c,d');
	check('move up', moveStep(l, 'c', -1).join() === 'a,c,b,d');
	check('move down', moveStep(l, 'c', 1).join() === 'a,b,d,c');
	check('top stays at the top', moveStep(l, 'a', -1).join() === 'a,b,c,d');
	check('bottom stays at the bottom', moveStep(l, 'd', 1).join() === 'a,b,c,d');
	const n = (name) => ({ name });
	check('by name, numbers as numbers, case ignored', byName([n('web10'), n('Web2'), n('db'), n('web1')]).map((s) => s.name).join() === 'db,web1,Web2,web10');
}

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
