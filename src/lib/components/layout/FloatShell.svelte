<script lang="ts">
	/**
	 * A floating session window (#lib/state/windows): a title bar, its own tab
	 * bar and the terminal area, nothing else. Settings, the vault and the
	 * session list stay in the main window.
	 */
	import type { Snippet } from 'svelte';
	import { onMount, onDestroy } from 'svelte';
	import { getCurrentWindow } from '@tauri-apps/api/window';
	import TabBar from './TabBar.svelte';
	import Toast from '#lib/components/shared/Toast.svelte';
	import Modal from '#lib/components/shared/Modal.svelte';
	import Button from '#lib/components/shared/Button.svelte';
	import { getTabs, getActiveTab, getSplit } from '#lib/state/tabs.svelte.js';
	import { vaultState } from '#lib/state/vault.svelte.js';
	import { t } from '#lib/state/i18n.svelte.js';
	import { isMac } from '#lib/platform.js';
	import { APP_NAME } from '#lib/brand.js';
	import { moveAllToMain, closeAllHere, closeSelf, focusMain, noteContents } from '#lib/state/windows.svelte.js';

	let { children }: { children: Snippet } = $props();

	const macWindow = isMac();
	let tabs = $derived(getTabs());
	let activeTab = $derived(getActiveTab());
	let title = $derived(activeTab ? activeTab.sessionName || activeTab.title : APP_NAME);

	// Once it has shown a tab, a window left with none closes: its last
	// tab closed, or went to another window.
	let hadTabs = false;
	$effect(() => {
		const n = tabs.length;
		if (n > 0) hadTabs = true;
		else if (hadTabs) void closeSelf();
	});

	// What it shows and where, kept for the next start; and its title.
	$effect(() => {
		void tabs.map((x) => x.id + (x.sessionId ?? '')).join();
		void getSplit();
		void activeTab?.id;
		noteContents();
	});

	let closeOpen = $state(false);
	let unlistenClose: (() => void) | undefined;

	onMount(async () => {
		try {
			unlistenClose = await getCurrentWindow().onCloseRequested(async (event) => {
				event.preventDefault();
				if (getTabs().length === 0) await closeSelf();
				else closeOpen = true;
			});
		} catch {
			// Browser preview: no window to close.
		}
	});
	onDestroy(() => unlistenClose?.());

	async function moveBack(): Promise<void> {
		closeOpen = false;
		await moveAllToMain();
	}

	async function endAll(): Promise<void> {
		closeOpen = false;
		await closeAllHere();
		await closeSelf();
	}
</script>

