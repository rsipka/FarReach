<script lang="ts">
	import type { Snippet } from 'svelte';
	import { t } from '#lib/state/i18n.svelte.js';

	/** The rows people rarely change, closed until asked for. Settings search
	 *  still finds them, and opens this when it does. */
	let { children }: { children: Snippet } = $props();
</script>

<details class="settings-advanced">
	<summary>
		<svg class="chev" width="12" height="12" viewBox="0 0 24 24" fill="none" aria-hidden="true">
			<path d="M9 6l6 6-6 6" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" />
		</svg>
		{t('settings.advanced')}
	</summary>
	<div class="card-body">
		{@render children()}
	</div>
</details>

<style>
	.settings-advanced {
		margin-bottom: 18px;
	}

	summary {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		padding: 6px 8px;
		margin-left: -6px;
		font-size: 0.8125rem;
		font-weight: 600;
		color: var(--color-text-secondary);
		border-radius: 6px;
		cursor: pointer;
		list-style: none;
		user-select: none;
	}

	summary::-webkit-details-marker {
		display: none;
	}

	summary:hover {
		color: var(--color-text-primary);
		background: var(--color-surface-hover);
	}

	.chev {
		transition: transform var(--duration-default) var(--ease-default);
	}

	details[open] .chev {
		transform: rotate(90deg);
	}

	.card-body {
		margin-top: 6px;
		padding: 2px 16px;
		background: var(--color-bg-secondary);
		border: 1px solid var(--color-border);
		border-radius: 10px;
	}
</style>
