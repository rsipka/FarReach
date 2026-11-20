<script lang="ts">
	/**
	 * A web session's window met a certificate the system does not trust
	 * (a device's self-signed one) and the session does not trust either.
	 * The backend did not load the page and asks here, in the app's own
	 * window, never in the web window: the page there cannot answer for the
	 * user. Trusting keeps the fingerprint on the session and loads the page
	 * again; a later, different certificate is asked about again, with a
	 * warning. Mounted once, in the app shell of the main window.
	 */
	import { onMount, onDestroy } from 'svelte';
	import { listen, type UnlistenFn } from '@tauri-apps/api/event';
	import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
	import Modal from '#lib/components/shared/Modal.svelte';
	import Button from '#lib/components/shared/Button.svelte';
	import { t } from '#lib/state/i18n.svelte.js';
	import { addToast } from '#lib/state/toasts.svelte.js';
	import { webSessionCertAnswer, type WebCertAsk } from '#lib/ipc/sessions.js';

	let queue = $state<WebCertAsk[]>([]);
	let current = $derived(queue[0]);
	let sending = $state(false);

	let unlisten: UnlistenFn | undefined;
	onMount(async () => {
		if (getCurrentWebviewWindow().label !== 'main') return;
		unlisten = await listen<WebCertAsk>('web-cert-ask', (e) => {
			const a = e.payload;
			// One question per window: a newer one replaces it.
			queue = [...queue.filter((q) => q.label !== a.label), a];
		});
	});
	onDestroy(() => unlisten?.());

	async function answer(trust: boolean): Promise<void> {
		const a = current;
		if (!a || sending) return;
		sending = true;
		try {
			await webSessionCertAnswer(a.label, a.fingerprint, trust);
		} catch (e) {
			// The window closed meanwhile, or another answer came first.
			if (trust) addToast(String(e), 'error');
		} finally {
			queue = queue.filter((q) => q !== a);
			sending = false;
		}
	}
</script>

{#if current}
	<Modal open={true} onclose={() => answer(false)} zIndex={1100} title={t('web.cert_title')} maxWidth="520px">
		<div class="body">
			<p class="msg">{t('web.cert_body', { host: current.host, session: current.sessionName })}</p>
			{#if current.changed}
				<p class="msg warn" role="alert">{t('web.cert_changed')}</p>
			{/if}
			<code class="fp">SHA-256 {current.fingerprint}</code>
		</div>
		{#snippet actions()}
			<Button variant="secondary" onclick={() => answer(false)} disabled={sending}>{t('web.cert_cancel')}</Button>
			<Button variant={current?.changed ? 'danger' : 'primary'} onclick={() => answer(true)} disabled={sending}>{t('web.cert_trust')}</Button>
		{/snippet}
	</Modal>
{/if}

<style>
	.body {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.msg {
		margin: 0;
		font-size: 0.85rem;
		line-height: 1.45;
		color: var(--color-text-primary);
	}

	.warn {
		color: var(--color-danger);
		font-weight: 600;
	}

	.fp {
		display: block;
		padding: 8px 10px;
		border-radius: var(--radius-sm, 6px);
		background: var(--color-bg-secondary);
		font-size: 0.75rem;
		line-height: 1.5;
		word-break: break-all;
		user-select: all;
	}
</style>
