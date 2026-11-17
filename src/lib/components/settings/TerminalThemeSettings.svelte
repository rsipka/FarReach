<script lang="ts">
	import Button from '#lib/components/shared/Button.svelte';
	import TerminalThemeList from '#lib/components/terminal/TerminalThemeList.svelte';
	import TerminalThemeEditor from '#lib/components/terminal/TerminalThemeEditor.svelte';
	import { getSettings, updateSetting } from '#lib/state/settings.svelte.js';
	import { themeState } from '#lib/state/theme.svelte.js';
	import { t } from '#lib/state/i18n.svelte.js';
	import { addToast } from '#lib/state/toasts.svelte.js';
	import { matchesQuery } from '#lib/utils/search.js';
	import { openUrl } from '@tauri-apps/plugin-opener';
	import {
		DOWNLOADABLE_SCHEMES,
		copyOfTerminalTheme,
		deleteCustomTerminalTheme,
		downloadTerminalScheme,
		findTerminalTheme,
		importTerminalSchemes,
		terminalThemes,
		type CustomTerminalTheme
	} from '#lib/state/terminal-themes.svelte.js';

	/**
	 * Settings → Appearance → Terminal colours: the default terminal theme,
	 * apart from the app theme, and the user's own themes (copied, edited,
	 * imported from a file or downloaded from iTerm2-Color-Schemes).
	 */

	const COLLECTION_URL = 'https://github.com/mbadolato/iTerm2-Color-Schemes';

	const settings = getSettings();
	let selected = $derived(settings.terminalThemeId ?? '');
	let current = $derived(findTerminalTheme(selected));
	let canEditOwn = $derived(terminalThemes.loaded);

	/** The theme open in the editor; a copy not saved yet for a new one. */
	let editing = $state<CustomTerminalTheme | null>(null);
	/** A new copy becomes the default once saved, as the copied one was. */
	let selectOnSave = false;

	function choose(id: string): void {
		updateSetting('terminalThemeId', id);
	}

	function copyAndEdit(): void {
		const from = current ?? { name: themeState.current.name, palette: themeState.current.terminal };
		editing = copyOfTerminalTheme(from, t('termtheme.copy_name', { name: from.name }));
		selectOnSave = true;
	}

	function editOwn(): void {
		const own = terminalThemes.custom.find((c) => c.id === selected);
		if (own) {
			editing = { ...own, palette: { ...own.palette } };
			selectOnSave = false;
		}
	}

	async function removeOwn(): Promise<void> {
		const id = selected;
		try {
			await deleteCustomTerminalTheme(id);
			choose('');
		} catch (err) {
			addToast(String(err), 'error');
		}
	}

	let fileInput = $state<HTMLInputElement | undefined>();

	async function importFile(e: Event & { currentTarget: HTMLInputElement }): Promise<void> {
		const file = e.currentTarget.files?.[0];
		e.currentTarget.value = '';
		if (!file) return;
		try {
			if (file.size > 1024 * 1024) throw new Error(t('termtheme.file_too_large'));
			const added = await importTerminalSchemes(await file.text(), file.name);
			if (added.length === 1) choose(added[0].id);
			addToast(t('termtheme.imported', { count: String(added.length) }), 'success');
		} catch (err) {
			addToast(t('termtheme.import_failed', { error: err instanceof Error ? err.message : String(err) }), 'error', 8000);
		}
	}

	let moreQuery = $state('');
	let downloading = $state<string | null>(null);
	let moreMatches = $derived(
		moreQuery.trim() ? DOWNLOADABLE_SCHEMES.filter((n) => matchesQuery(moreQuery, [n])).slice(0, 12) : []
	);

	async function download(name: string): Promise<void> {
		downloading = name;
		try {
			const theme = await downloadTerminalScheme(name);
			choose(theme.id);
			addToast(t('termtheme.downloaded', { name }), 'success');
			moreQuery = '';
		} catch (err) {
			addToast(t('termtheme.download_failed', { error: err instanceof Error ? err.message : String(err) }), 'error', 8000);
		} finally {
			downloading = null;
		}
	}
