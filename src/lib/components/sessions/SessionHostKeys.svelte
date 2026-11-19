<!--
	The server keys a saved SSH session trusts, one per server it goes
	through (its jump hosts have their own). They are saved by connecting
	(Accept and save) and kept with the session, so they sync and are shared
	with it. Each can be removed (the next connect asks again) or replaced
	by a fingerprint known before connecting. Changes are written at once,
	not with the editor's Save.
-->
<script lang="ts">
	import { onMount } from 'svelte';
	import Button from '#lib/components/shared/Button.svelte';
	import Disclosure from '#lib/components/shared/Disclosure.svelte';
	import Input from '#lib/components/shared/Input.svelte';
	import { sessionGet, sessionHostKeyRemove, sessionHostKeyReplace, type SavedHostKey } from '#lib/ipc/sessions.js';
	import { t } from '#lib/state/i18n.svelte.js';

	interface Props {
		sessionId: string;
		/** As the session was read when the editor opened. */
		initial?: SavedHostKey[] | null;
		disabled?: boolean;
	}

	let { sessionId, initial = null, disabled = false }: Props = $props();

	// svelte-ignore state_referenced_locally
	let keys = $state<SavedHostKey[]>(initial ?? []);
	let open = $state(false);
	let busy = $state(false);
	let error = $state('');
	/** The server whose key is being replaced, and what was typed. */
	let replacing = $state<string | null>(null);
	let typed = $state('');

	let summary = $derived(keys.length === 0 ? t('hostkeys.none') : t('hostkeys.count', { count: keys.length }));

	// A connect may save a key while the editor is open.
	onMount(() => {
		const reload = (e: Event) => {
			const id = (e as CustomEvent<{ hostKeys?: string } | undefined>).detail?.hostKeys;
			if (id && id !== sessionId) return;
			sessionGet(sessionId)
				.then((s) => (keys = s.host_keys ?? []))
				.catch(() => {});
		};
		window.addEventListener('reach:sessions-changed', reload);
		return () => window.removeEventListener('reach:sessions-changed', reload);
	});

	async function run(change: () => Promise<SavedHostKey[]>): Promise<boolean> {
		busy = true;
		error = '';
		try {
			keys = await change();
			window.dispatchEvent(new CustomEvent('reach:sessions-changed', { detail: { hostKeys: sessionId } }));
			return true;
		} catch (e) {
			error = String(e);
			return false;
		} finally {
			busy = false;
		}
	}

	function remove(host: string | null): void {
		void run(() => sessionHostKeyRemove(sessionId, host));
	}

	async function replace(host: string): Promise<void> {
		if (await run(() => sessionHostKeyReplace(sessionId, host, typed))) {
			replacing = null;
			typed = '';
		}
	}

	function savedOn(k: SavedHostKey): string {
		return k.saved_at ? new Date(k.saved_at * 1000).toLocaleDateString() : '';
	}
</script>

<Disclosure title={t('hostkeys.title')} {summary} hint={t('hostkeys.hint')} bind:open>
	{#if keys.length === 0}
		<p class="empty">{t('hostkeys.empty')}</p>
	{:else}
		<ul class="keys">
			{#each keys as k (k.host)}
				<li class="key">
					<div class="head">
						<span class="host mono">{k.host}</span>
						<span class="meta">
							{k.key_type ? k.key_type : t('hostkeys.typed_in')}{#if savedOn(k)} · {savedOn(k)}{/if}
						</span>
					</div>
					<span class="fp mono">{k.fingerprint}</span>
					{#if replacing === k.host}
						<div class="replace">
							<Input label={t('hostkeys.new_fingerprint')} bind:value={typed} placeholder="SHA256:…" disabled={busy || disabled} />
							<div class="actions">
								<Button variant="ghost" size="sm" disabled={busy} onclick={() => { replacing = null; typed = ''; error = ''; }}>
									{t('common.cancel')}
								</Button>
								<Button size="sm" disabled={busy || disabled || !typed.trim()} onclick={() => replace(k.host)}>
									{t('hostkeys.replace_save')}
								</Button>
							</div>
							<p class="note">{t('hostkeys.replace_hint')}</p>
						</div>
					{:else}
						<div class="actions">
							<Button variant="secondary" size="sm" disabled={busy || disabled} onclick={() => { replacing = k.host; typed = ''; error = ''; }}>
								{t('hostkeys.replace')}
							</Button>
							<Button variant="danger" size="sm" disabled={busy || disabled} onclick={() => remove(k.host)}>
								{t('hostkeys.remove')}
							</Button>
						</div>
					{/if}
				</li>
			{/each}
		</ul>
		{#if keys.length > 1}
			<div class="actions all">
				<Button variant="ghost" size="sm" disabled={busy || disabled} onclick={() => remove(null)}>
					{t('hostkeys.remove_all')}
				</Button>
			</div>
		{/if}
	{/if}
	{#if error}
		<p class="error">{error}</p>
	{/if}
</Disclosure>

<style>
	.empty,
	.note {
		margin: 0;
		font-size: 0.8125rem;
		color: var(--color-text-secondary);
	}

	.keys {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 10px;
	}

	.key {
		display: flex;
		flex-direction: column;
		gap: 6px;
		padding: 10px 12px;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
	}

	.head {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		justify-content: space-between;
		gap: 4px 12px;
	}

	.host {
		font-size: 0.8125rem;
		color: var(--color-text-primary);
		word-break: break-all;
	}

	.meta {
		font-size: 0.75rem;
		color: var(--color-text-secondary);
	}

	.fp {
		font-size: 0.75rem;
		color: var(--color-text-secondary);
		word-break: break-all;
	}

	.mono {
		font-family: var(--font-mono, monospace);
	}

	.actions {
		display: flex;
		flex-wrap: wrap;
		justify-content: flex-end;
		gap: 8px;
	}

	.all {
		margin-top: 10px;
	}

	.replace {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.error {
		margin: 8px 0 0;
		font-size: 0.8125rem;
		color: var(--color-danger);
	}
</style>
