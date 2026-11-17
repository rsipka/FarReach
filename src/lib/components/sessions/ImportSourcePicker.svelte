<script lang="ts" module>
	import type { ImportSourceId } from '#lib/sessions/import/sources.js';

	/** Every place sessions can be imported from, in the order the picker lists them. */
	export type AnyImportSource = 'ssh_config' | 'rdp' | ImportSourceId;
	export const ALL_IMPORT_SOURCES: AnyImportSource[] = ['ssh_config', 'rdp', 'rdm', 'mremoteng', 'mobaxterm', 'putty', 'winscp', 'tabby'];
</script>

<script lang="ts">
	/**
	 * The "Import from" select at the top of every import dialog. Each source
	 * has a dialog of its own; picking another one asks the session list to
	 * open that dialog instead.
	 */
	import { IMPORT_SOURCES } from '#lib/sessions/import/sources.js';
	import { t } from '#lib/state/i18n.svelte.js';

	interface Props {
		value: AnyImportSource;
		onchange: (source: AnyImportSource) => void;
		disabled?: boolean;
	}

	let { value, onchange, disabled = false }: Props = $props();

	function label(id: AnyImportSource): string {
		if (id === 'ssh_config') return t('session.import_src_ssh_config');
		if (id === 'rdp') return t('session.import_src_rdp');
		return IMPORT_SOURCES[id].app;
	}
</script>

<label class="source-field">
	<span>{t('session.import_source')}</span>
	<select
		class="source-select"
		{value}
		onchange={(e) => onchange((e.currentTarget as HTMLSelectElement).value as AnyImportSource)}
		{disabled}
	>
		{#each ALL_IMPORT_SOURCES as id (id)}<option value={id}>{label(id)}</option>{/each}
	</select>
</label>

<style>
	.source-field {
		display: flex;
		flex-direction: column;
		gap: 4px;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
	}

	.source-select {
		box-sizing: border-box;
		width: 100%;
		padding: 6px 8px;
		border-radius: 6px;
		border: 1px solid var(--color-border);
		background: var(--color-bg-secondary);
		color: var(--color-text-primary);
		font: inherit;
		font-size: 0.8125rem;
	}
</style>
