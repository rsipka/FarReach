<script lang="ts">
	// One row of the bulk editor that picks from a short list: "Keep" first,
	// then the choices. Used for the many on/off/default settings.
	import { t } from '#lib/state/i18n.svelte.js';

	interface Props {
		id: string;
		label: string;
		/** The KEEP sentinel, or a chosen option's value. */
		value: string;
		keep: string;
		/** What the sessions have now, shown in the Keep option. */
		current: string;
		options: { value: string; label: string }[];
		/** "Applies to 8 of 24"; omitted when it applies to all. */
		applies?: string;
		tooltip?: string;
		hint?: string;
		disabled?: boolean;
	}

	let { id, label, value = $bindable(), keep, current, options, applies, tooltip = '', hint, disabled = false }: Props = $props();
</script>

<div class="row" class:changed={value !== keep}>
	<label class="label" for={id}>{label}</label>
	<div class="field">
		<select {id} bind:value title={tooltip} {disabled}>
			<option value={keep}>{t('bulk.keep_value', { value: current })}</option>
			{#each options as o (o.value)}
				<option value={o.value}>{o.label}</option>
			{/each}
		</select>
		{#if hint}
			<p class="hint">{hint}</p>
		{/if}
		{#if applies}
			<p class="applies">{applies}</p>
		{/if}
	</div>
	{#if value !== keep}
		<button class="revert" type="button" onclick={() => (value = keep)} title={t('bulk.revert')} aria-label={t('bulk.revert')}>↺</button>
	{/if}
</div>

<style>
	.row {
		display: grid;
		grid-template-columns: 130px 1fr 52px;
		align-items: start;
		gap: 8px;
		padding: 4px 0 4px 8px;
		border-left: 3px solid transparent;
	}
	.row.changed {
		border-left-color: var(--color-accent);
	}
	.label {
		padding-top: 8px;
		font-size: 0.8125rem;
		color: var(--color-text-primary);
	}
	.field {
		display: flex;
		flex-direction: column;
		gap: 4px;
		min-width: 0;
	}
	select {
		width: 100%;
		box-sizing: border-box;
		padding: 7px 10px;
		font-family: var(--font-sans);
		font-size: 0.8125rem;
		color: var(--color-text-primary);
		background-color: var(--color-bg-elevated);
		border: 1px solid var(--color-border-strong);
		border-radius: var(--radius-btn);
		outline: none;
	}
	select:focus {
		border-color: var(--color-accent);
	}
	.applies,
	.hint {
		margin: 0;
		font-size: 0.6875rem;
		color: var(--color-text-tertiary);
	}
	.revert {
		margin-top: 4px;
		padding: 3px 6px;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
		background: transparent;
		border: 1px solid var(--color-border);
		border-radius: 6px;
		cursor: pointer;
	}
	.revert:hover {
		background: var(--color-surface-hover);
		color: var(--color-text-primary);
	}
	@media (max-width: 520px) {
		.row {
			grid-template-columns: 1fr auto;
		}
		.label {
			grid-column: 1 / -1;
			padding-top: 0;
		}
	}
</style>
