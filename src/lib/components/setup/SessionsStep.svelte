<script lang="ts">
	/**
	 * Setup step: how sessions open and close, and copy and paste in the
	 * terminal. Every answer is a setting that applies at once and stays
	 * changeable in Settings; the defaults are the same as there, so Next
	 * without touching anything changes nothing.
	 *
	 * The mouse questions (click to open, right and middle click, copy on
	 * select) are left out on a phone or tablet, which has no mouse: a tap
	 * always opens a session there.
	 */
	import Button from '#lib/components/shared/Button.svelte';
	import Dropdown from '#lib/components/shared/Dropdown.svelte';
	import Toggle from '#lib/components/shared/Toggle.svelte';
	import WizardRow from './WizardRow.svelte';
	import { isMobile } from '#lib/platform.js';
	import { t } from '#lib/state/i18n.svelte.js';
	import { getSettings, updateSetting } from '#lib/state/settings.svelte.js';
	import { rightClickMode } from '#lib/terminal/mouse.js';

	interface Props {
		onNext: () => void;
		onBack: () => void;
	}

	let { onNext, onBack }: Props = $props();

	const settings = getSettings();
	const mobile = isMobile();

	let rightClickOptions = $derived([
		{ label: t('settings.term_right_click_menu'), value: 'menu' },
		{ label: t('settings.term_right_click_paste'), value: 'paste' },
		{ label: t('settings.term_right_click_copy_paste'), value: 'copy-paste' }
	]);
</script>

<div class="step">
	<div class="step-header">
		<h2 class="step-title">{t('setup.sessions_title')}</h2>
		<p class="step-subtitle">{t('setup.sessions_subtitle')}</p>
	</div>

	<div class="group">
		{#if !mobile}
			<WizardRow label={t('setup.open_with')} hint={t('setup.open_hint')}>
				<div class="segmented" role="radiogroup" aria-label={t('setup.open_with')}>
					<button
						type="button"
						role="radio"
						aria-checked={!settings.doubleClickToOpen}
						class:on={!settings.doubleClickToOpen}
						onclick={() => updateSetting('doubleClickToOpen', false)}
					>{t('setup.open_single')}</button>
					<button
						type="button"
						role="radio"
						aria-checked={settings.doubleClickToOpen}
						class:on={settings.doubleClickToOpen}
						onclick={() => updateSetting('doubleClickToOpen', true)}
					>{t('setup.open_double')}</button>
				</div>
			</WizardRow>
		{/if}

		<WizardRow label={t('settings.close_tab_on_exit')} hint={t('setup.close_tab_hint')}>
			<Toggle
				hideLabel
				label={t('settings.close_tab_on_exit')}
				checked={settings.closeTabOnExit}
				onchange={(v) => updateSetting('closeTabOnExit', v)}
			/>
		</WizardRow>

		<WizardRow label={t('settings.server_monitoring')} hint={t('setup.monitoring_hint')}>
			<Toggle
				hideLabel
				label={t('settings.server_monitoring')}
				checked={settings.serverMonitoring}
				onchange={(v) => updateSetting('serverMonitoring', v)}
			/>
		</WizardRow>
	</div>

	<div class="group">
		<span class="group-label">{t('setup.copy_paste')}</span>

		{#if !mobile}
			<WizardRow label={t('settings.term_right_click')}>
				<Dropdown
					options={rightClickOptions}
					selected={settings.terminalRightClick}
					onchange={(v) => updateSetting('terminalRightClick', rightClickMode(v))}
				/>
			</WizardRow>

			<WizardRow label={t('settings.term_copy_on_select')} hint={t('settings.term_copy_on_select_desc')}>
				<Toggle
					hideLabel
					label={t('settings.term_copy_on_select')}
					checked={settings.terminalCopyOnSelect}
					onchange={(v) => updateSetting('terminalCopyOnSelect', v)}
				/>
			</WizardRow>

			<WizardRow label={t('settings.term_middle_click')} hint={t('settings.term_middle_click_desc')}>
				<Toggle
					hideLabel
					label={t('settings.term_middle_click')}
					checked={settings.terminalMiddleClickPaste}
					onchange={(v) => updateSetting('terminalMiddleClickPaste', v)}
				/>
			</WizardRow>
		{/if}

		<WizardRow label={t('settings.paste_warning')} hint={t('setup.paste_warning_hint')}>
			<Toggle
				hideLabel
				label={t('settings.paste_warning')}
				checked={settings.warnOnMultilinePaste}
				onchange={(v) => updateSetting('warnOnMultilinePaste', v)}
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

	.group-label {
		padding-top: var(--space-2);
		font-size: var(--text-2xs);
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.04em;
		color: var(--color-text-tertiary);
	}

	.segmented {
		display: inline-flex;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		overflow: hidden;
	}

	.segmented button {
		padding: 5px 10px;
		font-family: inherit;
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
		background: transparent;
		border: none;
		cursor: pointer;
		white-space: nowrap;
	}

	.segmented button + button {
		border-left: 1px solid var(--color-border);
	}

	.segmented button.on {
		color: #fff;
		background: var(--color-accent-fill);
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
