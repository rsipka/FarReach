<script lang="ts">
	/**
	 * One question in a setup step: a label and a one-line hint on the left,
	 * its control on the right. Narrower than a Settings row on purpose; the
	 * full explanation stays in Settings, where each of these lives too.
	 */
	import type { Snippet } from 'svelte';

	interface Props {
		label: string;
		hint?: string;
		/** Belongs to the row above: indented, as in Settings. */
		sub?: boolean;
		children: Snippet;
	}

	let { label, hint, sub = false, children }: Props = $props();
</script>

<div class="row" class:sub>
	<div class="info">
		<span class="label">{label}</span>
		{#if hint}<span class="hint">{hint}</span>{/if}
	</div>
	<div class="control">{@render children()}</div>
</div>

<style>
	.row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-3);
		padding: var(--space-2) 0;
	}

	.row.sub {
		padding-left: var(--space-4);
	}

	.info {
		display: flex;
		flex-direction: column;
		gap: 2px;
		min-width: 0;
	}

	.label {
		font-size: var(--text-sm);
		font-weight: 500;
		color: var(--color-text-primary);
	}

	.hint {
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
	}

	.control {
		flex-shrink: 0;
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}

	/* A phone is too narrow for a dropdown or a text field beside two lines
	   of text; a switch still fits beside them. */
	@media (max-width: 440px) {
		.row:has(:global(select), :global(input), :global(.segmented)) {
			flex-direction: column;
			align-items: stretch;
		}
		.control {
			justify-content: flex-end;
		}
	}
</style>
