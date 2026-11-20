<script lang="ts" module>
	/** One other address as the editor holds it: the port as typed. */
	export interface AddressRow {
		label: string;
		host: string;
		port: string;
		direct: boolean;
	}
</script>

<script lang="ts">
	/**
	 * The session editor's Addresses: a name for the main address, the other
	 * addresses the same server answers on in priority order, and how a
	 * connect picks one (see #lib/sessions/addresses).
	 */
	import { t } from '#lib/state/i18n.svelte.js';
	import Disclosure from '#lib/components/shared/Disclosure.svelte';
	import type { ConnectMode } from '#lib/ipc/sessions.js';

	interface Props {
		mainLabel: string;
		rows: AddressRow[];
		mode: ConnectMode;
		/** The session goes through jump hosts or a proxy: addresses can skip them. */
		routed: boolean;
		port: string;
		disabled?: boolean;
	}

	let { mainLabel = $bindable(), rows = $bindable(), mode = $bindable(), routed, port, disabled = false }: Props = $props();

	// Folded until there is something in it; open once addresses are read
	// in (the editor fills its form after this section is made).
	let open = $state(false);
	let unfolded = false;
	$effect(() => {
		if (!unfolded && rows.length > 0) {
			unfolded = true;
			open = true;
		}
	});

	function add(): void {
		rows = [...rows, { label: '', host: '', port: '', direct: false }];
		open = true;
	}

	function remove(i: number): void {
		rows = rows.filter((_, j) => j !== i);
	}

	function move(i: number, by: number): void {
		const j = i + by;
		if (j < 0 || j >= rows.length) return;
		const next = [...rows];
		[next[i], next[j]] = [next[j], next[i]];
		rows = next;
	}
</script>

<Disclosure bind:open title={t('session.addresses')} summary={rows.length ? t('session.addresses_n', { count: String(rows.length + 1) }) : t('session.addresses_none')} hint={t('session.addresses_hint')}>
	<div class="addresses">
		{#if rows.length}
			<label class="row main-row">
				<span class="prio">1</span>
				<input class="label-in" type="text" bind:value={mainLabel} placeholder={t('session.address_label_main')} {disabled} />
				<span class="main-note">{t('session.address_main')}</span>
			</label>
		{/if}
		{#each rows as row, i (i)}
			<div class="row">
				<span class="prio">{i + 2}</span>
				<input class="label-in" type="text" bind:value={row.label} placeholder={t('session.address_label')} aria-label={t('session.address_label')} {disabled} />
				<input class="host-in" type="text" bind:value={row.host} placeholder="server.example.com" aria-label={t('session.host')} {disabled} />
				<input class="port-in" type="number" bind:value={row.port} placeholder={port || '22'} aria-label={t('session.port')} {disabled} />
				{#if routed}
					<label class="direct" title={t('session.address_direct_hint')}>
						<input type="checkbox" bind:checked={row.direct} {disabled} />
						{t('session.address_direct')}
					</label>
				{/if}
				<span class="tools">
					<button type="button" class="icon" onclick={() => move(i, -1)} disabled={disabled || i === 0} title={t('session.move_up')} aria-label={t('session.move_up')}>↑</button>
					<button type="button" class="icon" onclick={() => move(i, 1)} disabled={disabled || i === rows.length - 1} title={t('session.move_down')} aria-label={t('session.move_down')}>↓</button>
					<button type="button" class="icon" onclick={() => remove(i)} {disabled} title={t('session.address_remove')} aria-label={t('session.address_remove')}>✕</button>
				</span>
			</div>
		{/each}
		<button type="button" class="link-btn" onclick={add} {disabled}>+ {t('session.address_add')}</button>

		{#if rows.length}
			<div class="mode">
				<label class="mode-label" for="connect-mode">{t('session.connect_mode')}</label>
				<select id="connect-mode" bind:value={mode} {disabled}>
					<option value="auto">{t('session.connect_mode_auto')}</option>
					<option value="ask">{t('session.connect_mode_ask')}</option>
					<option value="main">{t('session.connect_mode_main')}</option>
				</select>
				<p class="hint">{mode === 'auto' ? t('session.connect_mode_auto_hint') : mode === 'ask' ? t('session.connect_mode_ask_hint') : t('session.connect_mode_main_hint')}</p>
			</div>
		{/if}
	</div>
</Disclosure>

<style>
	.addresses {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.hint {
		margin: 0;
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
	}

	.row {
		display: flex;
		align-items: center;
		gap: 6px;
	}

	.prio {
		width: 16px;
		text-align: center;
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
		flex-shrink: 0;
	}

	input[type='text'],
	input[type='number'],
	select {
		min-width: 0;
		padding: 5px 8px;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-sm, 4px);
		background: var(--color-bg-input, var(--color-bg-secondary));
		color: var(--color-text-primary);
		font: inherit;
		font-size: var(--text-sm);
	}

	.label-in {
		flex: 1 1 70px;
		max-width: 120px;
	}

	.host-in {
		flex: 3 1 100px;
		font-family: var(--font-mono);
	}

	.port-in {
		flex: 0 0 64px;
		width: 64px;
	}

	.main-note {
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
	}

	.direct {
		display: flex;
		align-items: center;
		gap: 4px;
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
		white-space: nowrap;
	}

	.tools {
		display: flex;
		gap: 0;
		margin-left: auto;
		flex-shrink: 0;
	}

	.icon {
		width: 22px;
		height: 24px;
		border: none;
		border-radius: var(--radius-sm, 4px);
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
	}

	.icon:hover:not(:disabled) {
		background: var(--color-bg-hover, rgba(127, 127, 127, 0.12));
		color: var(--color-text-primary);
	}

	.icon:disabled {
		opacity: 0.3;
		cursor: default;
	}

	.link-btn {
		align-self: flex-start;
		padding: 2px 0;
		border: none;
		background: none;
		color: var(--color-accent);
		font: inherit;
		font-size: var(--text-sm);
		cursor: pointer;
	}

	.mode {
		display: flex;
		flex-direction: column;
		gap: 4px;
		margin-top: 4px;
	}

	.mode-label {
		font-size: var(--text-sm);
		font-weight: 500;
		color: var(--color-text-primary);
	}
</style>
