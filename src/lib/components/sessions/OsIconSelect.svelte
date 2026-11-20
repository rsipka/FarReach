<script lang="ts">
	/**
	 * The session editor's OS icon picker: a dropdown that shows each logo
	 * beside its name, which a native <select> cannot. Typing filters the list.
	 * Built like shared/LanguageSelect.svelte.
	 */
	import DistroIcon from './DistroIcon.svelte';
	import { t } from '#lib/state/i18n.svelte.js';
	import { matchesQuery } from '#lib/utils/search.js';

	interface OsIconOption {
		value: string;
		label: string;
		/** The logo to show; none for "No icon". */
		osId?: string | null;
	}

	interface Props {
		options: OsIconOption[];
		value: string;
		disabled?: boolean;
	}

	let { options, value = $bindable(''), disabled = false }: Props = $props();

	let open = $state(false);
	let query = $state('');
	let active = $state(0);
	let rootEl: HTMLDivElement | undefined = $state();
	let inputEl: HTMLInputElement | undefined = $state();
	let current = $derived(options.find((o) => o.value === value) ?? options[0]);
	let matches = $derived(options.filter((o) => matchesQuery(query, [o.label, o.value])));

	function choose(v: string) {
		open = false;
		query = '';
		value = v;
	}

	function toggle() {
		if (disabled) return;
		open = !open;
		if (open) {
			query = '';
			active = Math.max(0, options.findIndex((o) => o.value === value));
			queueMicrotask(() => {
				inputEl?.focus();
				rootEl?.querySelector('.option.selected')?.scrollIntoView({ block: 'nearest' });
			});
		}
	}

	$effect(() => {
		if (active >= matches.length) active = Math.max(0, matches.length - 1);
	});

	function onKeydown(e: KeyboardEvent) {
		if (!open) return;
		if (e.key === 'Escape') {
			e.stopPropagation();
			open = false;
			return;
		}
		if (e.key === 'ArrowDown') {
			e.preventDefault();
			active = matches.length ? (active + 1) % matches.length : 0;
		} else if (e.key === 'ArrowUp') {
			e.preventDefault();
			active = matches.length ? (active - 1 + matches.length) % matches.length : 0;
		} else if (e.key === 'Enter') {
			e.preventDefault();
			const pick = matches[active];
			if (pick) choose(pick.value);
		}
	}

	$effect(() => {
		if (!open) return;
		function onClickOutside(e: MouseEvent) {
			if (rootEl && !rootEl.contains(e.target as Node)) open = false;
		}
		document.addEventListener('click', onClickOutside, true);
		return () => document.removeEventListener('click', onClickOutside, true);
	});
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="select" bind:this={rootEl} onkeydown={onKeydown}>
	<button
		type="button"
		class="trigger"
		class:open
		onclick={toggle}
		{disabled}
		aria-haspopup="listbox"
		aria-expanded={open}
		aria-label={t('session.os_icon')}
	>
		<span class="icon"><DistroIcon osId={current?.osId} size={16} /></span>
		<span class="label">{current?.label ?? ''}</span>
		<svg class="chevron" width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden="true">
			<path d="M3 4.5L6 7.5L9 4.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
		</svg>
	</button>

	{#if open}
		<div class="menu">
			<input
				bind:this={inputEl}
				bind:value={query}
				class="search"
				type="text"
				autocomplete="off"
				spellcheck="false"
				placeholder={t('session.os_search')}
				aria-label={t('session.os_search')}
			/>
			{#if matches.length === 0}
				<p class="no-results">{t('session.os_no_match')}</p>
			{/if}
			<ul class="options" role="listbox" aria-label={t('session.os_icon')}>
				{#each matches as o, i (o.value)}
					<li>
						<button
							type="button"
							class="option"
							class:selected={o.value === value}
							class:active={i === active}
							role="option"
							aria-selected={o.value === value}
							onclick={() => choose(o.value)}
							onmouseenter={() => (active = i)}
						>
							<span class="icon"><DistroIcon osId={o.osId} size={16} /></span>
							<span class="label">{o.label}</span>
							{#if o.value === value}
								<svg class="tick" width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
									<path d="M13.3 4.7L6.5 11.5L2.7 7.7" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" />
								</svg>
							{/if}
						</button>
					</li>
				{/each}
			</ul>
		</div>
	{/if}
</div>

<style>
	.select {
		position: relative;
		flex: 1;
		min-width: 0;
	}

	.trigger {
		display: flex;
		align-items: center;
		gap: 8px;
		width: 100%;
		padding: 6px 10px;
		background: var(--color-bg-primary);
		border: 1px solid var(--color-border-strong);
		border-radius: var(--radius-btn);
		color: var(--color-text-primary);
		font-family: var(--font-sans);
		font-size: 0.75rem;
		cursor: pointer;
		text-align: left;
	}

	.trigger:disabled {
		opacity: 0.6;
		cursor: default;
	}

	.trigger.open,
	.trigger:focus-visible {
		border-color: var(--color-accent);
		outline: none;
	}

	.icon {
		display: inline-flex;
		width: 16px;
		height: 16px;
		justify-content: center;
		align-items: center;
		flex-shrink: 0;
	}

	.label {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.chevron {
		flex-shrink: 0;
		color: var(--color-text-secondary);
		transition: transform var(--duration-default) var(--ease-default);
	}

	.trigger.open .chevron {
		transform: rotate(180deg);
	}

	.menu {
		position: absolute;
		z-index: 10;
		top: calc(100% + 4px);
		left: 0;
		right: 0;
		display: flex;
		flex-direction: column;
		max-height: 280px;
		padding: var(--space-1);
		background: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		box-shadow: var(--shadow-elevated);
	}

	.search {
		flex-shrink: 0;
		width: 100%;
		padding: 6px var(--space-2);
		margin-bottom: var(--space-1);
		font-size: 0.75rem;
		font-family: inherit;
		color: var(--color-text-primary);
		background: var(--color-surface-sunken);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-sm);
		outline: none;
	}

	.search:focus {
		border-color: var(--color-accent);
	}

	.search::placeholder {
		color: var(--color-text-tertiary);
	}

	.options {
		flex: 1;
		overflow-y: auto;
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.no-results {
		margin: 0;
		padding: var(--space-3) var(--space-2);
		font-size: var(--text-xs);
		color: var(--color-text-tertiary);
		text-align: center;
	}

	.option {
		display: flex;
		align-items: center;
		gap: 8px;
		width: 100%;
		padding: 6px var(--space-2);
		background: none;
		border: none;
		border-radius: var(--radius-sm);
		color: var(--color-text-primary);
		font-family: var(--font-sans);
		font-size: 0.75rem;
		cursor: pointer;
		text-align: left;
	}

	.option:hover,
	.option.active {
		background: var(--color-surface-hover);
	}

	.option.selected .label {
		color: var(--color-accent);
	}

	.tick {
		flex-shrink: 0;
		color: var(--color-accent);
	}
</style>
