<!--
	A session folder's icon: the symbol or logo picked for it, in its colour,
	or the plain folder outline when none is picked.
-->
<script lang="ts">
	import FaIcon from '../shared/FaIcon.svelte';
	import { folderColorCss, resolveFolderIcon } from '#lib/data/folder-icons.js';

	interface Props {
		icon?: string | null;
		color?: string | null;
		size?: number;
	}

	let { icon = null, color = null, size = 12 }: Props = $props();

	let resolved = $derived(resolveFolderIcon(icon));
	let tint = $derived(folderColorCss(color));
	// A logo keeps its own colours unless a colour is picked; a black one
	// takes the text colour so it shows on a dark theme too.
	let logoFill = $derived.by(() => {
		if (resolved.kind !== 'logo') return '';
		if (tint) return tint;
		const n = parseInt(resolved.data.hex, 16);
		const c = [(n >> 16) & 255, (n >> 8) & 255, n & 255];
		return Math.max(...c) < 64 && Math.max(...c) - Math.min(...c) < 24 ? 'var(--color-text-primary)' : `#${resolved.data.hex}`;
	});
</script>

<span class="folder-icon-glyph" class:plain={resolved.kind === 'folder' && !tint} style:color={tint} style:width="{size}px" style:height="{size}px">
	{#if resolved.kind === 'symbol'}
		<FaIcon icon={resolved.icon} {size} />
	{:else if resolved.kind === 'logo'}
		<svg viewBox="0 0 24 24" width={size} height={size} fill={logoFill} aria-hidden="true"><path d={resolved.data.path} /></svg>
	{:else}
		<svg width={size} height={size} viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="M22 19a2 2 0 01-2 2H4a2 2 0 01-2-2V5a2 2 0 012-2h5l2 3h9a2 2 0 012 2z" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>
	{/if}
</span>

<style>
	.folder-icon-glyph {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
		color: var(--color-text-secondary);
	}

	/* The plain folder, as it always looked. */
	.folder-icon-glyph.plain {
		color: var(--color-warning);
		opacity: 0.7;
	}
</style>