<div class="float-shell">
	<header class="float-bar" class:mac={macWindow} data-tauri-drag-region>
		<img src="/app-icon.png" alt="" class="app-icon" draggable="false" />
		<span class="float-title" data-tauri-drag-region>{title}</span>
		<span class="float-count" data-tauri-drag-region>{t('windows.tab_count', { count: String(tabs.length) })}</span>
		<div class="float-actions">
			<button class="window-btn" onclick={() => void moveAllToMain()} title={t('windows.move_all_main')} aria-label={t('windows.move_all_main')}>
				<svg width="14" height="14" viewBox="0 0 24 24" fill="none" aria-hidden="true">
					<path d="M4 5h16v14H4zM4 9h16M14 15H8m0 0 2.5-2.5M8 15l2.5 2.5" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" />
				</svg>
			</button>
			{#if !macWindow}
				<button class="window-btn" onclick={() => getCurrentWindow().minimize()} aria-label={t('titlebar.minimize')}>
					<svg width="10" height="10" viewBox="0 0 10 10" fill="none"><path d="M1 5h8" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" /></svg>
				</button>
				<button class="window-btn" onclick={() => getCurrentWindow().toggleMaximize()} aria-label={t('titlebar.maximize')}>
					<svg width="10" height="10" viewBox="0 0 10 10" fill="none"><rect x="1" y="1" width="8" height="8" rx="1.5" stroke="currentColor" stroke-width="1.2" /></svg>
				</button>
				<button class="window-btn window-btn-close" onclick={() => getCurrentWindow().close()} aria-label={t('titlebar.close')}>
					<svg width="10" height="10" viewBox="0 0 10 10" fill="none"><path d="M1.5 1.5l7 7M8.5 1.5l-7 7" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" /></svg>
				</button>
			{/if}
		</div>
	</header>
	<TabBar />
	<main class="float-content">
		<!-- Inert while locked: no key reaches a terminal behind the cover. -->
		<div class="float-sessions" inert={vaultState.held}>
			{@render children()}
		</div>
		<!-- Locked in the main window: what is here is covered until it opens. -->
		{#if vaultState.held}
			<div class="float-locked" role="alert">
				<svg width="28" height="28" viewBox="0 0 24 24" fill="none" aria-hidden="true">
					<rect x="5" y="11" width="14" height="10" rx="2" stroke="currentColor" stroke-width="1.8" />
					<path d="M8 11V7a4 4 0 0 1 8 0v4" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" />
				</svg>
				<p class="float-locked-title">{t('windows.locked')}</p>
				<p class="float-locked-text">{t('windows.locked_text')}</p>
				<Button variant="primary" onclick={focusMain}>{t('windows.open_main')}</Button>
			</div>
		{/if}
	</main>
	<Toast />
</div>

<Modal open={closeOpen} onclose={() => (closeOpen = false)} title={t('windows.close_title')} maxWidth="440px">
	<p class="close-text">{t('windows.close_text', { count: String(tabs.length) })}</p>
	{#snippet actions()}
		<Button variant="secondary" onclick={() => (closeOpen = false)}>{t('common.cancel')}</Button>
		<Button variant="danger" onclick={() => void endAll()}>{t('windows.close_end')}</Button>
		<Button variant="primary" onclick={() => void moveBack()}>{t('windows.close_move')}</Button>
	{/snippet}
</Modal>

<style>
	.float-shell {
		display: grid;
		grid-template-rows: var(--titlebar-h, 38px) var(--tabbar-h, 36px) minmax(0, 1fr);
		grid-template-columns: minmax(0, 1fr);
		width: 100vw;
		height: 100vh;
		overflow: hidden;
		background-color: var(--color-bg-primary);
	}

	.float-bar {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 0 8px 0 12px;
		background-color: var(--color-bg-secondary);
		border-bottom: 1px solid var(--color-border);
		user-select: none;
		-webkit-app-region: drag;
		min-width: 0;
	}

	/* Room for the traffic lights, which sit over the bar's left end. */
	.float-bar.mac {
		padding-left: 78px;
	}

	.app-icon {
		width: 16px;
		height: 16px;
		pointer-events: none;
	}

	.float-title {
		font-size: 0.8125rem;
		font-weight: 600;
		color: var(--color-text-primary);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		min-width: 0;
	}

	.float-count {
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
		white-space: nowrap;
	}

	.float-actions {
		margin-left: auto;
		display: flex;
		align-items: center;
		gap: 2px;
		-webkit-app-region: no-drag;
	}

	.window-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 28px;
		height: 28px;
		border: none;
		border-radius: var(--radius-btn);
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		transition:
			background-color var(--duration-default) var(--ease-default),
			color var(--duration-default) var(--ease-default);
	}

	.window-btn:hover {
		background-color: var(--color-surface-hover);
		color: var(--color-text-primary);
	}

	.window-btn-close:hover {
		background-color: var(--color-danger);
		color: #fff;
	}

	.float-content {
		position: relative;
		min-height: 0;
		overflow: hidden;
	}

	.float-sessions {
		position: absolute;
		inset: 0;
	}

	.float-locked {
		position: absolute;
		inset: 0;
		z-index: 50;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 10px;
		padding: 24px;
		text-align: center;
		background: var(--color-bg-primary);
		color: var(--color-text-secondary);
	}

	.float-locked-title {
		margin: 0;
		font-size: var(--text-base, 0.9375rem);
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.float-locked-text {
		margin: 0 0 6px;
		max-width: 360px;
		font-size: var(--text-sm, 0.8125rem);
	}

	.close-text {
		margin: 0;
		font-size: var(--text-sm, 0.8125rem);
		color: var(--color-text-secondary);
		line-height: 1.5;
	}
</style>
