<script lang="ts">
	/**
	 * Where themes and recipes are listed from. Each defaults to the registry
	 * the app ships with; a saved address (https only) replaces it until Reset.
	 * The plugin marketplace keeps its own field in the Marketplace panel.
	 */
	import { onMount } from 'svelte';
	import Button from '#lib/components/shared/Button.svelte';
	import { addToast } from '#lib/state/toasts.svelte.js';
	import { t } from '#lib/state/i18n.svelte.js';
	import { registryUrlGet, registryUrlReset, registryUrlSet, type RegistryKind } from '#lib/ipc/registries.js';

	interface Row {
		kind: RegistryKind;
		label: string;
		input: string;
		isDefault: boolean;
		busy: boolean;
	}

	let rows = $state<Row[]>([
		{ kind: 'themes', label: 'settings.registry_themes', input: '', isDefault: true, busy: false },
		{ kind: 'recipes', label: 'settings.registry_recipes', input: '', isDefault: true, busy: false }
	]);

	async function load(row: Row): Promise<void> {
		try {
			const r = await registryUrlGet(row.kind);
			row.input = r.url;
			row.isDefault = r.isDefault;
		} catch (e) {
			addToast(String(e), 'error');
		}
	}

	async function save(row: Row): Promise<void> {
		const url = row.input.trim();
		if (!url || row.busy) return;
		row.busy = true;
		try {
			await registryUrlSet(row.kind, url);
			await load(row);
			addToast(t('settings.registry_saved'), 'success');
		} catch (e) {
			addToast(String(e), 'error');
		} finally {
			row.busy = false;
		}
	}

	async function reset(row: Row): Promise<void> {
		if (row.busy) return;
		row.busy = true;
		try {
			await registryUrlReset(row.kind);
			await load(row);
			addToast(t('settings.registry_reset_done'), 'success');
		} catch (e) {
			addToast(String(e), 'error');
		} finally {
			row.busy = false;
		}
	}

	onMount(() => {
		for (const row of rows) void load(row);
	});
</script>

<div class="section">
	<h3 class="section-title">{t('settings.registries')}</h3>
	<p class="section-desc">{t('settings.registries_desc')}</p>

	{#each rows as row (row.kind)}
		<div class="registry">
			<span class="registry-label">
				{t(row.label)}
				{#if row.isDefault}<span class="default-tag">{t('settings.registry_default')}</span>{/if}
			</span>
			<div class="registry-row">
				<input
					type="url"
					class="registry-input"
					spellcheck="false"
					autocomplete="off"
					aria-label={t(row.label)}
					bind:value={row.input}
					onkeydown={(e) => e.key === 'Enter' && save(row)}
				/>
				<Button variant="secondary" size="sm" onclick={() => save(row)} disabled={row.busy}>
					{t('settings.registry_save')}
				</Button>
				<Button variant="ghost" size="sm" onclick={() => reset(row)} disabled={row.busy || row.isDefault}>
					{t('settings.registry_reset')}
				</Button>
			</div>
		</div>
	{/each}
</div>

<style>
	.section {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}

	.section-desc {
		margin: 0;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
	}

	.registry {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.registry-label {
		font-size: 0.8125rem;
		color: var(--color-text-primary);
	}

	.default-tag {
		margin-left: 6px;
		font-size: 0.6875rem;
		color: var(--color-text-tertiary);
	}

	.registry-row {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.registry-input {
		flex: 1;
		min-width: 0;
		padding: 6px 10px;
		font-size: 0.8125rem;
		font-family: var(--font-mono);
		color: var(--color-text-primary);
		background-color: var(--color-bg-secondary);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-input, 6px);
		outline: none;
	}

	.registry-input:focus {
		border-color: var(--color-accent);
	}
</style>
