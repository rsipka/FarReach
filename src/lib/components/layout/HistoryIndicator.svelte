<script lang="ts">
	/**
	 * The active tab's recording state (click pauses, right-click searches),
	 * its file log (click for pause, resume and stop), and a button that
	 * opens the history search. Shown in the status bar and, when that steps
	 * aside, in the session monitor bar, at the size of the bar's other items.
	 */
	import DbMenu, { type MenuItem } from '#lib/components/db/DbMenu.svelte';
	import { writeText } from '@tauri-apps/plugin-clipboard-manager';
	import { getActiveTab } from '#lib/state/tabs.svelte.js';
	import { t } from '#lib/state/i18n.svelte.js';
	import { tabHistory, toggleTabHistoryPause, openTabHistory, toggleTabLogPause, stopTabLog } from '#lib/state/history.svelte.js';

	const ICONS = {
		pause: 'M9 6v12M15 6v12',
		resume: 'M8 5.5v13l10.5-6.5z',
		stop: 'M7 7h10v10H7z',
		copy: 'M9 9h10v10H9zM5 15V5h10'
	};

	let activeTab = $derived(getActiveTab());
	let tap = $derived(tabHistory(activeTab?.id));
	let recording = $derived(tap?.status.recording ?? null);
	let log = $derived(tap?.log ?? null);
	let terminal = $derived(!!activeTab && activeTab.type !== 'rdp' && activeTab.type !== 'vnc');

	let menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);

	function openLogMenu(e: MouseEvent): void {
		const tabId = activeTab?.id;
		if (!tabId || !log) return;
		const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
		const path = log.path;
		const items: MenuItem[] = [
			log.paused
				? { label: t('history.log_resume'), icon: ICONS.resume, action: () => void toggleTabLogPause(tabId) }
				: { label: t('history.log_pause'), icon: ICONS.pause, action: () => void toggleTabLogPause(tabId) },
			{ label: t('history.log_stop'), icon: ICONS.stop, danger: true, action: () => void stopTabLog(tabId) }
		];
		if (path) {
			items.push({ separator: true, label: '' }, { label: t('history.log_copy_path'), icon: ICONS.copy, action: () => void writeText(path).catch(() => {}) });
		}
		menu = { x: rect.left, y: rect.top - 4, items };
	}
</script>

{#if recording && tap}
	<button
		type="button"
		class="item rec"
		class:paused={tap.status.paused}
		title={tap.status.paused ? t('history.indicator_paused_hint') : t(recording === 'output' ? 'history.indicator_output_hint' : 'history.indicator_commands_hint')}
		onclick={() => activeTab && void toggleTabHistoryPause(activeTab.id)}
		oncontextmenu={(e) => { e.preventDefault(); openTabHistory(activeTab); }}
	>
		<span class="dot"></span>{tap.status.paused ? t('history.indicator_paused') : t('history.indicator')}
	</button>
{/if}
{#if log}
	<button
		type="button"
		class="item log"
		class:paused={log.paused}
		class:open={!!menu}
		title={`${log.paused ? t('history.log_paused_hint') : (log.path ?? t('history.log_hint'))}\n${t('history.log_click_hint')}`}
		aria-haspopup="menu"
		onclick={openLogMenu}
	>
		<svg width="10" height="10" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
			<path d="M4 1.8h5.5L12.5 5v9.2H4z" /><path d="M6.5 8h3.5M6.5 10.8h3.5" />
		</svg>
		<span class="label">{log.paused ? t('history.log_paused') : t('history.log_indicator')}</span>
	</button>
{/if}
{#if terminal}
	<button type="button" class="item search" title={`${t('history.search_menu')} (Ctrl+Shift+H)`} aria-label={t('history.search_menu')} onclick={() => openTabHistory(activeTab)}>
		<svg width="10" height="10" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" aria-hidden="true">
			<path d="M8 3.5V8l3 2" /><path d="M2.6 5.5A6 6 0 1 1 2 8" /><path d="M2 2.5v3h3" />
		</svg>
		<span class="label">{t('history.button')}</span>
	</button>
{/if}
{#if menu}
	<DbMenu x={menu.x} y={menu.y} items={menu.items} above onclose={() => (menu = null)} />
{/if}

<style>
	/* The same size and colour as the bar's other items (CPU, RAM, the user
	   count), whichever bar it sits in and whatever the theme. */
	.item {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		flex-shrink: 0;
		height: 20px;
		padding: 0 4px;
		margin: 0 -4px;
		border: none;
		border-radius: var(--radius-sm, 4px);
		background: none;
		font-family: var(--font-sans);
		font-size: 11px;
		font-weight: 400;
		line-height: 1;
		white-space: nowrap;
		color: var(--color-text-secondary);
		cursor: pointer;
	}

	.item:hover,
	.item.open {
		color: var(--color-text-primary);
		background: var(--color-surface-hover);
	}

	.item:focus-visible {
		outline: 1px solid var(--color-accent);
		outline-offset: 0;
	}

	.item svg {
		flex-shrink: 0;
	}

	.dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background-color: var(--color-danger);
	}

	.rec.paused .dot {
		background-color: var(--color-text-tertiary, var(--color-text-secondary));
	}

	/* Writing to a file is never silent: the icon carries the danger colour,
	   as the terminal bar's LOG badge does. */
	.log svg {
		color: var(--color-danger);
	}

	.log.paused svg {
		color: var(--color-text-tertiary, var(--color-text-secondary));
	}

	@container (max-width: 640px) {
		.search .label,
		.log .label {
			display: none;
		}
	}
</style>
