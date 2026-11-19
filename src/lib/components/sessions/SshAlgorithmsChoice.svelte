<script lang="ts">
	/**
	 * SSH algorithms (Default, Compatible with old devices, Custom) and the
	 * agent and X11 forwarding switches, as a few clicks over the session's
	 * ssh_config lines. The lines stay the truth: this reads them and writes
	 * them back, and picking a weaker setting here is its approval.
	 */
	import { t } from '#lib/state/i18n.svelte.js';
	import {
		COMPATIBLE_LINES,
		algorithmMode,
		enabledIn,
		flagOn,
		pubkeySha1,
		setEnabled,
		setFlag,
		setPubkeySha1,
		withoutAlgorithmLines,
		type AlgorithmMode,
		type CatalogGroup
	} from '#lib/utils/sshAlgorithms.js';

	interface Props {
		/** The session's ssh_config text, one line each. */
		lines: string[];
		catalog: CatalogGroup[];
		/** Show agent and X11 forwarding (desktop). */
		forwarding?: boolean;
		disabled?: boolean;
		onchange: (lines: string[]) => void;
	}

	let { lines, catalog, forwarding = true, disabled = false, onchange }: Props = $props();

	// Custom can hold the same lines as another choice (or none yet), so
	// remember the pick until another one is made.
	let customPicked = $state(false);
	let mode = $derived.by((): AlgorithmMode => {
		return customPicked ? 'custom' : algorithmMode(lines);
	});

	function pick(next: AlgorithmMode): void {
		const was = mode;
		customPicked = next === 'custom';
		if (next === was) return;
		if (next === 'default') onchange(withoutAlgorithmLines(lines));
		else if (next === 'compatible') onchange([...withoutAlgorithmLines(lines), ...COMPATIBLE_LINES]);
		// Custom starts from what is in force now.
	}

	function toggle(group: CatalogGroup, name: string, on: boolean): void {
		const now = enabledIn(lines, group);
		const next = on ? [...now, name] : now.filter((n) => n !== name);
		if (next.length === 0) return;
		customPicked = true;
		onchange(setEnabled(lines, group, next));
	}

	const GROUP_LABEL: Record<string, string> = {
		KexAlgorithms: 'sshalg.kex',
		HostKeyAlgorithms: 'sshalg.host_keys',
		Ciphers: 'sshalg.ciphers',
		MACs: 'sshalg.macs'
	};
</script>

<div class="algos">
	<span class="label">{t('sshalg.title')}</span>
	<div class="seg" role="radiogroup" aria-label={t('sshalg.title')}>
		{#each [['default', 'sshalg.mode_default'], ['compatible', 'sshalg.mode_compatible'], ['custom', 'sshalg.mode_custom']] as [m, key] (m)}
			<button
				type="button"
				class="seg-btn"
				class:active={mode === m}
				role="radio"
				aria-checked={mode === m}
				{disabled}
				onclick={() => pick(m as AlgorithmMode)}
			>
				{t(key)}
			</button>
		{/each}
	</div>
	<p class="hint" class:warn={mode !== 'default'}>
		{#if mode === 'default'}
			{t('sshalg.hint_default')}
		{:else if mode === 'compatible'}
			{t('sshalg.hint_compatible')}
		{:else}
			{t('sshalg.hint_custom')}
		{/if}
	</p>

	{#if mode === 'custom'}
		{#if catalog.length === 0}
			<p class="hint">{t('sshopt.checking')}</p>
		{/if}
		{#each catalog as group (group.keyword)}
			{@const on = enabledIn(lines, group)}
			<fieldset class="group">
				<legend>{t(GROUP_LABEL[group.keyword] ?? group.keyword)}</legend>
				<div class="names">
					{#each group.algorithms as a (a.name)}
						<label class="name" title={a.weak ?? ''}>
							<input
								type="checkbox"
								checked={on.includes(a.name)}
								disabled={disabled || (on.length === 1 && on.includes(a.name))}
								onchange={(e) => toggle(group, a.name, e.currentTarget.checked)}
							/>
							<code>{a.name}</code>
							{#if a.weak}<span class="weak">{t('sshalg.legacy')}</span>{/if}
						</label>
					{/each}
				</div>
			</fieldset>
		{/each}
		<label class="name">
			<input type="checkbox" checked={pubkeySha1(lines)} {disabled} onchange={(e) => onchange(setPubkeySha1(lines, e.currentTarget.checked))} />
			<span>{t('sshalg.pubkey_sha1')}</span>
			<span class="weak">{t('sshalg.legacy')}</span>
		</label>
	{/if}

	{#if forwarding}
		<label class="switch">
			<input type="checkbox" checked={flagOn(lines, 'ForwardAgent')} {disabled} onchange={(e) => onchange(setFlag(lines, 'ForwardAgent', e.currentTarget.checked))} />
			<span class="switch-text">{t('sshalg.forward_agent')}</span>
		</label>
		<p class="hint">{t('sshalg.forward_agent_hint')}</p>
		<label class="switch">
			<input type="checkbox" checked={flagOn(lines, 'ForwardX11')} {disabled} onchange={(e) => onchange(setFlag(lines, 'ForwardX11', e.currentTarget.checked))} />
			<span class="switch-text">{t('sshalg.forward_x11')}</span>
		</label>
		<p class="hint">{t('sshalg.forward_x11_hint')}</p>
	{/if}
</div>

<style>
	.algos {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.label {
		font-size: 0.6875rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--color-text-secondary);
	}

	.seg {
		display: flex;
		gap: 2px;
		padding: 3px;
		background: var(--color-bg-primary);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
	}

	.seg-btn {
		flex: 1 1 auto;
		min-width: 0;
		padding: 6px 10px;
		font-family: var(--font-sans);
		font-size: 0.8125rem;
		font-weight: 500;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		border: none;
		border-radius: 6px;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
	}

	.seg-btn:hover:not(:disabled):not(.active) {
		color: var(--color-text-primary);
	}

	.seg-btn.active {
		background-color: var(--color-bg-elevated);
		color: var(--color-accent);
		box-shadow: 0 1px 2px rgba(0, 0, 0, 0.25), inset 0 0 0 1px var(--color-border);
	}

	.hint {
		margin: 0;
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
	}

	.hint.warn {
		color: var(--color-warning);
	}

	.group {
		margin: 0;
		padding: 6px 8px;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
	}

	legend {
		padding: 0 4px;
		font-size: 0.6875rem;
		font-weight: 600;
		color: var(--color-text-secondary);
	}

	.names {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
		gap: 2px 10px;
	}

	.name {
		display: flex;
		align-items: center;
		gap: 6px;
		min-width: 0;
		font-size: 0.75rem;
		color: var(--color-text-primary);
		cursor: pointer;
	}

	.name code {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-size: 0.6875rem;
	}

	.weak {
		flex-shrink: 0;
		padding: 0 5px;
		border-radius: 999px;
		font-size: 0.625rem;
		color: var(--color-warning);
		border: 1px solid color-mix(in srgb, var(--color-warning) 45%, var(--color-border));
	}

	.switch {
		display: flex;
		align-items: center;
		gap: 8px;
		margin-top: 4px;
		cursor: pointer;
	}

	.switch-text {
		font-size: 0.8125rem;
		font-weight: 500;
		color: var(--color-text-primary);
	}
</style>
