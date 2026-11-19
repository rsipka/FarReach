<script lang="ts">
	import { sessionAddress } from '#lib/utils/address.js';
	/**
	 * The bar over a terminal pane: what it is connected to, and the actions
	 * people reach for while working in it, one click away instead of in the
	 * tab's right-click menu. Each pane of a split has its own. "More" opens
	 * that same tab menu, so nothing here is a second copy of it.
	 */
	import type { Tab } from '#lib/state/tabs.svelte.js';
	import { splitWith, unsplit, getSplit, getTabs } from '#lib/state/tabs.svelte.js';
	import { SPLIT_MIN_WIDTH } from '#lib/terminal/split.js';
	import { isMobile } from '#lib/platform.js';
	import { addToast } from '#lib/state/toasts.svelte.js';
	import { isBroadcastTarget, toggleBroadcastTarget } from '#lib/state/broadcast.svelte.js';
	import { openHistoryPanel, tabHistory } from '#lib/state/history.svelte.js';
	import { finderFor, pendingFind } from '#lib/state/terminal-find.svelte.js';
	import { t } from '#lib/state/i18n.svelte.js';
	import { tick } from 'svelte';

	interface Props {
		tab: Tab;
		/** Lets a hover-only bar stay shown while its find field is open. */
		onfindopen?: (open: boolean) => void;
	}

	let { tab, onfindopen }: Props = $props();

	const ICONS = {
		find: 'M11 4.5a6.5 6.5 0 1 0 0 13 6.5 6.5 0 0 0 0-13zM20 20l-4.2-4.2',
		files: 'M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z',
		snippets: 'M8 6l-5 6 5 6M16 6l5 6-5 6M13.5 4l-3 16',
		history: 'M3 12a9 9 0 1 0 3-6.7L3 8M3 3v5h5M12 7v5l3 2',
		broadcast: 'M12 10a2 2 0 1 0 0 4 2 2 0 0 0 0-4zM8 8a5.6 5.6 0 0 0 0 8M16 8a5.6 5.6 0 0 1 0 8M5 5a10 10 0 0 0 0 14M19 5a10 10 0 0 1 0 14',
		split: 'M5 4h14a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2zM12 4v16',
		more: 'M5 12h.01M12 12h.01M19 12h.01',
		up: 'M6 15l6-6 6 6',
		down: 'M6 9l6 6 6-6',
		close: 'M6 6l12 12M18 6L6 18'
	};

	let remote = $derived(tab.type === 'ssh' || tab.type === 'telnet' || tab.type === 'serial');
	let params = $derived(tab.sshConnectParams ?? tab.telnetConnectParams);
	let address = $derived(
		tab.serialConnectParams ? sessionAddress({ kind: 'serial', host: tab.serialConnectParams.port, serial: tab.serialConnectParams.options }) : params ? `${'username' in params && params.username ? params.username + '@' : ''}${params.host}:${params.port}` : ''
	);
	let protocol = $derived(
		tab.type === 'ssh' ? 'SSH' : tab.type === 'telnet' ? 'Telnet' : tab.type === 'serial' ? t('session.protocol_serial') : tab.wslDistro ? 'WSL' : t('termbar.local')
	);
	let tmuxName = $derived(tab.type === 'ssh' ? (tab.sshConnectParams?.tmux?.name ?? null) : null);
	let tap = $derived(tabHistory(tab.id));
	let log = $derived(tap?.log ?? null);
	let linkState = $derived(tab.link === 'down' ? 'down' : tab.link === 'reconnecting' ? 'reconnecting' : remote ? 'up' : 'local');
	let stateLabel = $derived(
		linkState === 'down' ? t('tabs.disconnected') : linkState === 'reconnecting' ? t('terminal.reconnecting') : linkState === 'up' ? t('termbar.connected') : t('termbar.local')
	);
	let broadcasting = $derived(isBroadcastTarget(tab.id));
	let split = $derived(getSplit());
	let inSplit = $derived(!!split && (split.first === tab.id || split.second === tab.id));
	// A phone or tablet has no local shell to open beside a lone tab, and a
	// phone's screen is too narrow to show a split at all.
	const mobile = isMobile();
	let innerWidth = $state(typeof window === 'undefined' ? 1200 : window.innerWidth);
	let splitShown = $derived(!mobile || innerWidth >= SPLIT_MIN_WIDTH);
	let splitBlocked = $derived(mobile && !inSplit && getTabs().length < 2);
	let splitTip = $derived(inSplit ? t('split.unsplit') : splitBlocked ? t('split.need_second') : t('split.right'));

	// ---- Find ----------------------------------------------------------------
	let findOpen = $state(false);
	let query = $state('');
	let caseSensitive = $state(false);
	let notFound = $state(false);
	let findEl = $state<HTMLInputElement>();

	async function openFind(): Promise<void> {
		findOpen = true;
		onfindopen?.(true);
		await tick();
		findEl?.focus();
		findEl?.select();
	}

	function closeFind(): void {
		findOpen = false;
		notFound = false;
		onfindopen?.(false);
		finderFor(tab.id)?.clear();
	}

	function find(backwards = false): void {
		const f = finderFor(tab.id);
		if (!f || !query) {
			notFound = false;
			return;
		}
		notFound = !(backwards ? f.previous(query, caseSensitive) : f.next(query, caseSensitive));
	}

	let lastRequest = 0;
	$effect(() => {
		const r = pendingFind();
		if (r && r.tabId === tab.id && r.at !== lastRequest) {
			lastRequest = r.at;
			void openFind();
		}
	});

	function onFindKey(e: KeyboardEvent): void {
		if (e.key === 'Enter') {
			e.preventDefault();
			find(e.shiftKey);
		} else if (e.key === 'Escape') {
			e.preventDefault();
			closeFind();
		}
	}

	// ---- Other actions -------------------------------------------------------
	function showSection(section: 'explorer' | 'snippets'): void {
		window.dispatchEvent(new CustomEvent('reach:sidebar-section', { detail: section }));
	}

	function toggleSplit(): void {
		if (splitBlocked) addToast(t('split.need_second'), 'info');
		else if (inSplit) unsplit();
		else splitWith(tab.id, 'right');
	}

	/** The tab's own right-click menu, opened under the button. */
	function openMore(e: MouseEvent): void {
		const el = document.querySelector<HTMLElement>(`[data-tab-id="${CSS.escape(tab.id)}"]`);
		if (!el) return;
		const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
		el.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: r.right, clientY: r.bottom + 2 }));
	}
