<script lang="ts">
	/**
	 * Search the session history: opened from a tab (Ctrl+Shift+H or its
	 * right-click menu, with the selection as the query), from a session's
	 * menu in the list, or from the command palette. Results can be copied,
	 * pasted into the tab without Enter (never run), shown in their whole
	 * run, or forgotten.
	 */
	import Modal from '#lib/components/shared/Modal.svelte';
	import { t } from '#lib/state/i18n.svelte.js';
	import { closeHistoryPanel, historyPanel, pasteHistoryCommand } from '#lib/state/history.svelte.js';
	import { getTerminalInput } from '#lib/state/snippets.svelte.js';
	import { getTabs } from '#lib/state/tabs.svelte.js';
	import { historyForget, historySearch, historyTranscript, type HistoryHit } from '#lib/ipc/history.js';
	import { writeText as clipboardWriteText } from '@tauri-apps/plugin-clipboard-manager';

	let open = $derived(historyPanel() !== null);
	let text = $state('');
	let scope = $state<'session' | 'all'>('all');
	let kind = $state<'all' | 'commands' | 'output'>('all');
	let hits = $state<HistoryHit[]>([]);
	let error = $state('');
	let busy = $state(false);
	/** A whole run, shown instead of the results. */
	let transcript = $state<{ hits: HistoryHit[]; focus: number } | null>(null);
	let inputEl = $state<HTMLInputElement | undefined>();

	let opened = $derived(historyPanel());
	let sessionId = $derived(opened?.sessionId ?? null);
	/** Opened from a tab running in tmux: its output is tmux's screen
	 *  redraws, so lines there can be cut or repeated. */
	let inTmux = $derived(!!getTabs().find((x) => x.id === opened?.tabId)?.sshConnectParams?.tmux);

	// Each opening starts from what it was opened with.
	$effect(() => {
		const o = historyPanel();
		if (!o) return;
		text = o.text ?? '';
		scope = o.sessionId ? 'session' : 'all';
		kind = 'all';
		transcript = null;
		queueMicrotask(() => inputEl?.focus());
	});

	let seq = 0;
	async function search(): Promise<void> {
		if (!opened) return;
		const mine = ++seq;
		busy = true;
		try {
			const result = await historySearch({
				text,
				sessionId: scope === 'session' ? sessionId : null,
				kind,
				limit: 200
			});
			if (mine !== seq) return;
			hits = result;
			error = '';
		} catch (e) {
			if (mine !== seq) return;
			hits = [];
			error = String(e).includes('locked') ? t('history.locked') : String(e);
		} finally {
			if (mine === seq) busy = false;
		}
	}

	// Searches as the user types, a moment after the last key.
	$effect(() => {
		void text;
		void scope;
		void kind;
		if (!open) return;
		const timer = setTimeout(() => void search(), 150);
		return () => clearTimeout(timer);
	});

	function when(ms: number): string {
		return new Date(ms).toLocaleString();
	}

	function clock(ms: number): string {
		return new Date(ms).toLocaleTimeString();
	}

	/** The results in runs: what a run shares (session, host, start) is said
	 *  once above its rows instead of on every row. */
	let groups = $derived.by(() => {
		const out: { run: number; first: HistoryHit; hits: HistoryHit[] }[] = [];
		for (const h of hits) {
			const last = out[out.length - 1];
			if (last && last.run === h.run) last.hits.push(h);
			else out.push({ run: h.run, first: h, hits: [h] });
		}
		return out;
	});
	/** Where a run happened: the server, or for a local shell the computer
	 *  itself; a WSL distro or serial port says which computer it was on. */
	function place(h: HistoryHit): string {
		if ((h.protocol === 'wsl' || h.protocol === 'serial') && h.device && h.device !== h.host) return `${h.host} · ${h.device}`;
		return h.host;
	}

	/** Filtered to one session, its name is in the scope menu already. */
	let oneSession = $derived(scope === 'session' && !!sessionId);

	async function copy(hit: HistoryHit): Promise<void> {
		await clipboardWriteText(hit.text).catch(() => {});
	}

	/** Into the tab it was opened from, as if pasted: never with Enter. */
	function pasteable(): boolean {
		return !!opened?.tabId && !!getTerminalInput(opened.tabId);
	}

	function paste(hit: HistoryHit): void {
		if (pasteHistoryCommand(opened?.tabId, hit.text)) closeHistoryPanel();
	}

	async function showRun(hit: HistoryHit): Promise<void> {
		try {
			transcript = { hits: await historyTranscript(hit.run), focus: hit.id };
		} catch (e) {
			error = String(e);
		}
	}

	async function forget(hit: HistoryHit): Promise<void> {
		await historyForget(hit.kind === 'command' ? { what: 'command', id: hit.id } : { what: 'output', id: hit.id });
		hits = hits.filter((h) => !(h.kind === hit.kind && h.id === hit.id));
		if (transcript) transcript.hits = transcript.hits.filter((h) => !(h.kind === hit.kind && h.id === hit.id));
	}

	async function forgetRun(run: number): Promise<void> {
		await historyForget({ what: 'run', id: run });
		hits = hits.filter((h) => h.run !== run);
		transcript = null;
	}

	function scrollFocus(node: HTMLElement, focus: boolean): void {
		if (focus) queueMicrotask(() => node.scrollIntoView({ block: 'center' }));
	}
