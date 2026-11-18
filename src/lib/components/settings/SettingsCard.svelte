<script lang="ts">
	import type { Snippet } from 'svelte';

	/** One section of a Settings page: a heading and its rows, in a box, so
	 *  the page reads as a few groups instead of one long list. */
	let { title, children }: { title?: string; children: Snippet } = $props();
</script>

<section class="settings-card">
	{#if title}<h3 class="card-title">{title}</h3>{/if}
	<div class="card-body">
		{@render children()}
	</div>
</section>

<style>
	.settings-card {
		margin-bottom: 18px;
	}

	.card-title {
		margin: 0 0 6px 2px;
		font-size: 0.75rem;
		font-weight: 600;
		letter-spacing: 0.02em;
		text-transform: uppercase;
		color: var(--color-text-secondary);
	}

	.card-body {
		padding: 2px 16px;
		background: var(--color-bg-secondary);
		border: 1px solid var(--color-border);
		border-radius: 10px;
	}

</style>
