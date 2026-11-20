<script lang="ts">
	/**
	 * The quick way back to an earlier command: a small list of the commands
	 * in the session history, newest first and each only once, filtered as
	 * you type. Enter pastes the chosen one into the tab it was opened from
	 * (without Enter, so it can be edited), Ctrl+Enter pastes and runs it.
	 * Opened with Ctrl+Shift+R (Settings > Keyboard shortcuts), from the
	 * terminal's and the tab's right-click menu, or the command palette.
	 * The full search panel (Ctrl+Shift+H) is still there for output and
	 * whole runs.
	 */
	import { tick } from 'svelte';
	import { t } from '#lib/state/i18n.svelte.js';
	import { closeCommandPicker, commandPicker, openHistoryPanel, pasteHistoryCommand } from '#lib/state/history.svelte.js';
	import { requestTerminalFocus } from '#lib/state/palette.svelte.js';
	import { matchesAction } from '#lib/state/shortcuts.svelte.js';
	import { historySearch, type HistoryHit } from '#lib/ipc/history.js';
	import { isMac } from '#lib/platform.js';
	import { writeText as clipboardWriteText } from '@tauri-apps/plugin-clipboard-manager';

	interface Row {
		hit: HistoryHit;
		/** How many times it was run (in what was fetched). */
		count: number;
	}

	let opened = $derived(commandPicker());
	let open = $derived(opened !== null);
	let sessionId = $derived(opened?.sessionId ?? null);
	let text = $state('');
	let scope = $state<'session' | 'all'>('all');
	let rows = $state<Row[]>([]);
	let selected = $state(0);
	let error = $state('');
	let loaded = $state(false);
	let input = $state<HTMLInputElement>();
	let list = $state<HTMLDivElement>();

	const mac = isMac();
	const keys = {
		run: mac ? '⌘↩' : 'Ctrl+Enter',
		copy: mac ? '⌘C' : 'Ctrl+C'
	};

	// Each opening starts from what it was opened with: this session's
	// commands when it came from a saved session's tab.
	$effect(() => {
		const o = commandPicker();
		if (!o) return;
		text = o.text ?? '';
		scope = o.sessionId ? 'session' : 'all';
		selected = 0;
		loaded = false;
		error = '';
		void tick().then(() => input?.focus());
	});

	let seq = 0;
	async function search(): Promise<void> {
		const mine = ++seq;
		try {
			const hits = await historySearch({
				text,
				sessionId: scope === 'session' ? sessionId : null,
				kind: 'commands',
				limit: 500
			});
			if (mine !== seq) return;
			rows = unique(hits);
			error = '';
		} catch (e) {
			if (mine !== seq) return;
			rows = [];
			error = String(e).includes('locked') ? t('history.locked') : String(e);
		} finally {
			if (mine === seq) {
				loaded = true;
				selected = Math.min(selected, Math.max(rows.length - 1, 0));
			}
		}
	}

	/** Each command once, where it was last run. */
	function unique(hits: HistoryHit[]): Row[] {
		const seen = new Map<string, Row>();
		const out: Row[] = [];
		for (const hit of hits) {
			const key = hit.text.trim();
			if (!key) continue;
			const row = seen.get(key);
			if (row) row.count++;
			else {
				const fresh = { hit, count: 1 };
				seen.set(key, fresh);
				out.push(fresh);
			}
		}
		return out;
	}

	// As the user types, a moment after the last key.
	$effect(() => {
		void text;
		void scope;
		if (!open) return;
		const timer = setTimeout(() => void search(), 80);
		return () => clearTimeout(timer);
	});

	$effect(() => {
		void text;
		void scope;
		selected = 0;
	});

	function close(): void {
		closeCommandPicker();
		requestTerminalFocus();
	}

	function paste(row: Row | undefined, run = false): void {
		if (!row) return;
		pasteHistoryCommand(opened?.tabId, row.hit.text, run);
		close();
	}

	async function copy(row: Row | undefined): Promise<void> {
		if (!row) return;
		await clipboardWriteText(row.hit.text).catch(() => {});
		close();
	}

	function fullHistory(): void {
		const o = opened;
		closeCommandPicker();
		openHistoryPanel({ text, sessionId: o?.sessionId ?? null, sessionName: o?.sessionName ?? null, tabId: o?.tabId ?? null });
	}

	async function move(by: number): Promise<void> {
		if (!rows.length) return;
		selected = (selected + by + rows.length) % rows.length;
		await tick();
		list?.querySelector('.row.selected')?.scrollIntoView({ block: 'nearest' });
	}

	function onkeydown(e: KeyboardEvent): void {
		const mod = mac ? e.metaKey : e.ctrlKey;
		if (e.key === 'Escape') {
			e.preventDefault();
			close();
		} else if (e.key === 'Enter') {
			e.preventDefault();
			paste(rows[selected], mod);
		} else if (e.key === 'ArrowDown' || (e.ctrlKey && !e.shiftKey && e.code === 'KeyN') || matchesAction(e, 'terminal.commandPicker')) {
			// The picker's own key again walks on down, as Ctrl+R does in a shell.
			e.preventDefault();
			void move(1);
		} else if (e.key === 'ArrowUp' || (e.ctrlKey && !e.shiftKey && e.code === 'KeyP')) {
			e.preventDefault();
			void move(-1);
		} else if (e.key === 'PageDown' || e.key === 'PageUp') {
			e.preventDefault();
			void move(e.key === 'PageDown' ? 10 : -10);
		} else if (e.key === 'Tab') {
			e.preventDefault();
			if (sessionId) scope = scope === 'session' ? 'all' : 'session';
		} else if (mod && !e.shiftKey && !e.altKey && e.code === 'KeyC' && input && input.selectionStart === input.selectionEnd) {
			e.preventDefault();
			void copy(rows[selected]);
		}
	}

	/** 5m, 3h, 2d: enough to tell recent from old. */
	function ago(ms: number): string {
		const s = Math.max(0, (Date.now() - ms) / 1000);
		if (s < 60) return t('history.ago_now');
		if (s < 3600) return t('history.ago_minutes', { n: String(Math.floor(s / 60)) });
		if (s < 86400) return t('history.ago_hours', { n: String(Math.floor(s / 3600)) });
		return t('history.ago_days', { n: String(Math.floor(s / 86400)) });
	}

	function where(h: HistoryHit): string {
		return h.name && h.name !== h.host ? h.name : h.host;
	}
