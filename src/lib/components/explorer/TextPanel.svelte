<script lang="ts">
	/**
	 * Read-only text over the full-pane file manager: one file (F3 View) or
	 * two side by side with their differences (Compare files). Esc, F3 or
	 * F10 closes it.
	 */
	import { onMount } from 'svelte';
	import { t } from '#lib/state/i18n.svelte.js';
	import { diffLines, countChanges, type DiffRow } from '#lib/explorer/compare.js';

	type Content =
		| { kind: 'view'; title: string; text: string }
		| { kind: 'diff'; leftTitle: string; left: string; rightTitle: string; right: string };

	interface Props {
		content: Content;
		onclose: () => void;
	}

	let { content, onclose }: Props = $props();

	let el: HTMLDivElement | undefined = $state();
	let wrap = $state(false);
	let allLines = $state(false);

	let lines = $derived(content.kind === 'view' ? content.text.replace(/\r\n/g, '\n').split('\n') : []);
	let rows = $derived<DiffRow[]>(content.kind === 'diff' ? diffLines(content.left, content.right, allLines ? null : 3) : []);
	let changes = $derived(content.kind === 'diff' ? countChanges(rows) : 0);

	onMount(() => el?.focus());

	function onKeydown(e: KeyboardEvent): void {
		if (e.key === 'Escape' || e.key === 'F3' || e.key === 'F10' || (e.key === 'q' && !e.ctrlKey && !e.metaKey)) {
			e.preventDefault();
			e.stopPropagation();
			onclose();
		} else if (e.key === 'w' && !e.ctrlKey && !e.metaKey) {
			wrap = !wrap;
			e.stopPropagation();
		} else {
			// Arrows, Page Up/Down and Home/End scroll the text natively; keep
			// them from reaching the file lists underneath.
			e.stopPropagation();
		}
	}
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div class="panel" bind:this={el} tabindex="0" role="dialog" aria-modal="true" aria-label={content.kind === 'view' ? content.title : t('files.compare_files')} onkeydown={onKeydown}>
	<header>
		{#if content.kind === 'view'}
			<span class="title">{content.title}</span>
			<span class="meta">{t('files.lines', { count: lines.length })}</span>
		{:else}
			<span class="title">{content.leftTitle}</span>
			<span class="vs">↔</span>
			<span class="title">{content.rightTitle}</span>
			<span class="meta" class:same={changes === 0}>{changes === 0 ? t('files.identical') : t('files.changed_lines', { count: changes })}</span>
			<label class="toggle"><input type="checkbox" bind:checked={allLines} /> {t('files.all_lines')}</label>
		{/if}
		<label class="toggle"><input type="checkbox" bind:checked={wrap} /> {t('files.wrap')}</label>
		<button class="close" type="button" onclick={onclose} title={`${t('common.close')} (Esc)`} aria-label={t('common.close')}>
			<svg width="14" height="14" viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="M6 6l12 12M18 6L6 18" stroke="currentColor" stroke-width="2" stroke-linecap="round" /></svg>
		</button>
	</header>

	<div class="body" class:wrap>
		{#if content.kind === 'view'}
			<table class="text">
				<tbody>
					{#each lines as line, i (i)}
						<tr><td class="no">{i + 1}</td><td class="line">{line}</td></tr>
					{/each}
				</tbody>
			</table>
		{:else}
			<table class="text diff">
				<tbody>
					{#each rows as row, i (i)}
						{#if row.kind === 'gap'}
							<tr class="gap"><td colspan="4">{t('files.unchanged_lines', { count: row.leftNo ?? 0 })}</td></tr>
						{:else}
							<tr class={row.kind}>
								<td class="no">{row.leftNo ?? ''}</td>
								<td class="line left">{row.left ?? ''}</td>
								<td class="no">{row.rightNo ?? ''}</td>
								<td class="line right">{row.right ?? ''}</td>
							</tr>
						{/if}
					{/each}
				</tbody>
			</table>
		{/if}
	</div>
</div>

<style>
	.panel {
		position: absolute;
		inset: 0;
		z-index: 10;
		display: flex;
		flex-direction: column;
		background: var(--color-bg-primary);
		outline: none;
	}

	header {
		display: flex;
		align-items: center;
		gap: 10px;
		height: 36px;
		padding: 0 8px 0 12px;
		border-bottom: 1px solid var(--color-border);
		background: var(--color-bg-secondary);
		font-size: 0.78125rem;
		min-width: 0;
	}

	.title {
		font-family: var(--font-mono, monospace);
		color: var(--color-text-primary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		min-width: 0;
		flex: 0 1 auto;
	}

	.vs {
		color: var(--color-text-tertiary);
	}

	.meta {
		color: var(--color-text-secondary);
		white-space: nowrap;
		margin-right: auto;
	}

	.meta.same {
		color: var(--color-success);
	}

	.toggle {
		display: flex;
		align-items: center;
		gap: 4px;
		color: var(--color-text-secondary);
		white-space: nowrap;
		cursor: pointer;
	}

	.close {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 26px;
		height: 26px;
		border: none;
		border-radius: 5px;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
	}

	.close:hover {
		background: var(--color-surface-hover);
		color: var(--color-text-primary);
	}

	.body {
		flex: 1 1 auto;
		overflow: auto;
		min-height: 0;
	}

	.text {
		border-collapse: collapse;
		font-family: var(--font-mono, monospace);
		font-size: 0.78125rem;
		line-height: 1.45;
		min-width: 100%;
	}

	.diff {
		table-layout: fixed;
		width: 100%;
	}

	.diff .no {
		width: 48px;
	}

	.no {
		padding: 0 8px;
		text-align: right;
		color: var(--color-text-tertiary);
		user-select: none;
		vertical-align: top;
		white-space: nowrap;
		background: var(--color-bg-secondary);
	}

	.line {
		padding: 0 10px;
		white-space: pre;
		color: var(--color-text-primary);
		vertical-align: top;
	}

	.diff .line {
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.wrap .line {
		white-space: pre-wrap;
		word-break: break-all;
	}

	.diff .left {
		border-right: 1px solid var(--color-border);
	}

	tr.changed .line,
	tr.removed .left,
	tr.added .right {
		background: color-mix(in srgb, var(--color-warning) 14%, transparent);
	}

	tr.removed .left {
		background: color-mix(in srgb, var(--color-danger) 16%, transparent);
	}

	tr.added .right {
		background: color-mix(in srgb, var(--color-success) 16%, transparent);
	}

	tr.gap td {
		padding: 2px 12px;
		color: var(--color-text-tertiary);
		background: var(--color-surface-sunken);
		font-family: inherit;
		font-style: italic;
	}
</style>
