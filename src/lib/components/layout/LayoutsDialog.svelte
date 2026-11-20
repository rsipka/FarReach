<script lang="ts">
	/**
	 * Saved split layouts: save the split on screen under a name, open or
	 * delete one saved before (see #lib/state/layouts).
	 */
	import Modal from '#lib/components/shared/Modal.svelte';
	import Button from '#lib/components/shared/Button.svelte';
	import Input from '#lib/components/shared/Input.svelte';
	import { t } from '#lib/state/i18n.svelte.js';
	import { addToast } from '#lib/state/toasts.svelte.js';
	import { getLayouts, canSaveLayout, saveLayout, deleteLayout, openLayout } from '#lib/state/layouts.svelte.js';
	import { savedLeaves } from '#lib/terminal/layouts.js';

	interface Props {
		open: boolean;
		onclose: () => void;
	}

	let { open, onclose }: Props = $props();

	let name = $state('');
	let layouts = $derived(getLayouts());
	let canSave = $derived(open && canSaveLayout());
	let replacing = $derived(layouts.some((l) => l.name.toLowerCase() === name.trim().toLowerCase()));

	function save(): void {
		if (!canSave || !name.trim()) return;
		if (saveLayout(name)) {
			addToast(t('layouts.saved', { name: name.trim() }), 'success');
			name = '';
		}
	}

	function panes(id: string): string {
		const l = layouts.find((x) => x.id === id);
		if (!l) return '';
		return savedLeaves(l.tree)
			.map((leaf) => ('session' in leaf ? (leaf.name ?? t('layouts.session')) : leaf.wslDistro ? `WSL ${leaf.wslDistro}` : t('layouts.local')))
			.join(' · ');
	}
</script>

<Modal {open} {onclose} title={t('layouts.title')} maxWidth="480px">
	<section class="save">
		{#if canSave}
			<form
				class="row"
				onsubmit={(e) => {
					e.preventDefault();
					save();
				}}
			>
				<div class="grow"><Input label={t('layouts.save_label')} placeholder={t('layouts.name_placeholder')} bind:value={name} /></div>
				<Button type="submit" disabled={!name.trim()}>{replacing ? t('layouts.replace') : t('common.save')}</Button>
			</form>
		{:else}
			<p class="hint">{t('layouts.save_hint')}</p>
		{/if}
	</section>

	{#if layouts.length === 0}
		<p class="hint">{t('layouts.empty')}</p>
	{:else}
		<ul class="list">
			{#each layouts as l (l.id)}
				<li>
					<div class="info">
						<span class="name">{l.name}</span>
						<span class="panes">{panes(l.id)}</span>
					</div>
					<Button
						size="sm"
						onclick={() => {
							onclose();
							openLayout(l.id);
						}}>{t('layouts.open')}</Button
					>
					<Button size="sm" variant="ghost" onclick={() => deleteLayout(l.id)}>{t('common.delete')}</Button>
				</li>
			{/each}
		</ul>
	{/if}
	<p class="note">{t('layouts.note')}</p>
</Modal>

<style>
	.save {
		margin-bottom: 16px;
	}

	.row {
		display: flex;
		align-items: flex-end;
		gap: 8px;
	}

	.grow {
		flex: 1;
		min-width: 0;
	}

	.hint,
	.note {
		margin: 0 0 12px;
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
	}

	.note {
		margin: 12px 0 0;
		font-size: var(--text-xs, 12px);
	}

	.list {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.list li {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 8px 10px;
		border: 1px solid var(--color-border);
		border-radius: 6px;
	}

	.info {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
	}

	.name {
		font-weight: 600;
		color: var(--color-text-primary);
	}

	.panes {
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
</style>
