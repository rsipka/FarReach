/**
 * Duplicating a tab (#160): the same kind of session again, in a new tab
 * just after the original and in its group.
 *
 * An SSH tab opens another login with the tab's own connect settings, as
 * Reconnect does, so it asks nothing a reconnect would not; a Telnet tab
 * connects again the same way. Remote desktops
 * are not duplicated: a second RDP login as the same user usually logs the
 * first one off.
 */

import { sshConnect } from '#lib/ipc/ssh.js';
import { createTab, getTabs, placeAfter, type Tab } from '#lib/state/tabs.svelte.js';
import { newViewTag } from '#lib/terminal/tmux.js';

export function canDuplicate(tab: Tab | undefined): boolean {
	if (!tab) return false;
	return tab.type === 'local' || (tab.type === 'ssh' && !!tab.sshConnectParams) || (tab.type === 'telnet' && !!tab.telnetConnectParams);
}

export async function duplicateTab(id: string): Promise<void> {
	const from = getTabs().find((t) => t.id === id);
	if (!from || !canDuplicate(from)) return;

	if (from.type === 'local') {
		// A WSL tab is known by its connection id; a plain shell has none.
		const wsl = from.connectionId?.startsWith('wsl-');
		const tab = createTab(
			'local',
			// A plain shell takes the next free name ("Local 2"); a shell in a
			// container or a distribution keeps the name that says which.
			from.localCommand ? from.title : undefined,
			wsl ? `wsl-${crypto.randomUUID()}` : undefined,
			from.sessionName,
			from.detectedOs,
			from.localCommand ? { program: from.localCommand.program, args: [...from.localCommand.args] } : undefined,
			from.wslDistro
		);
		keepSession(tab, from);
		placeAfter(tab.id, from.id);
		return;
	}

	if (from.type === 'telnet') {
		// Its terminal connects by itself, as the first one did.
		const tab = createTab('telnet', from.title, undefined, from.sessionName, from.detectedOs);
		tab.telnetConnectParams = { ...from.telnetConnectParams!, options: { ...from.telnetConnectParams!.options } };
		keepSession(tab, from);
		placeAfter(tab.id, from.id);
		return;
	}

	// A tab in a tmux view gets a view of its own, on the same window.
	const params = { ...from.sshConnectParams!, id: crypto.randomUUID() };
	if (params.tmux?.view) params.tmux = { ...params.tmux, view: newViewTag(), window: from.tmuxWindow?.id };
	await sshConnect(params);
	const tab = createTab('ssh', from.title, params.id, from.sessionName, from.detectedOs);
	tab.sshConnectParams = params;
	keepSession(tab, from);
	placeAfter(tab.id, from.id);
}

/** A copy of a session's tab is that session's too, colours included. */
export function keepSession(tab: Tab, from: Tab): void {
	tab.sessionId = from.sessionId;
	tab.terminalTheme = from.terminalTheme;
	tab.cursorStyle = from.cursorStyle;
	tab.cursorBlink = from.cursorBlink;
	tab.monitoring = from.monitoring;
	tab.historyMode = from.historyMode;
}
