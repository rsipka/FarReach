<script lang="ts">
	/**
	 * The active tab's recording state (click pauses, right-click searches),
	 * its file log, and a button that opens the history search. Shown in the
	 * status bar and, when that steps aside, in the session monitor bar.
	 */
	import { getActiveTab } from '#lib/state/tabs.svelte.js';
	import { t } from '#lib/state/i18n.svelte.js';
	import { tabHistory, toggleTabHistoryPause, openTabHistory } from '#lib/state/history.svelte.js';

	let activeTab = $derived(getActiveTab());
	let tap = $derived(tabHistory(activeTab?.id));
	let recording = $derived(tap?.status.recording ?? null);
	let logging = $derived(!!tap?.status.logFile || !!tap?.sshLog);
	let terminal = $derived(!!activeTab && activeTab.type !== 'rdp' && activeTab.type !== 'vnc');
</script>

{#if recording && tap}
	<button
		type="button"
		class="rec"
		class:paused={tap.status.paused}
		title={tap.status.paused ? t('history.indicator_paused_hint') : t(recording === 'output' ? 'history.indicator_output_hint' : 'history.indicator_commands_hint')}
		onclick={() => activeTab && void toggleTabHistoryPause(activeTab.id)}
		oncontextmenu={(e) => { e.preventDefault(); openTabHistory(activeTab); }}
	>
		<span class="dot"></span>{tap.status.paused ? t('history.indicator_paused') : t('history.indicator')}
	</button>
{/if}
{#if logging}
	<span class="log" title={tap?.status.logFile ?? t('history.log_hint')}>{t('history.log_indicator')}</span>
{/if}
{#if terminal}
	<button type="button" class="search" title={`${t('history.search_menu')} (Ctrl+Shift+H)`} aria-label={t('history.search_menu')} onclick={() => openTabHistory(activeTab)}>
		<svg width="11" height="11" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" aria-hidden="true">
			<path d="M8 3.5V8l3 2" /><path d="M2.6 5.5A6 6 0 1 1 2 8" /><path d="M2 2.5v3h3" />
		</svg>
		<span class="label">{t('history.button')}</span>
	</button>
{/if}

<style>
	.rec,
	.log,
	.search {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		flex-shrink: 0;
		white-space: nowrap;
		font-size: inherit;
		color: inherit;
	}

	.rec,
	.search {
		background: none;
		border: none;
		padding: 0;
		cursor: pointer;
	}

	.search:hover,
	.rec:hover {
		color: var(--color-text-primary);
	}

	.rec .dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background-color: var(--color-danger);
	}

	.rec.paused .dot {
		background-color: var(--color-text-secondary);
	}

	@container (max-width: 640px) {
		.search .label {
			display: none;
		}
	}
</style>