</script>

{#snippet icon(d: string, size = 15)}
	<svg width={size} height={size} viewBox="0 0 24 24" fill="none" aria-hidden="true">
		<path {d} stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
	</svg>
{/snippet}

<svelte:window bind:innerWidth />

<div class="term-bar" role="toolbar" aria-label={t('termbar.label')}>
	<span class="state {linkState}" role="img" aria-label={stateLabel} title={stateLabel}></span>
	<span class="name">{tab.sessionName || tab.title}</span>
	{#if address}<span class="address">{address}</span>{/if}
	{#if remote || tab.wslDistro}<span class="badge">{protocol}</span>{/if}
	{#if tmuxName}<span class="badge" title={tmuxName}>tmux</span>{/if}
	{#if log}<span class="rec" class:paused={log.paused} title={log.paused ? t('history.log_paused_hint') : t('termbar.logging')}>{t('termbar.log')}</span>{/if}

	<span class="grow"></span>

	{#if findOpen}
		<div class="find" class:missing={notFound}>
			<input
				bind:this={findEl}
				bind:value={query}
				type="text"
				placeholder={t('termbar.find_placeholder')}
				aria-label={t('termbar.find')}
				onkeydown={onFindKey}
				oninput={() => find()}
			/>
			<button class="ib small" class:on={caseSensitive} type="button" onclick={() => { caseSensitive = !caseSensitive; find(); }} title={t('termbar.match_case')} aria-label={t('termbar.match_case')} aria-pressed={caseSensitive}>Aa</button>
			<button class="ib small" type="button" onclick={() => find(true)} title={t('termbar.previous')} aria-label={t('termbar.previous')}>{@render icon(ICONS.up, 14)}</button>
			<button class="ib small" type="button" onclick={() => find()} title={t('termbar.next')} aria-label={t('termbar.next')}>{@render icon(ICONS.down, 14)}</button>
			<button class="ib small" type="button" onclick={closeFind} title={t('common.close')} aria-label={t('common.close')}>{@render icon(ICONS.close, 13)}</button>
		</div>
	{:else}
		<button class="ib" type="button" onclick={() => void openFind()} title={`${t('termbar.find')} (Ctrl+Shift+F)`} aria-label={t('termbar.find')}>{@render icon(ICONS.find)}</button>
	{/if}
	{#if tab.type === 'ssh'}
		<button class="ib" type="button" onclick={() => showSection('explorer')} title={t('termbar.files')} aria-label={t('termbar.files')}>{@render icon(ICONS.files)}</button>
	{/if}
	<button class="ib" type="button" onclick={() => showSection('snippets')} title={t('sidebar.snippets')} aria-label={t('sidebar.snippets')}>{@render icon(ICONS.snippets)}</button>
	{#if remote}
		<button class="ib" type="button" onclick={() => openHistoryPanel({ sessionId: tab.sessionId, sessionName: tab.sessionName, tabId: tab.id })} title={`${t('termbar.history')} (Ctrl+Shift+H)`} aria-label={t('termbar.history')}>{@render icon(ICONS.history)}</button>
	{/if}
	<span class="sep"></span>
	<button class="ib" class:on={broadcasting} type="button" onclick={() => toggleBroadcastTarget(tab.id)} title={t('broadcast.tab_toggle')} aria-label={t('broadcast.tab_toggle')} aria-pressed={broadcasting}>{@render icon(ICONS.broadcast)}</button>
	{#if splitShown}
		<button class="ib" class:on={inSplit} type="button" onclick={toggleSplit} class:blocked={splitBlocked} aria-disabled={splitBlocked} title={splitTip} aria-label={splitTip} aria-pressed={inSplit}>{@render icon(ICONS.split)}</button>
	{/if}
	<span class="sep"></span>
	<button class="ib" type="button" onclick={openMore} title={t('termbar.more')} aria-label={t('termbar.more')} aria-haspopup="menu">{@render icon(ICONS.more, 17)}</button>
</div>

<style>
	.term-bar {
		display: flex;
		align-items: center;
		gap: 6px;
		height: 34px;
		flex: 0 0 34px;
		padding: 0 6px 0 12px;
		background: var(--color-bg-secondary);
		border-bottom: 1px solid var(--color-border);
		font-size: 0.78125rem;
		color: var(--color-text-secondary);
		min-width: 0;
		overflow: hidden;
	}

	.state {
		flex: 0 0 8px;
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: var(--color-text-tertiary);
	}

	.state.up {
		background: var(--color-success);
		box-shadow: 0 0 0 3px color-mix(in srgb, var(--color-success) 22%, transparent);
	}

	.state.reconnecting {
		background: var(--color-warning);
	}

	.state.down {
		background: var(--color-danger);
	}

	.name {
		font-weight: 600;
		color: var(--color-text-primary);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		min-width: 0;
		flex: 0 1 auto;
	}

	.address {
		font-family: var(--font-mono);
		font-size: 0.75rem;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		min-width: 0;
		flex: 0 2 auto;
	}

	.badge {
		flex-shrink: 0;
		padding: 0 6px;
		border: 1px solid var(--color-border-strong);
		border-radius: 4px;
		font-size: var(--text-2xs);
		font-weight: 600;
		line-height: 16px;
	}

	/* Writing to a log file is never silent. */
	.rec {
		flex-shrink: 0;
		display: inline-flex;
		align-items: center;
		gap: 5px;
		padding: 0 8px;
		border-radius: 10px;
		font-size: var(--text-2xs);
		font-weight: 700;
		line-height: 18px;
		color: var(--color-danger);
		background: color-mix(in srgb, var(--color-danger) 12%, transparent);
	}

	.rec.paused {
		color: var(--color-text-secondary);
		background: color-mix(in srgb, var(--color-text-secondary) 12%, transparent);
	}

	.rec::before {
		content: '';
		width: 7px;
		height: 7px;
		border-radius: 50%;
		background: currentColor;
	}

	.grow {
		flex: 1;
		min-width: 4px;
	}

	.sep {
		flex-shrink: 0;
		width: 1px;
		height: 18px;
		background: var(--color-border);
	}

	.ib {
		flex-shrink: 0;
		display: inline-grid;
		place-items: center;
		width: 28px;
		height: 26px;
		padding: 0;
		border: 1px solid transparent;
		border-radius: 6px;
		background: none;
		color: var(--color-text-secondary);
		font: inherit;
		font-size: 0.6875rem;
		font-weight: 600;
		cursor: pointer;
	}

	.ib:hover {
		background: var(--color-surface-hover);
		border-color: var(--color-border);
		color: var(--color-text-primary);
	}

	.ib.blocked {
		opacity: 0.45;
	}

	.ib.on {
		background: var(--color-accent-subtle);
		color: var(--color-accent);
	}

	.ib.small {
		width: 24px;
		height: 22px;
	}

	.find {
		display: flex;
		align-items: center;
		gap: 2px;
		padding: 0 2px 0 8px;
		height: 26px;
		border: 1px solid var(--color-border-strong);
		border-radius: 6px;
		background: var(--color-bg-primary);
	}

	.find.missing {
		border-color: var(--color-danger);
	}

	.find input {
		width: 160px;
		min-width: 60px;
		border: none;
		outline: none;
		background: none;
		color: var(--color-text-primary);
		font: inherit;
	}

	/* A phone: the name and the actions; the address and badges give way. */
	@media (max-width: 640px) {
		.address,
		.badge {
			display: none;
		}

		.find input {
			width: 90px;
		}
	}

	@media (pointer: coarse) {
		.term-bar {
			height: 40px;
			flex-basis: 40px;
		}

		.ib {
			width: 34px;
			height: 34px;
		}
	}
</style>
