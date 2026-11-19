<script lang="ts">
	import { getDistroIcon } from '#lib/data/distro-icons.js';

	let { osId, size = 16 }: { osId: string | undefined | null; size?: number } = $props();

	let icon = $derived(getDistroIcon(osId));
	// A near-black logo (Apple, MikroTik) takes the text colour, so it shows
	// on a dark theme and a light one alike.
	let fill = $derived(icon ? (isDark(icon.hex) ? 'var(--color-text-primary)' : `#${icon.hex}`) : '');

	function isDark(hex: string): boolean {
		const n = parseInt(hex, 16);
		const [r, g, b] = [(n >> 16) & 255, (n >> 8) & 255, n & 255];
		return 0.2126 * r + 0.7152 * g + 0.0722 * b < 64;
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
