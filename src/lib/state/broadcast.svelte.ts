/**
 * Typing into several terminals at once, as Remote Desktop Manager's
 * multi-send: the terminals chosen, and sending to them. The rules are in
 * #lib/terminal/broadcast. Nothing is saved: a broadcast lasts while its
 * tabs are open.
 */
import * as rules from '#lib/terminal/broadcast.js';
import { getTabs, type Tab } from './tabs.svelte';
import { getTerminalInput } from './snippets.svelte';

let chosen = $state<string[]>([]);

/** Tabs with a terminal in them: local and SSH, not desktops. */
export function isTerminalTab(tab: Pick<Tab, 'type'> | undefined): boolean {
	return tab?.type === 'local' || tab?.type === 'ssh';
}

function openTerminals(): string[] {
	return getTabs().filter(isTerminalTab).map((t) => t.id);
}

/** The chosen terminals still open. */
export function getBroadcastTargets(): string[] {
	return rules.live(chosen, openTerminals());
}

export function isBroadcastTarget(tabId: string): boolean {
	return getBroadcastTargets().includes(tabId);
}

/** Whether typing in a chosen terminal reaches another one. */
export function isBroadcasting(): boolean {
	return rules.isOn(chosen, openTerminals());
}

export function toggleBroadcastTarget(tabId: string): void {
	if (!isTerminalTab(getTabs().find((t) => t.id === tabId))) return;
	chosen = rules.toggle(getBroadcastTargets(), tabId);
}

/** Every open terminal, or those of one tab group. */
export function broadcastToAll(groupId?: string): void {
	const ids = getTabs()
		.filter((t) => isTerminalTab(t) && (!groupId || t.groupId === groupId))
		.map((t) => t.id);
	chosen = rules.add(getBroadcastTargets(), ids);
}

export function stopBroadcast(): void {
	chosen = [];
}

/** Input typed into `source` (already sent there) goes to the other chosen
 *  terminals, if `source` is one of them. */
export function broadcastInput(source: string, data: number[]): void {
	for (const id of rules.recipients(chosen, source, openTerminals())) {
		getTerminalInput(id)?.send(data);
	}
}
