<script lang="ts">
	import { getDistroIcon } from '#lib/data/distro-icons.js';

	let { osId, size = 16 }: { osId: string | undefined | null; size?: number } = $props();

	let icon = $derived(getDistroIcon(osId));
	// A black logo (Apple, MikroTik) takes the text colour, so it shows on a
	// dark theme and a light one alike. Every logo with a colour of its own
	// (Debian's red, Red Hat's, SUSE's green) keeps it.
	let fill = $derived(icon ? (isBlack(icon.hex) ? 'var(--color-text-primary)' : `#${icon.hex}`) : '');

	function isBlack(hex: string): boolean {
		const n = parseInt(hex, 16);
		const c = [(n >> 16) & 255, (n >> 8) & 255, n & 255];
		return Math.max(...c) < 64 && Math.max(...c) - Math.min(...c) < 24;
	}
</script>

{#if icon}
	<svg
		xmlns="http://www.w3.org/2000/svg"
		viewBox="0 0 24 24"
		width={size}
		height={size}
		fill={fill}
		aria-label={icon.title}
		role="img"
		class="distro-icon"
	>
		<path d={icon.path} />
	</svg>
{/if}

<style>
	.distro-icon {
		display: inline-flex;
		align-items: center;
		flex-shrink: 0;
		opacity: 0.85;
		transition: opacity 150ms ease;
	}

	.distro-icon:hover {
		opacity: 1;
	}
</style>