</script>

{#if open}
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<div class="scrim" onclick={close}></div>
	<div class="picker" role="dialog" aria-modal="true" aria-label={t('history.picker_title')}>
		<div class="search">
			<span class="prompt" aria-hidden="true">$</span>
			<input
				bind:this={input}
				bind:value={text}
				{onkeydown}
				placeholder={t('history.picker_placeholder')}
				aria-label={t('history.picker_placeholder')}
				aria-controls="command-picker-list"
				aria-activedescendant={rows[selected] ? `command-picker-${selected}` : undefined}
				autocomplete="off"
				spellcheck="false"
			/>
			{#if sessionId}
				<button
					type="button"
					class="scope"
					title={t('history.picker_scope_hint')}
					onclick={() => {
						scope = scope === 'session' ? 'all' : 'session';
						input?.focus();
					}}
				>
					{scope === 'session' ? opened?.sessionName || t('history.this_session') : t('history.all_sessions')}
				</button>
			{/if}
		</div>
		<div class="list" id="command-picker-list" role="listbox" bind:this={list}>
			{#each rows as row, i (row.hit.id)}
				<!-- svelte-ignore a11y_click_events_have_key_events -->
				<div
					id="command-picker-{i}"
					class="row"
					class:selected={i === selected}
					role="option"
					tabindex="-1"
					aria-selected={i === selected}
					title={row.hit.text}
					onclick={(e) => paste(row, mac ? e.metaKey : e.ctrlKey)}
					onmousemove={() => (selected = i)}
				>
					<code class="cmd">{row.hit.text.replace(/[\r\n]+/g, ' ')}</code>
					{#if row.hit.exit !== null && row.hit.exit !== 0}<span class="exit" title={`exit ${row.hit.exit}`}>✕</span>{/if}
					{#if row.count > 1}<span class="count">×{row.count}</span>{/if}
					{#if scope === 'all' || !sessionId}<span class="where">{where(row.hit)}</span>{/if}
					<span class="ago" title={new Date(row.hit.at).toLocaleString()}>{ago(row.hit.at)}</span>
				</div>
			{:else}
				{#if error}
					<div class="empty error">{error}</div>
				{:else if loaded}
					<div class="empty">
						{text.trim() ? t('history.no_results') : t('history.empty')}
						{#if scope === 'session' && sessionId}
							<br /><button type="button" class="link" onclick={() => (scope = 'all')}>{t('history.picker_search_all')}</button>
						{/if}
					</div>
				{/if}
			{/each}
		</div>
		<div class="foot">
			<span>{t('history.picker_help', keys)}</span>
			<button type="button" class="link" onclick={fullHistory}>{t('history.picker_full')}</button>
		</div>
	</div>
{/if}

<style>
	.scrim {
		position: fixed;
		inset: 0;
		z-index: 900;
		background: rgba(0, 0, 0, 0.25);
	}

	.picker {
		position: fixed;
		z-index: 901;
		top: calc(var(--safe-top) + min(12dvh, 96px));
		left: 50%;
		transform: translateX(-50%);
		width: min(720px, calc(100vw - var(--safe-left) - var(--safe-right) - 24px));
		max-height: calc(100dvh - var(--safe-top) - var(--safe-bottom) - min(12dvh, 96px) - 16px);
		display: flex;
		flex-direction: column;
		background: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-card);
		box-shadow: var(--shadow-elevated);
		overflow: hidden;
		animation: drop var(--duration-default) var(--ease-default);
	}

	@keyframes drop {
		from {
			opacity: 0;
			transform: translate(-50%, -6px);
		}
	}

	.search {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-2) var(--space-3);
		border-bottom: 1px solid var(--color-border);
	}

	.prompt {
		font-family: var(--font-mono);
		color: var(--color-accent);
	}

	.search input {
		flex: 1;
		min-width: 0;
		border: none;
		outline: none;
		background: transparent;
		color: var(--color-text-primary);
		font-family: var(--font-mono);
		font-size: var(--text-sm);
	}

	.search input::placeholder {
		color: var(--color-text-tertiary);
		font-family: var(--font-sans);
	}

	.scope {
		flex-shrink: 0;
		max-width: 40%;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		padding: 2px 8px;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-sm);
		background: transparent;
		color: var(--color-text-secondary);
		font-size: var(--text-2xs);
		cursor: pointer;
	}

	.scope:hover {
		color: var(--color-text-primary);
		background: var(--color-surface-hover);
	}

	.list {
		overflow-y: auto;
		padding: var(--space-1);
		max-height: 420px;
	}

	.row {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-height: 26px;
		padding: 0 var(--space-2);
		border-radius: var(--radius-btn);
		cursor: pointer;
		font-size: var(--text-2xs);
		color: var(--color-text-tertiary);
	}

	.row.selected {
		background: var(--color-surface-hover);
	}

	.cmd {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-family: var(--font-mono);
		font-size: var(--text-xs);
		color: var(--color-text-primary);
	}

	.exit {
		color: var(--color-danger);
	}

	.where {
		max-width: 30%;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.where,
	.count,
	.ago {
		flex-shrink: 0;
	}

	.ago {
		min-width: 2.5em;
		text-align: right;
	}

	.empty {
		padding: var(--space-4);
		text-align: center;
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
	}

	.error {
		color: var(--color-danger);
	}

	.foot {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		padding: var(--space-2) var(--space-3);
		border-top: 1px solid var(--color-border);
		font-size: var(--text-2xs);
		color: var(--color-text-tertiary);
	}

	.foot span {
		flex: 1;
		min-width: 0;
	}

	.link {
		background: none;
		border: none;
		padding: 0;
		font-size: var(--text-2xs);
		color: var(--color-accent);
		cursor: pointer;
	}

	@media (prefers-reduced-motion: reduce) {
		.picker {
			animation: none;
		}
	}

	@media (pointer: coarse) {
		.foot span {
			visibility: hidden;
		}

		.row {
			min-height: 40px;
		}
	}
</style>
