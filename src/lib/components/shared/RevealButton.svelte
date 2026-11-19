<!--
	The eye button that shows or hides what a password field holds. Used by
	Input (type="password") and by PasswordField for plain <input>s.
-->
<script lang="ts">
	import { t } from '#lib/state/i18n.svelte.js';

	interface Props {
		shown: boolean;
		disabled?: boolean;
	}

	let { shown = $bindable(false), disabled = false }: Props = $props();

	let label = $derived(shown ? t('common.hide_password') : t('common.show_password'));
</script>

<button
	type="button"
	class="reveal-btn"
	aria-label={label}
	aria-pressed={shown}
	title={label}
	{disabled}
	onmousedown={(e) => e.preventDefault()}
	onclick={() => (shown = !shown)}
>
	<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
		{#if shown}
			<path d="M17.94 17.94A10 10 0 0 1 12 20C5 20 1 12 1 12a18 18 0 0 1 5.06-5.94M9.9 4.24A9 9 0 0 1 12 4c7 0 11 8 11 8a18 18 0 0 1-2.16 3.19M1 1l22 22" />
		{:else}
			<path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8S1 12 1 12z" /><circle cx="12" cy="12" r="3" />
		{/if}
	</svg>
</button>

<style>
	.reveal-btn {
		position: absolute;
		top: 50%;
		right: 4px;
		transform: translateY(-50%);
		display: flex;
		align-items: center;
		justify-content: center;
		width: 28px;
		height: 28px;
		padding: 0;
		border: none;
		border-radius: var(--radius-btn, 6px);
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		z-index: 1;
	}

	.reveal-btn:hover:not(:disabled) {
		color: var(--color-text-primary);
		background: var(--color-bg-hover, rgba(127, 127, 127, 0.12));
	}

	.reveal-btn:focus-visible {
		outline: 2px solid var(--color-accent);
		outline-offset: -2px;
	}

	.reveal-btn:disabled {
		cursor: default;
		opacity: 0.5;
	}
</style>
