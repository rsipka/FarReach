<script lang="ts">
	/**
	 * Setup step: the file log and the session history. Both stay off unless
	 * turned on here, as in Settings, because both keep what the server
	 * printed. Only the choices people actually make up front are asked;
	 * the file name pattern, append and the masking patterns stay in Settings.
	 *
	 * History is desktop only for now: a phone has no encrypted store for it
	 * (src-tauri/src/history/store.rs), so it is not offered there. The log
	 * folder is fixed on a phone, so only the mode is asked.
	 */
	import { onMount } from 'svelte';
	import { open as pickFolder } from '@tauri-apps/plugin-dialog';
	import Button from '#lib/components/shared/Button.svelte';
	import WizardRow from './WizardRow.svelte';
	import { isMobile } from '#lib/platform.js';
	import { t } from '#lib/state/i18n.svelte.js';
	import { getSettings, updateSetting } from '#lib/state/settings.svelte.js';
	import { pushHistoryRetention } from '#lib/state/history.svelte.js';
	import { sessionLogDefaultDir } from '#lib/ipc/ssh.js';

	interface Props {
		onNext: () => void;
		onBack: () => void;
	}

	let { onNext, onBack }: Props = $props();

	const settings = getSettings();
	const mobile = isMobile();

	let logDefaultDir = $state('');

	onMount(() => {
		sessionLogDefaultDir().then((d) => (logDefaultDir = d)).catch(() => {});
	});

	async function browseLogFolder(): Promise<void> {
		try {
			const dir = await pickFolder({ directory: true, defaultPath: settings.sessionLogFolder || logDefaultDir || undefined });
			if (typeof dir === 'string') updateSetting('sessionLogFolder', dir);
		} catch {
			// No picker (or it was refused): the field can still be typed in.
		}
	}

	function days(value: string): number {
		return Math.max(0, Math.floor(Number(value) || 0));
	}

	function setRetention(key: 'historyOutputDays' | 'historyCommandDays', value: string): void {
		updateSetting(key, days(value));
		pushHistoryRetention();
	}
</script>

<div class="step">
	<div class="step-header">
		<h2 class="step-title">{t('setup.logging_title')}</h2>
		<p class="step-subtitle">{t('setup.logging_subtitle')}</p>
	</div>

	<div class="group">
		<WizardRow label={t('settings.log_mode')} hint={t('setup.log_hint')}>
			<select
				class="field"
				aria-label={t('settings.log_mode')}
				value={settings.sessionLogMode}
				onchange={(e) => updateSetting('sessionLogMode', e.currentTarget.value as 'off' | 'printable' | 'all')}
			>
				<option value="off">{t('settings.log_off')}</option>
				<option value="printable">{t('settings.log_printable')}</option>
				<option value="all">{t('settings.log_all')}</option>
			</select>
		</WizardRow>

		{#if settings.sessionLogMode !== 'off'}
			{#if mobile}
				{#if logDefaultDir}
					<p class="note sub">{t('settings.log_folder')}: {logDefaultDir}</p>
				{/if}
			{:else}
				<WizardRow sub label={t('settings.log_folder')}>
					<input
						class="field folder"
						type="text"
						spellcheck="false"
						placeholder={logDefaultDir}
						title={settings.sessionLogFolder || logDefaultDir}
						aria-label={t('settings.log_folder')}
						value={settings.sessionLogFolder}
						onchange={(e) => updateSetting('sessionLogFolder', e.currentTarget.value.trim())}
					/>
					<button class="browse" type="button" onclick={browseLogFolder}>{t('settings.log_browse')}</button>
				</WizardRow>
			{/if}
		{/if}
	</div>

	{#if !mobile}
		<div class="group">
			<WizardRow label={t('history.settings_mode')} hint={t('setup.history_hint')}>
				<select
					class="field"
					aria-label={t('history.settings_mode')}
					value={settings.historyMode}
					onchange={(e) => updateSetting('historyMode', e.currentTarget.value as 'off' | 'commands' | 'output')}
				>
					<option value="off">{t('history.mode_off')}</option>
					<option value="commands">{t('history.mode_commands')}</option>
					<option value="output">{t('history.mode_output')}</option>
				</select>
			</WizardRow>

			{#if settings.historyMode !== 'off'}
				<WizardRow sub label={t('history.settings_keep')} hint={t('setup.history_keep_hint')}>
					<label class="num-label">
						{t('history.settings_command_days')}
						<input class="field num" type="number" min="0" value={settings.historyCommandDays}
							onchange={(e) => setRetention('historyCommandDays', e.currentTarget.value)} />
					</label>
					{#if settings.historyMode === 'output'}
						<label class="num-label">
							{t('history.settings_output_days')}
							<input class="field num" type="number" min="0" value={settings.historyOutputDays}
								onchange={(e) => setRetention('historyOutputDays', e.currentTarget.value)} />
						</label>
					{/if}
				</WizardRow>
			{/if}
		</div>
	{/if}

	<p class="note">{t('setup.logging_per_session')}</p>

	<div class="step-actions">
		<Button variant="ghost" size="md" onclick={onBack}>{t('setup.back')}</Button>
		<Button variant="primary" size="md" onclick={onNext}>{t('setup.next')}</Button>
	</div>
</div>

<style>
	.step {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-4);
		width: 100%;
	}

	.step-header {
		text-align: center;
	}

	.step-title {
		margin: 0;
		font-size: var(--text-lg);
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.step-subtitle {
		margin: 6px 0 0;
		font-size: var(--text-sm);
		color: var(--color-text-secondary);
	}

	.group {
		display: flex;
		flex-direction: column;
		width: 100%;
		padding: var(--space-1) var(--space-3);
		background: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-card);
	}

	.field {
		padding: 5px var(--space-2);
		font-family: inherit;
		font-size: var(--text-xs);
		color: var(--color-text-primary);
		background: var(--color-surface-sunken);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-sm);
		outline: none;
	}

	.field:focus {
		border-color: var(--color-accent);
	}

	.field.folder {
		width: 170px;
	}

	.field.num {
		width: 64px;
	}

	.num-label {
		display: flex;
		flex-direction: column;
		gap: 2px;
		font-size: var(--text-2xs);
		color: var(--color-text-tertiary);
	}

	.browse {
		padding: 5px var(--space-2);
		font-family: inherit;
		font-size: var(--text-xs);
		color: var(--color-text-primary);
		background: var(--color-bg-secondary);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-sm);
		cursor: pointer;
	}

	.browse:hover {
		background: var(--color-surface-hover);
	}

	.note {
		margin: 0;
		font-size: var(--text-xs);
		color: var(--color-text-tertiary);
		text-align: center;
		overflow-wrap: anywhere;
	}

	.note.sub {
		text-align: left;
		padding: 0 0 var(--space-2) var(--space-4);
	}

	.step-actions {
		display: flex;
		justify-content: center;
		gap: var(--space-3);
		width: 100%;
		position: sticky;
		bottom: 0;
		padding-top: var(--space-3);
		background: linear-gradient(to bottom, transparent, var(--color-bg-secondary) 40%);
	}
</style>
