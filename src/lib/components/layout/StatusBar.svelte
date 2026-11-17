<script lang="ts">
	import { getActiveTab } from '#lib/state/tabs.svelte.js';
	import { t } from '#lib/state/i18n.svelte.js';
	import Clock from './Clock.svelte';
	import { tabHistory, toggleTabHistoryPause, openHistoryPanel } from '#lib/state/history.svelte.js';

	// What the active tab records: the session history (pausable) and the
	// file log, shown so that recording is never silent.
	let tap = $derived(tabHistory(getActiveTab()?.id));
	let recording = $derived(tap?.status.recording ?? null);
	let logging = $derived(!!tap?.status.logFile || !!tap?.sshLog);

	let activeTab = $derived(getActiveTab());

	// The tab's own link state, as its mark in the tab bar shows it: an SSH
	// tab whose connection dropped is not "Connected to" anything.
	let link = $derived(activeTab?.type === 'ssh' ? activeTab.link : undefined);

	let connectionStatus = $derived.by(() => {
		if (!activeTab) return t('statusbar.no_session');
		if (link === 'down') return t('tabs.disconnected');
		if (link === 'reconnecting') return t('terminal.reconnecting');
		if (activeTab.type === 'ssh' && activeTab.connectionId) {
			return t('statusbar.connected_to', { title: activeTab.title });
		}
		return t('statusbar.local');
	});
</script>

<footer class="statusbar">
	<div class="statusbar-left">
		<div
			class="status-indicator"
			class:connected={activeTab?.type === 'ssh' && !link}
			class:down={link === 'down'}
			class:reconnecting={link === 'reconnecting'}
		></div>
		<span class="status-text">{connectionStatus}</span>
	</div>

	<div class="statusbar-center"></div>

	<div class="statusbar-right">
		{#if recording && tap}
			<button
				type="button"
				class="rec"
				class:paused={tap.status.paused}
				title={tap.status.paused ? t('history.indicator_paused_hint') : t(recording === 'output' ? 'history.indicator_output_hint' : 'history.indicator_commands_hint')}
				onclick={() => activeTab && void toggleTabHistoryPause(activeTab.id)}
				oncontextmenu={(e) => { e.preventDefault(); openHistoryPanel({ sessionId: activeTab?.sessionId, sessionName: activeTab?.sessionName, tabId: activeTab?.id }); }}
			>
				<span class="dot"></span>{tap.status.paused ? t('history.indicator_paused') : t('history.indicator')}
			</button>
		{/if}
		{#if logging}
			<span class="log" title={tap?.status.logFile ?? t('history.log_hint')}>{t('history.log_indicator')}</span>
		{/if}
		<Clock />
	</div>
</footer>

<style>
	.statusbar {
		display: flex;
		align-items: center;
		justify-content: space-between;
		/* The shell's row: shorter on a short screen (AppShell). */
		height: var(--statusbar-h, 24px);
		min-height: var(--statusbar-h, 24px);
		padding: 0 12px;
		background-color: var(--color-bg-secondary);
		border-top: 1px solid var(--color-border);
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
		user-select: none;
	}

	/* One line, whatever the width: wrapped, the status grew the bar past
	   its 24px and pushed the window's bottom off a phone's screen. */
	.statusbar-left {
		display: flex;
		align-items: center;
		gap: 6px;
		flex: 1;
		min-width: 0;
	}

	.status-text {
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.statusbar-center {
		flex-shrink: 0;
		text-align: center;
	}

	.statusbar-right {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: flex-end;
		gap: 10px;
	}

	.rec,
	.log {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		white-space: nowrap;
		font-size: inherit;
		color: inherit;
	}

	.rec {
		background: none;
		border: none;
		padding: 0;
		cursor: pointer;
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

	.status-indicator {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background-color: var(--color-text-secondary);
		transition: background-color var(--duration-default) var(--ease-default);
	}

	.status-indicator.connected {
		background-color: var(--color-success);
	}

	.status-indicator.down {
		background-color: var(--color-danger);
	}

	.status-indicator.reconnecting {
		background-color: var(--color-warning);
	}
</style>
