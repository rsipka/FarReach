<script lang="ts">
	import { openUrl } from '@tauri-apps/plugin-opener';
	import Modal from '#lib/components/shared/Modal.svelte';
	import Button from '#lib/components/shared/Button.svelte';
	import ReleaseNotes from '#lib/components/shared/ReleaseNotes.svelte';
	import { getUpdaterState, closeWhatsNew, downloadAndInstall } from '#lib/state/updater.svelte.js';
	import { t } from '#lib/state/i18n.svelte.js';
	import { __APP_VERSION__ } from '#lib/version.js';

	/** What's new in the update on offer: its version and the notes of its
	 *  release, opened from the update banner's Info button or from
	 *  Settings > About. The update itself stays one press away. */
	const updater = getUpdaterState();

	let open = $derived(updater.whatsNewOpen && updater.updateAvailable);
	let busy = $derived(updater.downloading || updater.installing);

	function update() {
		closeWhatsNew();
		void downloadAndInstall();
	}
</script>

<Modal {open} onclose={closeWhatsNew} title={t('updater.whats_new')} maxWidth="520px" zIndex={160}>
	<div class="versions">
		<span class="current">v{__APP_VERSION__}</span>
		<span class="arrow" aria-hidden="true">→</span>
		<span class="new">v{updater.updateVersion ?? ''}</span>
	</div>

	<div class="notes-box">
		{#if updater.updateNotes?.trim()}
			<ReleaseNotes notes={updater.updateNotes} />
		{:else}
			<p class="empty">{t('updater.no_notes')}</p>
		{/if}
	</div>

	<button class="release-link" onclick={() => void openUrl(updater.releasePageUrl)}>
		{t('updater.release_page')}
	</button>

	{#snippet actions()}
		<Button variant="secondary" onclick={closeWhatsNew}>{t('updater.later')}</Button>
		<Button variant="primary" disabled={busy} onclick={update}>
			{updater.isDownloadLink ? t('updater.download') : t('updater.update_now')}
		</Button>
	{/snippet}
</Modal>

<style>
	.versions {
		display: flex;
		align-items: center;
		gap: 8px;
		margin-bottom: 14px;
		font-size: 0.8125rem;
	}

	.current {
		color: var(--color-text-secondary);
	}

	.arrow {
		color: var(--color-text-tertiary, var(--color-text-secondary));
	}

	.new {
		padding: 2px 10px;
		border-radius: 20px;
		font-weight: 600;
		color: var(--color-accent);
		background: color-mix(in srgb, var(--color-accent) 12%, transparent);
	}

	.notes-box {
		max-height: min(50vh, 380px);
		overflow-y: auto;
		padding: 12px 14px;
		border-radius: 8px;
		background: var(--color-surface-hover);
		border: 1px solid var(--color-border);
	}

	.empty {
		margin: 0;
		font-size: 0.8125rem;
		color: var(--color-text-secondary);
	}

	.release-link {
		margin-top: 10px;
		padding: 0;
		border: none;
		background: none;
		font: inherit;
		font-size: 0.75rem;
		color: var(--color-accent);
		text-decoration: underline;
		text-underline-offset: 2px;
		cursor: pointer;
	}
</style>
