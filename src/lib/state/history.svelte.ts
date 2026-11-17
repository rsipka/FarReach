/**
 * Session history in the window: what each tab records (for the bottom-bar
 * indicator) and the search panel's state. The data itself is in the
 * backend's encrypted file; see src-tauri/src/history.
 */
import { historyConfigure, historyPause, type TapStatus } from '#lib/ipc/history.js';
import { getSettings } from '#lib/state/settings.svelte.js';

/** By tab id: its stream and what it records. */
const taps = $state<Record<string, { stream: string; status: TapStatus; sshLog: boolean }>>({});

export function setTabHistory(tabId: string, stream: string, status: TapStatus, sshLog = false): void {
	taps[tabId] = { stream, status, sshLog };
}

export function clearTabHistory(tabId: string): void {
	delete taps[tabId];
}

export function tabHistory(tabId: string | undefined): { stream: string; status: TapStatus; sshLog: boolean } | undefined {
	return tabId ? taps[tabId] : undefined;
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
