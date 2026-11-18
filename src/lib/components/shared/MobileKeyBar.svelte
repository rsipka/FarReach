<script lang="ts" module>
	/** A key the phone keyboard does not have, or hides behind a long press. */
	export type BarKey =
		| 'esc'
		| 'tab'
		| 'up'
		| 'down'
		| 'left'
		| 'right'
		| 'home'
		| 'end'
		| 'pgup'
		| 'pgdn'
		| 'del'
		| 'win'
		| 'ctrl-alt-del'
		| 'smaller'
		| 'larger'
		| { fn: number }
		| { char: string };
</script>

<script lang="ts">
	/**
	 * The keys a phone keyboard lacks, above it, as Termux does: Esc, Tab,
	 * Ctrl and Alt, the arrows and the keys around them, and the symbols a
	 * shell needs most. Ctrl and Alt are sticky: tap one, then a key on the
	 * phone keyboard or here, and it applies to that key only.
	 *
	 * The buttons never take focus. Taking it would close the phone keyboard
	 * the user is typing on, so every press is handled on pointerdown with
	 * the default (focus) prevented.
	 */
	import type { Snippet } from 'svelte';
	import { t } from '#lib/state/i18n.svelte.js';
	import { getSettings } from '#lib/state/settings.svelte.js';
	import { barKeys, barKeyLabel } from '#lib/terminal/bar-keys.js';

	interface Props {
		/** Sticky Ctrl, bound so the owner can apply it to typed text. */
		ctrl?: boolean;
		/** Sticky Alt, likewise. */
		alt?: boolean;
		/** Remote desktop adds Win, Del and Ctrl+Alt+Del. */
		desktop?: boolean;
		/** A terminal adds smaller and larger text, as a phone has no wheel. */
		zoom?: boolean;
		onkey: (key: BarKey) => void;
		/** Buttons of the owner's own, before the keys (keyboard, full screen). */
		leading?: Snippet;
	}

	let { ctrl = $bindable(false), alt = $bindable(false), desktop = false, zoom = false, onkey, leading }: Props = $props();

	const settings = getSettings();
	/** The user's keys, in their order (Settings > Terminal > Phone keys). */
	let keys = $derived(barKeys(settings.mobileBarKeys));

	const ARIA: Record<string, string> = { left: 'keys.left', up: 'keys.up', down: 'keys.down', right: 'keys.right' };

	function send(id: string): void {
		if (id === 'ctrl') ctrl = !ctrl;
		else if (id === 'alt') alt = !alt;
		else if (id.startsWith('sym:')) onkey({ char: id.slice(4) });
		else if (/^f\d+$/.test(id)) onkey({ fn: Number(id.slice(1)) });
		else onkey(id as BarKey);
	}

	function press(e: PointerEvent, action: () => void): void {
		e.preventDefault();
		action();
	}
</script>

<div class="key-bar" role="toolbar" aria-label={t('keys.extra')}>
	{#if leading}
		{@render leading()}
		<span class="divider" aria-hidden="true"></span>
	{/if}
	{#each keys as id, i (id)}
		<button
			type="button"
			tabindex="-1"
			class:symbol={id.startsWith('sym:')}
			class:on={(id === 'ctrl' && ctrl) || (id === 'alt' && alt)}
			aria-pressed={id === 'ctrl' ? ctrl : id === 'alt' ? alt : undefined}
			aria-label={ARIA[id] ? t(ARIA[id]) : undefined}
			onpointerdown={(e) => press(e, () => send(id))}>{barKeyLabel(id)}</button
		>
		<!-- Remote desktop's Win sits after Alt, where it is on a keyboard. -->
		{#if desktop && id === 'alt'}
			<button type="button" tabindex="-1" onpointerdown={(e) => press(e, () => onkey('win'))}>Win</button>
		{/if}
		{#if desktop && i === keys.length - 1}
			{#if !keys.includes('alt')}
				<button type="button" tabindex="-1" onpointerdown={(e) => press(e, () => onkey('win'))}>Win</button>
			{/if}
			{#if !keys.includes('del')}
				<button type="button" tabindex="-1" onpointerdown={(e) => press(e, () => onkey('del'))}>Del</button>
			{/if}
			<button type="button" tabindex="-1" onpointerdown={(e) => press(e, () => onkey('ctrl-alt-del'))}>Ctrl+Alt+Del</button>
		{/if}
	{/each}
	{#if zoom}
		<span class="divider" aria-hidden="true"></span>
		<button type="button" tabindex="-1" aria-label={t('keys.smaller')} onpointerdown={(e) => press(e, () => onkey('smaller'))}
			>A−</button
		>
		<button type="button" tabindex="-1" aria-label={t('keys.larger')} onpointerdown={(e) => press(e, () => onkey('larger'))}
			>A+</button
		>
	{/if}
</div>

<style>
	.key-bar {
		display: flex;
		align-items: center;
		gap: 4px;
		padding: 4px 6px;
		overflow-x: auto;
		scrollbar-width: none;
		background: var(--color-bg-secondary);
		border-top: 1px solid var(--color-border);
		/* No room for the gesture bar here: the app keeps clear of it below
		   everything (AppShell), and this bar is never the last thing on screen. */
		flex-shrink: 0;
		touch-action: pan-x;
	}

	.key-bar::-webkit-scrollbar {
		display: none;
	}

	.key-bar button,
	.key-bar :global(.bar-button) {
		flex-shrink: 0;
		min-width: 44px;
		height: 38px;
		padding: 0 10px;
		font-family: var(--font-mono, monospace);
		font-size: 0.8125rem;
		color: var(--color-text-primary);
		background: var(--color-surface-hover);
		border: 1px solid var(--color-border);
		border-radius: 8px;
		cursor: pointer;
		user-select: none;
		-webkit-user-select: none;
	}

	.key-bar button:active,
	.key-bar :global(.bar-button:active) {
		background: var(--color-surface-active);
	}

	.key-bar button.on {
		color: var(--color-bg-primary);
		background: var(--color-accent-fill);
		border-color: var(--color-accent);
	}

	.key-bar button.symbol {
		min-width: 38px;
		font-size: 1rem;
	}

	.divider {
		flex-shrink: 0;
		width: 1px;
		height: 24px;
		margin: 0 2px;
		background: var(--color-border);
	}
</style>
