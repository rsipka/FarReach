<script lang="ts">
	/**
	 * Above the terminals while any is chosen for broadcast: what typing in
	 * the one in front does, and the way out. Typing into the wrong servers
	 * is the danger, so this never hides while a broadcast is on.
	 */
	import { getActiveTab } from '#lib/state/tabs.svelte.js';
	import {
		getBroadcastTargets,
		isTerminalTab,
		stopBroadcast,
		toggleBroadcastTarget
	} from '#lib/state/broadcast.svelte.js';
	import { BROADCAST_ICON } from '#lib/terminal/broadcast.js';
	import { shortcutLabel } from '#lib/state/shortcuts.svelte.js';
	import { t } from '#lib/state/i18n.svelte.js';

	let targets = $derived(getBroadcastTargets());
	let active = $derived(getActiveTab());
	let here = $derived(!!active && targets.includes(active.id));
	let canJoin = $derived(!here && isTerminalTab(active));
	let on = $derived(targets.length >= 2);

	let message = $derived.by(() => {
		if (!on) return t('broadcast.bar_one');
		if (here) return t('broadcast.bar_on', { count: String(targets.length) });
		return t('broadcast.bar_elsewhere', { count: String(targets.length) });
	});
</script>

{#if targets.length > 0}
	<div class="broadcast-bar" class:live={on && here} role="status" aria-live="polite">
		<svg width="16" height="16" viewBox="0 0 24 24" fill="none" aria-hidden="true">
			<path d={BROADCAST_ICON} stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
		</svg>
		<span class="text">{message}</span>
		{#if canJoin && active}
			{@const id = active.id}
			<button class="bar-btn" onclick={() => toggleBroadcastTarget(id)}>{t('broadcast.include_this')}</button>
		{:else if here && active}
			{@const id = active.id}
			<button class="bar-btn" onclick={() => toggleBroadcastTarget(id)} title={shortcutLabel('B', { ctrl: true, shift: true })}>
				{t('broadcast.leave_this')}
			</button>
		{/if}
		<button class="bar-btn stop" onclick={stopBroadcast}>{t('broadcast.stop')}</button>
	</div>
{/if}

<style>
	.broadcast-bar {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 4px 10px;
		min-height: 30px;
		font-size: var(--text-sm, 0.8125rem);
		color: var(--color-text-primary);
		background: color-mix(in srgb, var(--color-warning) 14%, var(--color-bg-primary));
		border-bottom: 1px solid color-mix(in srgb, var(--color-warning) 50%, transparent);
		flex-shrink: 0;
	}

	/* Typing here goes to several servers: as loud as the bar gets. */
	.broadcast-bar.live {
		background: var(--color-warning);
		color: #1c1c1e;
		font-weight: 600;
		border-bottom-color: var(--color-warning);
	}

	.broadcast-bar svg {
		flex-shrink: 0;
	}

	.text {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.bar-btn {
		flex-shrink: 0;
		padding: 2px 10px;
		border-radius: 6px;
		border: 1px solid currentColor;
		background: transparent;
		color: inherit;
		font: inherit;
		font-weight: 600;
		cursor: pointer;
	}

	.bar-btn:hover {
		background: color-mix(in srgb, currentColor 12%, transparent);
	}

	.bar-btn:focus-visible {
		outline: 2px solid var(--color-accent);
		outline-offset: 1px;
	}
</style>
