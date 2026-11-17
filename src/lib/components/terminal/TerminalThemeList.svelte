<script lang="ts">
	import { t } from '#lib/state/i18n.svelte.js';
	import { matchesQuery } from '#lib/utils/search.js';
	import { allTerminalThemes, findTerminalTheme, type TerminalTheme } from '#lib/state/terminal-themes.svelte.js';
	import type { TerminalPalette } from '#lib/state/theme.svelte.js';

	/**
	 * A searchable list of terminal themes, each drawn in its own colours.
	 * The first row is "none" (`noneLabel`): follow the app theme in
	 * Settings, the default in a session.
	 */
	interface Props {
		/** The chosen theme's id; empty for the "none" row. */
		selected: string;
		onselect: (id: string) => void;
		noneLabel: string;
		/** The colours the "none" row stands for, to draw it with. */
		nonePalette: TerminalPalette;
		disabled?: boolean;
	}

	let { selected, onselect, noneLabel, nonePalette, disabled = false }: Props = $props();

	/** Rows drawn at a time: 400-odd themes each with 17 swatches is a lot of DOM. */
	const PAGE = 60;
	let query = $state('');
	let shown = $state(PAGE);

	const groupLabel = (kind: TerminalTheme['kind']) =>
		kind === 'custom' ? t('termtheme.group_custom') : kind === 'app' ? t('termtheme.group_app') : t('termtheme.group_schemes');

	let matching = $derived(
		allTerminalThemes().filter((th) =>
			matchesQuery(query, [th.name, th.light ? t('termtheme.light') : t('termtheme.dark'), groupLabel(th.kind)])
		)
	);
	let visible = $derived(matching.slice(0, shown));
	let current = $derived(findTerminalTheme(selected));

	$effect(() => {
		query;
		shown = PAGE;
	});

	const ANSI: (keyof TerminalPalette)[] = ['black', 'red', 'green', 'yellow', 'blue', 'magenta', 'cyan', 'white'];
	const BRIGHT: (keyof TerminalPalette)[] = ['brightBlack', 'brightRed', 'brightGreen', 'brightYellow', 'brightBlue', 'brightMagenta', 'brightCyan', 'brightWhite'];
</script>

{#snippet sample(p: TerminalPalette, label: string)}
	<span class="sample" style:background={p.background} style:color={p.foreground}>
		<span class="sample-text"><span style:color={p.green}>user</span>@<span style:color={p.blue}>{label}</span>:~$</span>
		<span class="dots">
			{#each ANSI as k (k)}<span class="dot" style:background={p[k]}></span>{/each}
		</span>
		<span class="dots">
			{#each BRIGHT as k (k)}<span class="dot" style:background={p[k]}></span>{/each}
		</span>
	</span>
{/snippet}

<div class="tt-list">
	<input
		class="tt-search"
		type="text"
		bind:value={query}
		autocomplete="off"
		spellcheck="false"
		placeholder={t('termtheme.search')}
		aria-label={t('termtheme.search')}
		{disabled}
	/>
	<div class="tt-rows" role="listbox" aria-label={t('termtheme.title')}>
		{#if !query.trim()}
			<button type="button" role="option" aria-selected={!current} class="tt-row" class:active={!current} onclick={() => onselect('')} {disabled}>
				<span class="tt-name">{noneLabel}</span>
				{@render sample(nonePalette, 'server')}
			</button>
		{/if}
		{#each visible as th, i (th.id)}
			{#if i === 0 || visible[i - 1].kind !== th.kind}
				<div class="tt-group">{groupLabel(th.kind)}</div>
			{/if}
			<button
				type="button"
				role="option"
				aria-selected={current?.id === th.id}
				class="tt-row"
				class:active={current?.id === th.id}
				onclick={() => onselect(th.id)}
				title={th.name}
				{disabled}
			>
				<span class="tt-name">{th.name}</span>
				{@render sample(th.palette, 'server')}
			</button>
		{/each}
		{#if matching.length === 0}
			<div class="tt-empty">{t('termtheme.no_match')}</div>
		{/if}
		{#if matching.length > visible.length}
			<button type="button" class="tt-more" onclick={() => (shown += PAGE)}>
				{t('termtheme.show_more', { shown: String(visible.length), total: String(matching.length) })}
			</button>
		{/if}
	</div>
</div>

<style>
	.tt-list {
		display: flex;
		flex-direction: column;
		gap: 6px;
		min-width: 0;
	}

	.tt-search {
		width: 100%;
		padding: 6px 10px;
		font-size: 0.8125rem;
		color: var(--color-text-primary);
		background: var(--color-bg-primary);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn, 6px);
		outline: none;
		box-sizing: border-box;
	}

	.tt-search:focus {
		border-color: var(--color-accent);
	}

	.tt-rows {
		display: flex;
		flex-direction: column;
		gap: 2px;
		max-height: 300px;
		overflow-y: auto;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn, 6px);
		padding: 4px;
		background: var(--color-bg-secondary);
	}

	.tt-group {
		padding: 6px 6px 2px;
		font-size: 0.6875rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.03em;
		color: var(--color-text-tertiary);
	}

	.tt-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 10px;
		padding: 4px 6px;
		border: 1px solid transparent;
		border-radius: 6px;
		background: transparent;
		color: var(--color-text-primary);
		font-size: 0.8125rem;
		text-align: left;
		cursor: pointer;
	}

	.tt-row:hover {
		background: var(--color-surface-hover);
	}

	.tt-row.active {
		border-color: var(--color-accent);
		background: var(--color-surface-active);
	}

	.tt-name {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.sample {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 2px;
		flex-shrink: 0;
		padding: 3px 6px;
		border-radius: 4px;
		border: 1px solid var(--color-border);
		font-family: var(--font-mono, monospace);
		font-size: 0.6875rem;
		line-height: 1.2;
	}

	.dots {
		display: flex;
		gap: 2px;
	}

	.dot {
		width: 8px;
		height: 8px;
		border-radius: 2px;
	}

	.tt-empty {
		padding: 10px;
		font-size: 0.8125rem;
		color: var(--color-text-tertiary);
	}

	.tt-more {
		margin-top: 4px;
		padding: 6px;
		border: none;
		background: transparent;
		color: var(--color-accent);
		font-size: 0.8125rem;
		cursor: pointer;
	}
</style>
