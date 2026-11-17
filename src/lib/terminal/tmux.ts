/**
 * tmux on the server (see src-tauri/src/ssh/tmux.rs): which SSH sessions
 * start inside a tmux session, and under which name.
 *
 * A session says `off` or `attach` itself, or leaves it to its folder; the
 * nearest folder that says on or off decides, else it is off. The name is
 * the session's own, or `reacher-` and the session's name in plain letters.
 */

import type { Folder, SessionConfig, TmuxSetting } from '#lib/ipc/sessions.js';
import type { TmuxStart } from '#lib/ipc/ssh.js';

export type { TmuxStart };

const MAX_NAME = 64;

/** Letters, digits, '-' and '_', not starting with '-': what the backend takes. */
export function validTmuxName(name: string): boolean {
	return name.length > 0 && name.length <= MAX_NAME && !name.startsWith('-') && /^[A-Za-z0-9_-]+$/.test(name);
}

/** "Web 01 (prod)" → "reacher-web-01-prod". */
export function defaultTmuxName(sessionName: string): string {
	const slug = sessionName
		.normalize('NFKD')
		.replace(/[̀-ͯ]/g, '')
		.toLowerCase()
		.replace(/[^a-z0-9]+/g, '-')
		.replace(/^-+|-+$/g, '')
		.slice(0, MAX_NAME - 'reacher-'.length)
		.replace(/-+$/, '');
	return slug ? `reacher-${slug}` : 'reacher';
}

/** The folder setting that applies to a session in `folderId`: the nearest that says. */
export function folderTmux(folderId: string | null | undefined, folders: Folder[]): boolean | null {
	const seen = new Set<string>();
	let id = folderId ?? null;
	while (id && !seen.has(id)) {
		seen.add(id);
		const f = folders.find((x) => x.id === id);
		if (!f) return null;
		if (f.tmux != null) return f.tmux;
		id = f.parent_id;
	}
	return null;
}

/** Whether a session starts in tmux, and as what; null: it does not. */
export function tmuxFor(session: Pick<SessionConfig, 'name' | 'kind' | 'folder_id' | 'tmux'>, folders: Folder[]): TmuxStart | null {
	if (session.kind && session.kind !== 'ssh') return null;
	const own: TmuxSetting | null | undefined = session.tmux;
	const on = own?.mode === 'attach' ? true : own?.mode === 'off' ? false : folderTmux(session.folder_id, folders) === true;
	if (!on) return null;
	const name = own?.name?.trim();
	return { name: name && validTmuxName(name) ? name : defaultTmuxName(session.name) };
}

/**
 * A tmux tab whose connection dropped tries again by itself: nothing is
 * lost on the server. The wait before try `attempt` (counted from 0), or
 * null once the tab has tried for long enough since the drop.
 */
const RETRY_SECONDS = [2, 5, 10, 30];
export const AUTO_RECONNECT_FOR_MS = 10 * 60_000;
export function autoReconnectDelay(attempt: number, sinceDropMs: number): number | null {
	if (sinceDropMs >= AUTO_RECONNECT_FOR_MS) return null;
	return (RETRY_SECONDS[attempt] ?? 60) * 1000;
}

/** A failure that trying again by itself cannot fix: the login needs the
 *  user (a password, a passphrase, a host key, an approval). */
export function needsUser(error: string): boolean {
	return /auth|password|passphrase|host key|host identity|permission denied|denied|approv|refused the key|locked/i.test(error);
}
