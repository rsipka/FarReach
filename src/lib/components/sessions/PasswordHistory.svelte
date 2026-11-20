<!--
	The passwords a session or saved credential used before, under its
	password field: each one hidden until its eye is pressed, with Use (puts
	it back in the field; it is saved with the rest of the form) and Copy.
	Read only when opened, so old passwords never sit in the session list.
	Shows nothing when there is no history.
-->
<script lang="ts">
	import { writeText } from '@tauri-apps/plugin-clipboard-manager';
	import { t } from '#lib/state/i18n.svelte.js';
	import { formatDate } from '#lib/utils/formatters.js';
	import RevealButton from '#lib/components/shared/RevealButton.svelte';
	import { passwordHistory, passwordHistoryClear, type HistoryOwner, type PastPassword } from '#lib/ipc/passwordHistory.js';

	interface Props {
		owner: HistoryOwner;
		/** Put an old password back in the form's password field. */
		onrestore: (password: string) => void;
		disabled?: boolean;
	}

	let { owner, onrestore, disabled = false }: Props = $props();

	let entries = $state<PastPassword[]>([]);
	let shown = $state<boolean[]>([]);
	let expanded = $state(false);
	let confirmClear = $state(false);
	let restored = $state<number | null>(null);
	let copied = $state<number | null>(null);
	let error = $state('');

	$effect(() => {
		const o = owner;
		entries = [];
		expanded = false;
		passwordHistory(o)
			.then((list) => {
				entries = list;
				shown = list.map(() => false);
			})
			.catch(() => (entries = []));
	});

	function restore(i: number): void {
		onrestore(entries[i].password);
		restored = i;
	}

	async function copy(i: number): Promise<void> {
		try {
			await writeText(entries[i].password);
			copied = i;
			setTimeout(() => {
				if (copied === i) copied = null;
			}, 1500);
		} catch (e) {
			error = String(e);
		}
	}

	async function clear(): Promise<void> {
		try {
			await passwordHistoryClear(owner);
			entries = [];
			confirmClear = false;
		} catch (e) {
			error = String(e);
		}
	}
</script>

{#if entries.length > 0}
	<div class="pw-history">
		<button type="button" class="pw-history-toggle" aria-expanded={expanded} onclick={() => (expanded = !expanded)}>
			<span class="chevron" class:open={expanded} aria-hidden="true">›</span>
			{t('password_history.title', { count: String(entries.length) })}
		</button>
		{#if expanded}
			<ul class="pw-history-list">
				{#each entries as entry, i (entry.replaced_at + ':' + i)}
					<li class="pw-history-row">
						<span class="pw-when" title={t('password_history.replaced_at')}>{formatDate(entry.replaced_at)}</span>
						<span class="pw-value-wrap">
							<input class="pw-value" type={shown[i] ? 'text' : 'password'} value={entry.password} readonly aria-label={t('password_history.old_password')} />
							<RevealButton bind:shown={shown[i]} />
						</span>
						<button type="button" class="pw-action" {disabled} onclick={() => void copy(i)}>{copied === i ? t('password_history.copied') : t('password_history.copy')}</button>
						<button type="button" class="pw-action" {disabled} onclick={() => restore(i)}>{restored === i ? t('password_history.restored') : t('password_history.use')}</button>
					</li>
				{/each}
			</ul>
			<p class="pw-hint">{t('password_history.hint')}</p>
			<div class="pw-clear">
				{#if confirmClear}
					<span>{t('password_history.clear_confirm')}</span>
					<button type="button" class="pw-action danger" {disabled} onclick={() => void clear()}>{t('password_history.clear')}</button>
					<button type="button" class="pw-action" onclick={() => (confirmClear = false)}>{t('common.cancel')}</button>
				{:else}
					<button type="button" class="pw-action" {disabled} onclick={() => (confirmClear = true)}>{t('password_history.clear')}</button>
				{/if}
			</div>
			{#if error}<p class="pw-error">{error}</p>{/if}
		{/if}
	</div>
{/if}

<style>
	.pw-history {
		margin-top: 4px;
		font-size: 0.75rem;
	}

	.pw-history-toggle {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		padding: 2px 0;
		border: none;
		background: none;
		color: var(--color-text-secondary);
		font-size: 0.75rem;
		cursor: pointer;
	}

	.pw-history-toggle:hover {
		color: var(--color-text-primary);
	}

	.chevron {
		display: inline-block;
		transition: transform 0.15s;
	}

	.chevron.open {
		transform: rotate(90deg);
	}

	.pw-history-list {
		list-style: none;
		margin: 6px 0 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.pw-history-row {
		display: flex;
		align-items: center;
		gap: 6px;
	}

	.pw-when {
		flex: 0 0 auto;
		min-width: 9.5em;
		color: var(--color-text-secondary);
	}

	.pw-value-wrap {
		position: relative;
		flex: 1 1 auto;
		min-width: 0;
	}

	.pw-value {
		width: 100%;
		box-sizing: border-box;
		padding: 4px 34px 4px 8px;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn, 6px);
		background: var(--color-bg-secondary, transparent);
		color: var(--color-text-primary);
		font-family: var(--font-mono, monospace);
		font-size: 0.75rem;
	}

	.pw-action {
		flex: 0 0 auto;
		padding: 3px 8px;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn, 6px);
		background: transparent;
		color: var(--color-text-primary);
		font-size: 0.75rem;
		cursor: pointer;
	}

	.pw-action:hover:not(:disabled) {
		background: var(--color-bg-hover, rgba(127, 127, 127, 0.12));
	}

	.pw-action.danger {
		color: var(--color-danger, #e5484d);
		border-color: var(--color-danger, #e5484d);
	}

	.pw-hint {
		margin: 6px 0 0;
		color: var(--color-text-secondary);
	}

	.pw-clear {
		display: flex;
		align-items: center;
		gap: 6px;
		margin-top: 6px;
	}

	.pw-error {
		margin: 4px 0 0;
		color: var(--color-danger, #e5484d);
	}

	@media (max-width: 480px) {
		.pw-history-row {
			flex-wrap: wrap;
		}

		.pw-when {
			min-width: 100%;
		}
	}
</style>
