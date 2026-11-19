<!--
	Adds the show/hide eye to a plain <input>. The input stays in the caller's
	markup (so its own styles keep applying) and takes the type from the snippet:

	<PasswordField>
		{#snippet children(type)}
			<input {type} class="my-input" bind:value={secret} />
		{/snippet}
	</PasswordField>
-->
<script lang="ts">
	import type { Snippet } from 'svelte';
	import RevealButton from './RevealButton.svelte';

	interface Props {
		children: Snippet<['password' | 'text']>;
		disabled?: boolean;
		/** Keep the wrapper as wide as its content instead of filling the row. */
		inline?: boolean;
		/** False for a field that is not secret right now: plain text, no eye. */
		secret?: boolean;
	}

	let { children, disabled = false, inline = false, secret = true }: Props = $props();

	let shown = $state(false);
</script>

<div class="password-field" class:inline class:secret>
	{@render children(!secret || shown ? 'text' : 'password')}
	{#if secret}
		<RevealButton bind:shown {disabled} />
	{/if}
</div>

<style>
	.password-field {
		position: relative;
		display: flex;
		align-items: stretch;
		width: 100%;
		min-width: 0;
	}

	.password-field.inline {
		display: inline-flex;
		width: auto;
	}

	.password-field > :global(input) {
		flex: 1 1 auto;
		min-width: 0;
	}

	.password-field.secret > :global(input) {
		padding-right: 36px !important;
	}
</style>
