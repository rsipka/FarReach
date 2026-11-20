<!--
	Pick a session folder's icon and colour: a symbol of the app's own or a
	logo, and one of a few colours. Nothing is saved until Save.
-->
<script lang="ts">
	import Modal from '#lib/components/shared/Modal.svelte';
	import Button from '#lib/components/shared/Button.svelte';
	import FaIcon from '../shared/FaIcon.svelte';
	import FolderIcon from './FolderIcon.svelte';
	import { folderColors, folderLogos, folderSymbols } from '#lib/data/folder-icons.js';
	import { t } from '#lib/state/i18n.svelte.js';
	import { matchesQuery } from '#lib/utils/search.js';

	interface Props {
		open: boolean;
		/** The folder's name, for the title and the preview. */
		name: string;
		icon: string | null;
		color: string | null;
		onclose: () => void;
		onsave: (icon: string | null, color: string | null) => void;
	}

	let { open, name, icon, color, onclose, onsave }: Props = $props();

	let pickedIcon = $state<string | null>(null);
	let pickedColor = $state<string | null>(null);
	let query = $state('');

	// Start from the folder's own each time the dialog opens.
	$effect(() => {
		if (open) {
			pickedIcon = icon;
			pickedColor = color;
			query = '';
		}
	});

	const logos = folderLogos();
	let symbols = $derived(folderSymbols.filter((s) => matchesQuery(query, [s.key, s.words])));
	let shownLogos = $derived(logos.filter((l) => matchesQuery(query, [l.title, l.value.slice(3)])));
</script>

<Modal {open} {onclose} title={t('folder.icon_title', { name })} maxWidth="520px">
	<div class="picker">
		<div class="top">
			<div class="preview" aria-hidden="true">
				<FolderIcon icon={pickedIcon} color={pickedColor} size={18} />
				<span class="preview-name">{name}</span>
			</div>
			<input class="search" type="search" placeholder={t('folder.icon_search')} aria-label={t('folder.icon_search')} bind:value={query} />
		</div>

		<div class="label">{t('folder.icon_color')}</div>
		<div class="colors" role="radiogroup" aria-label={t('folder.icon_color')}>
			<button type="button" class="swatch none" class:current={!pickedColor} role="radio" aria-checked={!pickedColor} title={t('folder.icon_color_default')} aria-label={t('folder.icon_color_default')} onclick={() => (pickedColor = null)}>
				<svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true"><path d="M2 10L10 2" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" /></svg>
			</button>
			{#each folderColors as c (c.key)}
				<button type="button" class="swatch" class:current={pickedColor === c.key} role="radio" aria-checked={pickedColor === c.key} title={c.key} aria-label={c.key} style:background={c.hex} onclick={() => (pickedColor = c.key)}></button>
			{/each}
		</div>

		<div class="scroll">
			{#if !query.trim()}
				<div class="label">{t('folder.icon_default')}</div>
				<div class="grid">
					<button type="button" class="cell" class:current={!pickedIcon} title={t('folder.icon_default')} aria-label={t('folder.icon_default')} onclick={() => (pickedIcon = null)}>
						<FolderIcon icon={null} color={pickedColor} size={16} />
					</button>
				</div>
			{/if}
			{#if symbols.length}
				<div class="label">{t('folder.icon_symbols')}</div>
				<div class="grid">
					{#each symbols as s (s.key)}
						{@const value = `fa:${s.key}`}
						<button type="button" class="cell" class:current={pickedIcon === value} title={s.key} aria-label={s.key} onclick={() => (pickedIcon = value)}>
							<FolderIcon icon={value} color={pickedColor} size={16} />
						</button>
					{/each}
				</div>
			{/if}
			{#if shownLogos.length}
				<div class="label">{t('folder.icon_logos')}</div>
				<div class="grid">
					{#each shownLogos as l (l.value)}
						<button type="button" class="cell" class:current={pickedIcon === l.value} title={l.title} aria-label={l.title} onclick={() => (pickedIcon = l.value)}>
							<FolderIcon icon={l.value} color={pickedColor} size={16} />
						</button>
					{/each}
				</div>
			{/if}
			{#if !symbols.length && !shownLogos.length}
				<p class="empty">{t('folder.icon_none_found')}</p>
			{/if}
		</div>
	</div>
	{#snippet actions()}
		<Button variant="ghost" onclick={onclose}>{t('common.cancel')}</Button>
		<Button variant="primary" onclick={() => onsave(pickedIcon, pickedColor)}>{t('common.save')}</Button>
	{/snippet}
</Modal>

<style>
	.picker {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.top {
		display: flex;
		align-items: center;
		gap: 12px;
	}

	.preview {
		display: flex;
		align-items: center;
		gap: 8px;
		min-width: 0;
		flex: 1;
		padding: 6px 10px;
		border-radius: var(--radius-btn, 6px);
		background: var(--color-bg-secondary);
	}

	.preview-name {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-weight: 500;
		color: var(--color-text-primary);
		font-size: var(--font-size-sm, 13px);
	}

	.search {
		flex: 1;
		min-width: 0;
		padding: 6px 10px;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn, 6px);
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		font: inherit;
		font-size: var(--font-size-sm, 13px);
	}

	.label {
		margin-top: 4px;
		font-size: 11px;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.04em;
		color: var(--color-text-secondary);
	}

	.colors {
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
	}

	.swatch {
		width: 22px;
		height: 22px;
		border-radius: 50%;
		border: 2px solid transparent;
		padding: 0;
		cursor: pointer;
		box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.15);
	}

	.swatch.none {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		background: var(--color-bg-secondary);
		color: var(--color-text-secondary);
	}

	.swatch.current {
		border-color: var(--color-text-primary);
	}

	.scroll {
		max-height: min(46vh, 360px);
		overflow-y: auto;
		padding-right: 2px;
	}

	.grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(34px, 1fr));
		gap: 4px;
		margin: 4px 0 6px;
	}

	.cell {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		height: 34px;
		border: 1px solid transparent;
		border-radius: var(--radius-btn, 6px);
		background: transparent;
		cursor: pointer;
	}

	.cell:hover {
		background: var(--color-bg-secondary);
	}

	.cell.current {
		border-color: var(--color-accent);
		background: var(--color-bg-secondary);
	}

	.empty {
		margin: 12px 0;
		color: var(--color-text-secondary);
		font-size: var(--font-size-sm, 13px);
	}
</style>
