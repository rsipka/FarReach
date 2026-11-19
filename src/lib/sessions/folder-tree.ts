import type { Folder, SessionConfig } from '#lib/ipc/sessions.js';

export interface FolderTreeNode {
	folder: Folder;
	sessions: SessionConfig[];
	children: FolderTreeNode[];
	depth: number;
	totalSessions: number;
}

export interface FolderTree {
	roots: FolderTreeNode[];
	ungrouped: SessionConfig[];
}

type ShowEmpty = (folder: Folder) => boolean;

/** The vault a folder or session lives in: null for this device's Private one. */
export function vaultOf(item: { vault_id?: string | null }): string | null {
	return item.vault_id ?? null;
}

/** The folders of one vault (null: Private). */
export function foldersInVault(folders: Folder[], vault: string | null): Folder[] {
	return folders.filter((folder) => vaultOf(folder) === vault);
}

/**
 * The one vault all these sessions are in, or undefined when they are in
 * several (or there are none): a session can only be filed in a folder of
 * its own vault, so sessions from several vaults share no folder.
 */
export function commonVault(sessions: { vault_id?: string | null }[]): string | null | undefined {
	const vaults = new Set(sessions.map(vaultOf));
	return vaults.size === 1 ? [...vaults][0] : undefined;
}

function hasParentCycle(folderId: string, parentId: string, folders: Map<string, Folder>): boolean {
	const seen = new Set<string>();
	let current: string | null = parentId;
	while (current) {
		if (current === folderId || seen.has(current)) return true;
		seen.add(current);
		current = folders.get(current)?.parent_id ?? null;
	}
	return false;
}

export function buildFolderTree(
	folders: Folder[],
	sessions: SessionConfig[],
	showEmpty: ShowEmpty = () => false,
): FolderTree {
	const folderMap = new Map(folders.map((folder) => [folder.id, folder]));
	const nodes = new Map<string, FolderTreeNode>(
		folders.map((folder) => [folder.id, { folder, sessions: [], children: [], depth: 0, totalSessions: 0 }]),
	);
	const ungrouped: SessionConfig[] = [];

	for (const session of sessions) {
		const node = session.folder_id ? nodes.get(session.folder_id) : undefined;
		// A session filed in another vault's folder (older versions allowed
		// it) shows ungrouped in its own vault rather than pulling that
		// folder into the list.
		if (node && vaultOf(node.folder) === vaultOf(session)) node.sessions.push(session);
		else ungrouped.push(session);
	}

	const roots: FolderTreeNode[] = [];
	for (const folder of folders) {
		const node = nodes.get(folder.id)!;
		const parent = folder.parent_id ? nodes.get(folder.parent_id) : undefined;
		if (parent && !hasParentCycle(folder.id, parent.folder.id, folderMap)) parent.children.push(node);
		else roots.push(node);
	}

	function finish(node: FolderTreeNode, depth: number): FolderTreeNode | null {
		const children = node.children
			.map((child) => finish(child, depth + 1))
			.filter((child): child is FolderTreeNode => child !== null);
		const totalSessions = node.sessions.length + children.reduce((sum, child) => sum + child.totalSessions, 0);
		if (totalSessions === 0 && children.length === 0 && !showEmpty(node.folder)) return null;
		return { ...node, children, depth, totalSessions };
	}

	return {
		roots: roots.map((root) => finish(root, 0)).filter((root): root is FolderTreeNode => root !== null),
		ungrouped,
	};
}

export function folderPath(folderId: string, folders: Folder[], separator = ' / '): string {
	const folderMap = new Map(folders.map((folder) => [folder.id, folder]));
	const names: string[] = [];
	const seen = new Set<string>();
	let current: string | null = folderId;
	while (current && !seen.has(current)) {
		seen.add(current);
		const folder = folderMap.get(current);
		if (!folder) break;
		names.push(folder.name);
		current = folder.parent_id;
	}
	return names.reverse().join(separator);
}

export function canMoveFolder(folderId: string, parentId: string | null, folders: Folder[]): boolean {
	if (!parentId) return true;
	if (folderId === parentId) return false;
	const folderMap = new Map(folders.map((folder) => [folder.id, folder]));
	const folder = folderMap.get(folderId);
	const parent = folderMap.get(parentId);
	if (!folder || !parent || folder.vault_id !== parent.vault_id) return false;
	return !hasParentCycle(folderId, parentId, folderMap);
}

// ---- The saved order of the sessions in a folder ---------------------------

/**
 * Sessions in their saved order: those with a place first, lowest first,
 * then those without one (made or moved here since, or older than the
 * order itself) as they came. Stable, so equal places keep their order.
 */
export function bySavedOrder<T extends { order?: number | null }>(list: T[]): T[] {
	const rank = (s: T) => (typeof s.order === 'number' ? s.order : Number.POSITIVE_INFINITY);
	return list
		.map((s, i) => ({ s, i }))
		.sort((a, b) => rank(a.s) - rank(b.s) || a.i - b.i)
		.map(({ s }) => s);
}

/**
 * The ids of a folder's sessions after putting `moving` (in the order
 * given) at `index` of `ids`, counted before they are taken out. Ids in
 * `moving` that are not in `ids` (sessions coming from another folder) are
 * put in too.
 */
export function placeAt(ids: string[], moving: string[], index: number): string[] {
	const set = new Set(moving);
	const before = ids.slice(0, Math.max(0, Math.min(index, ids.length))).filter((id) => !set.has(id));
	const after = ids.slice(Math.max(0, index)).filter((id) => !set.has(id));
	return [...before, ...moving, ...after];
}

/** The ids with one moved a step up (-1) or down (+1); as they were at an end. */
export function moveStep(ids: string[], id: string, step: -1 | 1): string[] {
	const i = ids.indexOf(id);
	const j = i + step;
	if (i < 0 || j < 0 || j >= ids.length) return ids;
	const out = [...ids];
	[out[i], out[j]] = [out[j], out[i]];
	return out;
}

/** Sessions by name, as people read numbers in them (web2 before web10). */
export function byName<T extends { name: string }>(list: T[]): T[] {
	return [...list].sort((a, b) => a.name.localeCompare(b.name, undefined, { numeric: true, sensitivity: 'base' }));
}
