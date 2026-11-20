<script lang="ts">
	import { openUrl } from '@tauri-apps/plugin-opener';
	import { parseReleaseNotes } from '#lib/updater/release-notes.js';

	/** Release notes from the update feed, drawn as headings, a list and
	 *  paragraphs, with web links that open in the browser. */
	let { notes }: { notes: string | null } = $props();

	let blocks = $derived(parseReleaseNotes(notes));
</script>

{#snippet line(parts: ReturnType<typeof parseReleaseNotes>[number]['parts'])}
	{#each parts as part, i (i)}
		{#if 'url' in part}
			<a
				href={part.url}
				title={part.url}
				onclick={(e) => {
					e.preventDefault();
					void openUrl(part.url);
				}}>{part.url}</a
			>
		{:else}{part.text}{/if}
	{/each}
{/snippet}

<div class="notes">
	{#each blocks as block, i (i)}
		{#if block.kind === 'heading'}
			<h4>{@render line(block.parts)}</h4>
		{:else if block.kind === 'item'}
			<div class="item"><span class="bullet" aria-hidden="true">•</span><span>{@render line(block.parts)}</span></div>
		{:else}
			<p>{@render line(block.parts)}</p>
		{/if}
	{/each}
</div>

<style>
	.notes {
		font-size: 0.8125rem;
		line-height: 1.55;
		color: var(--color-text-secondary);
		word-wrap: break-word;
		overflow-wrap: anywhere;
	}

	h4 {
		margin: 10px 0 4px;
		font-size: 0.8125rem;
		font-weight: 600;
		color: var(--color-text-primary);
	}

	h4:first-child,
	p:first-child {
		margin-top: 0;
	}

	p {
		margin: 8px 0;
	}

	p:last-child {
		margin-bottom: 0;
	}

	.item {
		display: flex;
		gap: 8px;
		margin: 2px 0;
		color: var(--color-text-primary);
	}

	.bullet {
		color: var(--color-accent);
		flex-shrink: 0;
	}

	a {
		color: var(--color-accent);
		text-decoration: underline;
		text-underline-offset: 2px;
		cursor: pointer;
	}
</style>