</script>

<div class="setting-section">
	<span class="section-label">{t('termtheme.title')}</span>
	<p class="hint">{t('termtheme.hint')}</p>

	<TerminalThemeList
		{selected}
		onselect={choose}
		noneLabel={t('termtheme.follow_app', { name: themeState.current.name })}
		nonePalette={themeState.current.terminal}
	/>

	<div class="actions">
		<Button variant="secondary" size="sm" onclick={copyAndEdit} disabled={!canEditOwn}>{t('termtheme.copy_edit')}</Button>
		{#if current?.kind === 'custom'}
			<Button variant="secondary" size="sm" onclick={editOwn} disabled={!canEditOwn}>{t('termtheme.edit')}</Button>
			<Button variant="danger" size="sm" onclick={removeOwn} disabled={!canEditOwn}>{t('termtheme.delete')}</Button>
		{/if}
		<Button variant="secondary" size="sm" onclick={() => fileInput?.click()} disabled={!canEditOwn}>{t('termtheme.import')}</Button>
		<input bind:this={fileInput} type="file" accept=".itermcolors,.json" class="hidden" onchange={importFile} tabindex="-1" aria-hidden="true" />
	</div>
	{#if !canEditOwn}
		<p class="hint">{t('termtheme.locked')}</p>
	{/if}

	<div class="more">
		<span class="sub-label">{t('termtheme.more_title')}</span>
		<p class="hint">
			{t('termtheme.more_hint', { count: String(DOWNLOADABLE_SCHEMES.length) })}
			<button type="button" class="link" onclick={() => void openUrl(COLLECTION_URL)}>iTerm2-Color-Schemes</button>
		</p>
		<input
			class="more-search"
			type="text"
			bind:value={moreQuery}
			placeholder={t('termtheme.more_search')}
			aria-label={t('termtheme.more_search')}
			autocomplete="off"
			spellcheck="false"
			disabled={!canEditOwn}
		/>
		{#if moreMatches.length > 0}
			<div class="more-rows">
				{#each moreMatches as name (name)}
					<div class="more-row">
						<span class="more-name">{name}</span>
						<Button variant="secondary" size="sm" onclick={() => download(name)} disabled={downloading !== null}>
							{downloading === name ? t('termtheme.downloading') : t('termtheme.download')}
						</Button>
					</div>
				{/each}
			</div>
		{:else if moreQuery.trim()}
			<p class="hint">{t('termtheme.no_match')}</p>
		{/if}
	</div>
</div>

<TerminalThemeEditor
	theme={editing}
	onclose={() => (editing = null)}
	onsaved={(id) => {
		if (selectOnSave) choose(id);
	}}
/>

<style>
	.setting-section {
		padding: 12px 0;
		border-bottom: 1px solid var(--color-border);
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.section-label {
		display: block;
		font-size: 0.875rem;
		font-weight: 500;
		color: var(--color-text-primary);
	}

	.sub-label {
		font-size: 0.8125rem;
		font-weight: 500;
		color: var(--color-text-primary);
	}

	.hint {
		margin: 0;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
		line-height: 1.4;
	}

	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
	}

	.hidden {
		display: none;
	}

	.more {
		display: flex;
		flex-direction: column;
		gap: 6px;
		margin-top: 6px;
	}

	.more-search {
		padding: 6px 10px;
		font-size: 0.8125rem;
		color: var(--color-text-primary);
		background: var(--color-bg-primary);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn, 6px);
		outline: none;
	}

	.more-search:focus {
		border-color: var(--color-accent);
	}

	.more-rows {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.more-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
		padding: 2px 4px;
		font-size: 0.8125rem;
		color: var(--color-text-primary);
	}

	.more-name {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.link {
		padding: 0;
		border: none;
		background: none;
		color: var(--color-accent);
		font: inherit;
		cursor: pointer;
	}
</style>
