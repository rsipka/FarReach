/**
 * Session history in the window: what each tab records (for the bottom-bar
 * indicator) and the search panel's state. The data itself is in the
 * backend's encrypted file; see src-tauri/src/history.
 */
import { historyConfigure, historyPause, type TapStatus } from '#lib/ipc/history.js';
import { sessionLogPause, sessionLogStatus, sessionLogStop } from '#lib/ipc/ssh.js';
import { getSettings } from '#lib/state/settings.svelte.js';
import { getTerminalInput } from '#lib/state/snippets.svelte.js';

/** A tab's file log: where it writes (null until known) and whether it is paused. */
export interface TabLog {
	path: string | null;
	paused: boolean;
}

export interface TabTap {
	stream: string;
	status: TapStatus;
	/** Null when the tab is not logged to a file, or logging was stopped. */
	log: TabLog | null;
}

/** By tab id: its stream, what it records and its file log. */
const taps = $state<Record<string, TabTap>>({});

/**
 * `sshLog`: an SSH tab writes its own log, opened at connect; its path is
 * asked for here.
 */
export function setTabHistory(tabId: string, stream: string, status: TapStatus, sshLog = false): void {
	const log = status.logFile ? { path: status.logFile, paused: false } : sshLog ? { path: null, paused: false } : null;
	taps[tabId] = { stream, status, log };
	if (sshLog) {
		void sessionLogStatus(stream)
			.then((st) => {
				const tap = taps[tabId];
				if (st && tap?.stream === stream && tap.log) tap.log = { path: st.path, paused: st.paused };
			})
			.catch(() => {});
	}
}

export function clearTabHistory(tabId: string): void {
	delete taps[tabId];
}

export function tabHistory(tabId: string | undefined): TabTap | undefined {
	return tabId ? taps[tabId] : undefined;
}

/** Pauses or resumes a tab's file log. */
export async function toggleTabLogPause(tabId: string): Promise<void> {
	const tap = taps[tabId];
	if (!tap?.log) return;
	const next = await sessionLogPause(tap.stream, !tap.log.paused);
	tap.log = next ? { path: next.path, paused: next.paused } : null;
}

/** Stops a tab's file log and closes the file. */
export async function stopTabLog(tabId: string): Promise<void> {
	const tap = taps[tabId];
	if (!tap?.log) return;
	await sessionLogStop(tap.stream);
	tap.log = null;
	tap.status = { ...tap.status, logFile: null };
}

/** Pauses or resumes recording a tab (its file log goes on). */
export async function toggleTabHistoryPause(tabId: string): Promise<void> {
	const tap = taps[tabId];
	if (!tap?.status.recording) return;
	const next = await historyPause(tap.stream, !tap.status.paused);
	if (next) tap.status = next;
}

export interface HistoryPanelOpen {
	text?: string;
	/** Start filtered to this saved session. */
	sessionId?: string | null;
	sessionName?: string | null;
	/** Where "Paste into tab" pastes. */
	tabId?: string | null;
}

let panel = $state<HistoryPanelOpen | null>(null);

export function openHistoryPanel(opts: HistoryPanelOpen = {}): void {
	panel = { ...opts };
}

export function closeHistoryPanel(): void {
	panel = null;
}

export function historyPanel(): HistoryPanelOpen | null {
	return panel;
}

/** Hands the retention settings to the backend; at start and on change. */
export function pushHistoryRetention(): void {
	const s = getSettings();
	void historyConfigure({ outputDays: s.historyOutputDays, commandDays: s.historyCommandDays, maxMb: s.historyMaxMb }).catch(() => {});
}

/** Opens the search on a tab's session, pasting back into that tab. */
export function openTabHistory(tab: { id: string; sessionId?: string | null; sessionName?: string | null } | undefined, text?: string): void {
	openHistoryPanel({ text, sessionId: tab?.sessionId ?? null, sessionName: tab?.sessionName ?? null, tabId: tab?.id ?? null });
}

/** The quick command picker (Ctrl+Shift+R): a small list of earlier
 *  commands to paste into a tab, beside the full search panel. */
let picker = $state<HistoryPanelOpen | null>(null);

export function openCommandPicker(opts: HistoryPanelOpen = {}): void {
	picker = { ...opts };
}

export function closeCommandPicker(): void {
	picker = null;
}

export function commandPicker(): HistoryPanelOpen | null {
	return picker;
}

/** Opens the picker on a tab's session, pasting into that tab. */
export function openTabCommandPicker(tab: { id: string; sessionId?: string | null; sessionName?: string | null } | undefined, text?: string): void {
	openCommandPicker({ text, sessionId: tab?.sessionId ?? null, sessionName: tab?.sessionName ?? null, tabId: tab?.id ?? null });
}

/**
 * Types a command from history into a tab as one pasted line (bracketed
 * when the shell asks for it). With `run` Enter follows; without, the
 * line waits for the user. False when the tab has no terminal to type into.
 */
export function pasteHistoryCommand(tabId: string | null | undefined, command: string, run = false): boolean {
	const input = tabId ? getTerminalInput(tabId) : undefined;
	if (!input) return false;
	const line = command.replace(/[\r\n]+/g, ' ');
	const body = (input.bracketed() ? `\x1b[200~${line}\x1b[201~` : line) + (run ? '\r' : '');
	input.send(Array.from(new TextEncoder().encode(body)));
	return true;
}
