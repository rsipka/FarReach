<script lang="ts">
	/**
	 * Setup step: locking, key passphrases and update checks. The same rows as
	 * Settings > Security and Settings > About, with the same defaults, so
	 * Next without touching anything changes nothing. Update checks are here
	 * because they are the one thing the app sends out on its own.
	 */
	import Button from '#lib/components/shared/Button.svelte';
	import Dropdown from '#lib/components/shared/Dropdown.svelte';
	import Toggle from '#lib/components/shared/Toggle.svelte';
	import WizardRow from './WizardRow.svelte';
	import { t } from '#lib/state/i18n.svelte.js';
	import { getSettings, updateSetting } from '#lib/state/settings.svelte.js';

	interface Props {
		onNext: () => void;
		onBack: () => void;
	}

	let { onNext, onBack }: Props = $props();

	const settings = getSettings();

	const autoLockOptions = $derived([
		{ label: t('security.auto_lock_never'), value: '0' },
		...[1, 5, 15, 30, 60].map((m) => ({ label: t('security.auto_lock_minutes', { count: m }), value: String(m) }))
	]);
</script>

<div class="step">
	<div class="step-header">
		<h2 class="step-title">{t('setup.security_title')}</h2>
		<p class="step-subtitle">{t('setup.security_subtitle')}</p>
	</div>

	<div class="group">
		<WizardRow label={t('security.auto_lock')} hint={t('setup.auto_lock_hint')}>
			<Dropdown
				options={autoLockOptions}
				selected={String(settings.autoLockMinutes)}
				onchange={(value) => updateSetting('autoLockMinutes', Number(value))}
			/>
		</WizardRow>

		<WizardRow label={t('security.lock_on_system_lock')}>
			<Toggle
				hideLabel
				label={t('security.lock_on_system_lock')}
				checked={settings.lockOnSystemLock}
				onchange={(v) => updateSetting('lockOnSystemLock', v)}
			/>
		</WizardRow>

		<WizardRow label={t('security.unlock_keys_at_start')} hint={t('setup.unlock_keys_hint')}>
			<Toggle
				hideLabel
				label={t('security.unlock_keys_at_start')}
				checked={settings.unlockKeysAtStart}
				onchange={(v) => updateSetting('unlockKeysAtStart', v)}
			/>
		</WizardRow>
	</div>

	<div class="group">
		<WizardRow label={t('settings.check_updates')} hint={t('setup.check_updates_hint')}>
			<Toggle
				hideLabel
				label={t('settings.check_updates')}
				checked={settings.checkForUpdates}
				onchange={(v) => updateSetting('checkForUpdates', v)}
			/>
		</WizardRow>
	</div>

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
