/**
 * Edit a server file in the program this computer uses for it (F4 in the
 * full-pane file manager, in the editor chosen in Settings), as WinSCP does:
 * it is downloaded to a temporary
 * folder, opened there, and each time it is saved it is uploaded back.
 */
import { sftpDownload, sftpUpload } from '#lib/ipc/sftp.js';
import { localEditCopyPath, localStat, type LocalStat } from '#lib/ipc/localfs.js';
import { openInEditor } from './editor-launch.js';
import { runTransfer } from './run-transfer.js';

interface Watch {
	localPath: string;
	last: LocalStat;
	timer: ReturnType<typeof setInterval>;
	uploading: boolean;
	lastActive: number;
}

/** How often a saved copy is looked at. */
const POLL_MS = 1500;
/** A copy nobody saved for this long is no longer watched. */
const IDLE_MS = 8 * 60 * 60 * 1000;

const watches = new Map<string, Watch>();

export interface ExternalEditEvents {
	/** The edited copy went back to the server. */
	onUploaded: (name: string) => void;
	onError: (message: string) => void;
}

function stop(key: string): void {
	const w = watches.get(key);
	if (w) clearInterval(w.timer);
	watches.delete(key);
}

/** Open `remotePath` in the system's program for it; saves go back to the
 *  server while the connection lasts. Opening it again reuses the copy. */
export async function editExternally(conn: string, remotePath: string, name: string, size: number, events: ExternalEditEvents): Promise<void> {
	const key = `${conn}\u0000${remotePath}`;
	const existing = watches.get(key);
	if (existing) {
		await openInEditor(existing.localPath);
		return;
	}

	const localPath = await localEditCopyPath(name);
	const done = await runTransfer({
		kind: 'downloading',
		connectionId: conn,
		filename: name,
		totalBytes: size,
		start: (id) => sftpDownload(conn, remotePath, localPath, id)
	});
	if (!done) return;

	const watch: Watch = {
		localPath,
		last: await localStat(localPath),
		uploading: false,
		lastActive: Date.now(),
		timer: setInterval(() => void check(), POLL_MS)
	};
	watches.set(key, watch);

	async function check(): Promise<void> {
		if (watch.uploading) return;
		if (Date.now() - watch.lastActive > IDLE_MS) return stop(key);
		let now: LocalStat;
		try {
			now = await localStat(localPath);
		} catch {
			// The copy was deleted (a temp clean-up, say): nothing to send.
			return stop(key);
		}
		if (now.modifiedMs === watch.last.modifiedMs && now.size === watch.last.size) return;
		watch.last = now;
		watch.lastActive = Date.now();
		watch.uploading = true;
		try {
			const sent = await runTransfer({
				kind: 'uploading',
				connectionId: conn,
				filename: name,
				totalBytes: now.size,
				start: (id) => sftpUpload(conn, localPath, remotePath, id)
			});
			if (sent) events.onUploaded(name);
		} catch (err) {
			// Kept watching: the next save tries again.
			events.onError(err instanceof Error ? err.message : String(err));
		} finally {
			watch.uploading = false;
		}
	}

	await openInEditor(localPath);
}

/** Stop watching every copy of a connection's files (it closed). */
export function stopExternalEdits(conn: string): void {
	for (const key of [...watches.keys()]) if (key.startsWith(`${conn}\u0000`)) stop(key);
}
