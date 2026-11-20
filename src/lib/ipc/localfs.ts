import { invoke } from '@tauri-apps/api/core';
import type { FileEntry } from './sftp.js';

/** A folder on this computer: its full path, its parent (none at a root)
 *  and what is in it. Desktop only. */
export interface LocalListing {
	path: string;
	parent: string | null;
	entries: FileEntry[];
}

/** An empty path lists the home folder. */
export async function localListDir(path: string): Promise<LocalListing> {
	return invoke<LocalListing>('local_list_dir', { path });
}

/** Make `name` in `dir`; with `existOk`, a folder already there is fine. */
export async function localMkdir(dir: string, name: string, existOk = false): Promise<string> {
	return invoke<string>('local_mkdir', { dir, name, existOk });
}

/** Rename in place; a name already taken is refused. */
export async function localRename(path: string, newName: string): Promise<string> {
	return invoke<string>('local_rename', { path, newName });
}

/** Delete a file, or a folder with everything in it. */
export async function localDelete(path: string): Promise<void> {
	return invoke('local_delete', { path });
}

/** `dir` + `name` as this computer writes a path. */
export async function localJoin(dir: string, name: string): Promise<string> {
	return invoke<string>('local_join', { dir, name });
}

/** The home folder, then the drives (Windows) or `/`. */
export async function localRoots(): Promise<string[]> {
	return invoke<string[]>('local_roots');
}