</script>

<Modal {open} onclose={closeHistoryPanel} title={t('history.title')} maxWidth="860px">
	<div class="history">
		{#if transcript}
			<div class="bar">
				<button type="button" class="link" onclick={() => (transcript = null)}>← {t('history.back')}</button>
				{#if transcript.hits[0]}
					<span class="meta">{#if transcript.hits[0].name}{transcript.hits[0].name} · {/if}{place(transcript.hits[0])} · {when(transcript.hits[0].at)}</span>
				{/if}
				<span class="spacer"></span>
				{#if transcript.hits[0]}
					<button type="button" class="danger" onclick={() => transcript && transcript.hits[0] && void forgetRun(transcript.hits[0].run)}>{t('history.forget_run')}</button>
				{/if}
			</div>
			<div class="transcript">
				{#each transcript.hits as h (h.kind + h.id)}
					<div class="item {h.kind}" class:focus={h.id === transcript.focus} use:scrollFocus={h.id === transcript.focus}>
						{#if h.kind === 'command'}
							<span class="prompt">$</span> {h.text}{#if h.exit !== null && h.exit !== 0}<span class="exit">exit {h.exit}</span>{/if}
						{:else}
							<pre>{h.text}</pre>
						{/if}
					</div>
				{/each}
			</div>
		{:else}
			<div class="bar">
				<input
					bind:this={inputEl}
					class="query"
					type="search"
					spellcheck="false"
					placeholder={t('history.placeholder')}
					aria-label={t('history.placeholder')}
					bind:value={text}
				/>
				{#if sessionId}
					<select bind:value={scope} aria-label={t('history.scope')}>
						<option value="session">{opened?.sessionName || t('history.this_session')}</option>
						<option value="all">{t('history.all_sessions')}</option>
					</select>
				{/if}
				<select bind:value={kind} aria-label={t('history.kind')}>
					<option value="all">{t('history.kind_all')}</option>
					<option value="commands">{t('history.kind_commands')}</option>
					<option value="output">{t('history.kind_output')}</option>
				</select>
			</div>
			{#if inTmux}
				<p class="tmux-note">{t('tmux.history_note')}</p>
			{/if}
			{#if error}
				<p class="error">{error}</p>
			{:else if !busy && hits.length === 0}
				<p class="empty">{text.trim() ? t('history.no_results') : t('history.empty')}</p>
			{/if}
			<ul class="hits">
				{#each groups as g, i (g.run + ':' + i)}
					<li class="run">
						<span class="meta">{#if !oneSession && g.first.name}{g.first.name} · {/if}{place(g.first)} · {new Date(g.first.at).toLocaleDateString()}</span>
					</li>
					{#each g.hits as h (h.kind + h.id)}
						<li class="hit {h.kind}">
							<div class="head">
								<span class="badge">{h.kind === 'command' ? t('history.command') : t('history.output')}</span>
								<span class="meta" title={when(h.at)}>{clock(h.at)}{#if h.cwd} · {h.cwd}{/if}</span>
								{#if h.exit !== null && h.exit !== 0}<span class="exit">exit {h.exit}</span>{/if}
								{#if h.redacted}<span class="redacted" title={t('history.redacted_hint')}>{t('history.redacted')}</span>{/if}
							</div>
							{#if h.kind === 'command'}
								<code class="text">{h.text}</code>
							{:else}
								<pre class="text">{h.text}</pre>
							{/if}
							<div class="actions">
								<button type="button" onclick={() => void copy(h)}>{t('textmenu.copy')}</button>
								{#if h.kind === 'command' && pasteable()}
									<button type="button" onclick={() => paste(h)}>{t('history.paste')}</button>
								{/if}
								<button type="button" onclick={() => void showRun(h)}>{t('history.show_run')}</button>
								<button type="button" class="danger" onclick={() => void forget(h)}>{t('history.forget')}</button>
							</div>
						</li>
					{/each}
				{/each}
			</ul>
		{/if}
	</div>
</Modal>

<style>
	.history {
		display: flex;
		flex-direction: column;
		gap: 8px;
		min-height: 320px;
		max-height: 70vh;
	}

	.bar {
		display: flex;
		gap: 8px;
		align-items: center;
	}

	.spacer {
		flex: 1;
	}

	.query {
		flex: 1;
		min-width: 0;
		padding: 6px 8px;
		font-size: 0.8125rem;
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-sm, 4px);
	}

	select {
		padding: 5px 6px;
		font-size: 0.75rem;
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		border: 1px solid var(--color-border-strong);
		border-radius: var(--radius-sm, 4px);
	}

	.hits,
	.transcript {
		list-style: none;
		margin: 0;
		padding: 0;
		overflow: auto;
		flex: 1;
	}

	.run {
		padding: 10px 4px 2px;
		border-bottom: 1px solid var(--color-border);
		font-weight: 600;
	}

	.hit {
		padding: 6px 4px 6px 12px;
		border-bottom: 1px solid var(--color-border);
	}

	.head {
		display: flex;
		gap: 8px;
		align-items: center;
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
	}

	.badge {
		text-transform: uppercase;
		letter-spacing: 0.03em;
		font-weight: 600;
	}

	.exit {
		color: var(--color-danger);
		font-size: 0.6875rem;
		margin-left: 6px;
	}

	.redacted {
		color: var(--color-warning);
	}

	.text,
	pre {
		display: block;
		margin: 4px 0;
		font-family: var(--font-mono, monospace);
		font-size: 0.75rem;
		white-space: pre-wrap;
		word-break: break-all;
		color: var(--color-text-primary);
	}

	.actions {
		display: flex;
		gap: 10px;
	}

	.actions button,
	.link,
	.bar .danger {
		background: none;
		border: none;
		padding: 0;
		font-size: 0.6875rem;
		color: var(--color-accent);
		cursor: pointer;
	}

	.danger {
		color: var(--color-danger) !important;
	}

	.item {
		padding: 2px 4px;
		font-family: var(--font-mono, monospace);
		font-size: 0.75rem;
	}

	.item.command {
		color: var(--color-accent);
		margin-top: 6px;
	}

	.item.focus {
		background: var(--color-bg-tertiary, rgba(127, 127, 127, 0.15));
	}

	.prompt {
		opacity: 0.6;
	}

	.meta {
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
	}

	.empty,
	.error {
		font-size: 0.75rem;
		color: var(--color-text-secondary);
	}

	.error {
		color: var(--color-danger);
	}

	.tmux-note {
		margin: 0 0 8px;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
	}
</style>
