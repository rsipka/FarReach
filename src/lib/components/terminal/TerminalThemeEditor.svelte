<script lang="ts">
	import Modal from '#lib/components/shared/Modal.svelte';
	import Button from '#lib/components/shared/Button.svelte';
	import { t } from '#lib/state/i18n.svelte.js';
	import { saveCustomTerminalTheme, type CustomTerminalTheme } from '#lib/state/terminal-themes.svelte.js';
	import { isHexColour } from '#lib/terminal/color-schemes.js';
	import type { TerminalPalette } from '#lib/state/theme.svelte.js';

	/**
	 * Edit a custom terminal theme: its name and its 22 colours, with the
	 * result drawn as a terminal as you go. Saving replaces the theme, so
	 * every session that uses it takes the new colours.
	 */
	interface Props {
		/** The theme to edit (a fresh copy for a new one); null: closed. */
		theme: CustomTerminalTheme | null;
		onclose: () => void;
		onsaved?: (id: string) => void;
	}

	let { theme, onclose, onsaved }: Props = $props();

	let name = $state('');
	let palette = $state<TerminalPalette>({} as TerminalPalette);
	let saving = $state(false);
	let error = $state<string | null>(null);

	$effect(() => {
		if (!theme) return;
		name = theme.name;
		palette = { ...theme.palette };
		error = null;
	});

	const SPECIAL: { key: keyof TerminalPalette; label: string }[] = $derived([
		{ key: 'background', label: t('termtheme.c_background') },
		{ key: 'foreground', label: t('termtheme.c_foreground') },
		{ key: 'cursor', label: t('termtheme.c_cursor') },
		{ key: 'cursorAccent', label: t('termtheme.c_cursor_text') },
		{ key: 'selectionBackground', label: t('termtheme.c_selection') },
		{ key: 'selectionForeground', label: t('termtheme.c_selection_text') }
	]);

	const HUES = ['black', 'red', 'green', 'yellow', 'blue', 'magenta', 'cyan', 'white'] as const;
	const bright = (h: (typeof HUES)[number]) => `bright${h[0].toUpperCase()}${h.slice(1)}` as keyof TerminalPalette;
	const hueLabel = (h: (typeof HUES)[number]) => t(`termtheme.c_${h}`);

	let valid = $derived(name.trim().length > 0 && Object.values(palette).every(isHexColour));

	async function save(): Promise<void> {
		if (!theme || !valid) return;
		saving = true;
		error = null;
		try {
			await saveCustomTerminalTheme({ id: theme.id, name: name.trim(), palette: { ...palette } });
			onsaved?.(theme.id);
			onclose();
		} catch (err) {
			error = String(err);
		} finally {
			saving = false;
		}
	}
</script>

{#snippet colour(key: keyof TerminalPalette, label: string)}
	<label class="colour" title={label}>
		<input type="color" value={isHexColour(palette[key]) ? palette[key] : '#000000'} oninput={(e) => (palette[key] = e.currentTarget.value)} aria-label={label} />
		<input
			class="hex"
			class:bad={!isHexColour(palette[key])}
			value={palette[key]}
			oninput={(e) => (palette[key] = e.currentTarget.value.trim())}
			maxlength="7"
			spellcheck="false"
			aria-label={label}
		/>
	</label>
{/snippet}

<Modal open={!!theme} {onclose} title={t('termtheme.edit_title')} maxWidth="640px">
	{#snippet children()}
		<div class="editor">
			<label class="name-field">
				<span class="label">{t('termtheme.name')}</span>
				<input class="name-input" bind:value={name} maxlength="80" />
			</label>

			<div class="preview" style:background={palette.background} style:color={palette.foreground}>
				<div><span style:color={palette.green}>user</span>@<span style:color={palette.blue}>prod-web</span>:<span style:color={palette.cyan}>~/app</span>$ ls --color</div>
				<div>
					<span style:color={palette.blue}>src</span>&nbsp; <span style:color={palette.green}>build.sh</span>&nbsp;
					<span style:color={palette.red}>core.dump</span>&nbsp; <span style:color={palette.magenta}>logo.png</span>&nbsp;
					<span style:color={palette.yellow}>notes.txt</span>&nbsp; <span style:color={palette.cyan}>link</span>
				</div>
				<div><span style:color={palette.brightBlack}># {t('termtheme.preview_comment')}</span></div>
				<div>
					<span style:background={palette.selectionBackground} style:color={palette.selectionForeground}>{t('termtheme.preview_selected')}</span>
					<span class="cursor" style:background={palette.cursor} style:color={palette.cursorAccent}>_</span>
				</div>
				<div class="row">
					{#each HUES as h (h)}<span class="chip" style:background={palette[h]}></span>{/each}
				</div>
				<div class="row">
					{#each HUES as h (h)}<span class="chip" style:background={palette[bright(h)]}></span>{/each}
				</div>
			</div>

			<div class="specials">
				{#each SPECIAL as s (s.key)}
					<div class="special">
						<span class="label">{s.label}</span>
						{@render colour(s.key, s.label)}
					</div>
				{/each}
			</div>

			<div class="grid">
				<span></span>
				<span class="label">{t('termtheme.normal')}</span>
				<span class="label">{t('termtheme.bright')}</span>
				{#each HUES as h (h)}
					<span class="label">{hueLabel(h)}</span>
					{@render colour(h, hueLabel(h))}
					{@render colour(bright(h), `${t('termtheme.bright')} ${hueLabel(h)}`)}
				{/each}
			</div>

			{#if error}<p class="error">{error}</p>{/if}
		</div>
	{/snippet}
	{#snippet actions()}
		<Button variant="secondary" onclick={onclose} disabled={saving}>{t('common.cancel')}</Button>
		<Button variant="primary" onclick={save} disabled={!valid || saving}>{t('common.save')}</Button>
	{/snippet}
</Modal>

<style>
	.editor {
		display: flex;
		flex-direction: column;
		gap: 14px;
	}

	.label {
		font-size: 0.75rem;
		color: var(--color-text-secondary);
	}

	.name-field {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.name-input,
	.hex {
		padding: 5px 8px;
		font-size: 0.8125rem;
		color: var(--color-text-primary);
		background: var(--color-bg-primary);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn, 6px);
		outline: none;
	}

	.name-input:focus,
	.hex:focus {
		border-color: var(--color-accent);
	}

	.hex {
		width: 76px;
		font-family: var(--font-mono, monospace);
	}

	.hex.bad {
		border-color: var(--color-danger);
	}

	.preview {
		padding: 10px 12px;
		border-radius: 8px;
		border: 1px solid var(--color-border);
		font-family: var(--font-mono, monospace);
		font-size: 0.8125rem;
		line-height: 1.5;
		overflow: hidden;
		white-space: nowrap;
	}

	.cursor {
		display: inline-block;
		width: 0.6em;
	}

	.row {
		display: flex;
		gap: 4px;
		margin-top: 4px;
	}

	.chip {
		width: 18px;
		height: 12px;
		border-radius: 3px;
	}

	.specials {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
		gap: 8px 16px;
	}

	.special {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
	}

	.grid {
		display: grid;
		grid-template-columns: max-content max-content max-content;
		align-items: center;
		gap: 6px 16px;
	}

	.colour {
		display: flex;
		align-items: center;
		gap: 6px;
	}

	input[type='color'] {
		width: 28px;
		height: 24px;
		padding: 0;
		border: 1px solid var(--color-border);
		border-radius: 4px;
		background: none;
		cursor: pointer;
	}

	.error {
		margin: 0;
		font-size: 0.8125rem;
		color: var(--color-danger);
	}
</style>
