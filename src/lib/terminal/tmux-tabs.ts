/**
 * tmux windows as tabs. Every SSH tab in tmux joins its tmux session as a
 * view of its own (see src-tauri/src/ssh/tmux.rs), so each tab can show a
 * different window of the same session: a new tab can be a new window, a
 * window can open in a tab of its own, and connecting can open them all.
 */

import { sshConnect, sshTmuxWindows, type TmuxStart, type TmuxWindow } from '#lib/ipc/ssh.js';
import { activateTab, createTab, getActiveTab, placeAfter, type Tab } from '#lib/state/tabs.svelte.js';
import { getSettings } from '#lib/state/settings.svelte.js';
import { isMobile } from '#lib/platform.js';
import { addToast } from '#lib/state/toasts.svelte.js';
import { t } from '#lib/state/i18n.svelte.js';
import { keepSession } from './duplicate-tab.js';
import { newViewTag, tmuxWindowFromTitle } from './tmux.js';

/** An SSH tab in a tmux view of its own, connected. */
export function isTmuxViewTab(tab: Tab | undefined): tab is Tab {
	return !!tab && tab.type === 'ssh' && !!tab.connectionId && !!tab.sshConnectParams?.tmux?.view && tab.link !== 'down';
}

/** A new tab, beside `from`, showing `window` of its tmux session: 'new'
 *  for a new window, or a window id. */
export async function openTmuxWindowTab(from: Tab, window: string, after: string = from.id, activate = true): Promise<Tab | null> {
	const base = from.sshConnectParams;
	if (!base?.tmux) return null;
	const params = { ...base, id: crypto.randomUUID(), tmux: { ...base.tmux, ask: false, view: newViewTag(), window } };
	await sshConnect(params);
	const previous = getActiveTab();
	const tab = createTab('ssh', from.title, params.id, from.sessionName, from.detectedOs);
	tab.sshConnectParams = params;
	keepSession(tab, from);
	placeAfter(tab.id, after);
	if (!activate && previous) activateTab(previous.id);
	return tab;
}

/** The windows of `tab`'s session; while its view is still being made
 *  (just connected), asked again for a moment. */
async function windowsOf(tab: Tab): Promise<TmuxWindow[]> {
	for (let attempt = 0; attempt < 10; attempt++) {
		const list = tab.connectionId && tab.sshConnectParams?.tmux ? await sshTmuxWindows(tab.connectionId, tab.sshConnectParams.tmux).catch(() => []) : [];
		if (list.length > 0) return list;
		await new Promise((r) => setTimeout(r, 400));
	}
	return [];
}

/** Just connected: open the session's other windows in tabs of their own,
 *  if Settings says so. */
export async function openOtherWindows(tab: Tab): Promise<void> {
	if (getSettings().tmuxConnectWindows !== 'all' || !isTmuxViewTab(tab)) return;
	const list = await windowsOf(tab);
	let after = tab.id;
	for (const w of list) {
		if (w.active) continue;
		try {
			const opened = await openTmuxWindowTab(tab, w.id, after, false);
			if (opened) after = opened.id;
		} catch (err) {
			addToast(t('tmux.window_failed', { error: String(err) }), 'error');
			return;
		}
	}
}

/** Ctrl+T and the tab bar's +: on a tmux tab, a new tmux window in a new
 *  tab (unless Settings says otherwise); else a local terminal, except on a
 *  phone or tablet, which has no local shell. */
export function newTab(): void {
	const active = getActiveTab();
	if (getSettings().tmuxNewTabWindow && isTmuxViewTab(active)) {
		openTmuxWindowTab(active, 'new').catch((err) => addToast(t('tmux.window_failed', { error: String(err) }), 'error'));
		return;
	}
	if (isMobile()) return;
	createTab('local');
}

/** The terminal title a tmux view sets: the tab learns its window, and a
 *  reconnect goes back to that window. False: not a tmux view's title. */
export function takeTmuxTitle(tab: Tab, title: string): boolean {
	const tmux = tab.sshConnectParams?.tmux;
	if (!tmux?.view) return false;
	const w = tmuxWindowFromTitle(title);
	if (!w) return false;
	if (tab.tmuxWindow?.id !== w.id || tab.tmuxWindow.name !== w.name) tab.tmuxWindow = w;
	if (tmux.window !== w.id) tab.sshConnectParams = { ...tab.sshConnectParams!, tmux: { ...tmux, window: w.id } };
	return true;
}

/** An SSH tab's tmux start with a view of its own (see TmuxStart.view), and
 *  the status line as Settings says. */
export function tmuxView(start: TmuxStart | null): TmuxStart | null {
	return start ? { ...start, view: newViewTag(), hideStatus: getSettings().tmuxHideStatus || undefined } : null;
}
