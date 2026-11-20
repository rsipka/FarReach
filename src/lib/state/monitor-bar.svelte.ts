/**
 * Whether the session monitor bar is at the bottom of the window. The
 * terminal page draws it, and while it is there the status bar steps aside
 * and the monitor bar carries what the status bar said (the connection's
 * state and the time): two bars said the same thing in 56px.
 */
import { getActiveTab, getTabs } from '#lib/state/tabs.svelte.js';
import { getActivePage } from '#lib/state/navigation.svelte.js';
import * as share from '#lib/state/share.svelte.js';
import { getSettings } from '#lib/state/settings.svelte.js';
import type { Tab } from '#lib/state/tabs.svelte.js';

/** Whether a tab's server is monitored: its session's own choice, or the
 *  default from Settings. A Windows or MikroTik RouterOS host is off unless
 *  its session turns it on: the stats come from /proc, which neither has. */
export function monitoringWanted(tab: Tab | undefined): boolean {
	if (!tab) return false;
	if (tab.monitoring != null) return tab.monitoring;
	if (tab.type === 'ssh' && (tab.detectedOs === 'windows' || tab.detectedOs === 'mikrotik')) return false;
	return getSettings().serverMonitoring;
}

/** Server stats come from an SSH or WSL session; the bar also holds the
 *  sharing controls, so it stays for a shared local shell. Never under a
 *  remote desktop. */
export function monitorBarShown(): boolean {
	if (getActivePage() !== 'terminal' || getTabs().length === 0) return false;
	const tab = getActiveTab();
	if (!tab) return false;
	return tab.type === 'ssh' || !!tab.wslDistro || (share.isEnabled() && tab.type !== 'rdp' && tab.type !== 'vnc' && tab.type !== 'spice');
}
