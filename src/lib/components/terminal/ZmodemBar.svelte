<script lang="ts">
	/**
	 * ZMODEM in a terminal (SSH, Telnet, serial). When the remote runs `rz`
	 * this asks for files to send; when it runs `sz`, for a folder to save
	 * into. While the transfer runs it shows over the bottom of the terminal
	 * with its progress and a Cancel button. The transfer itself runs in the
	 * backend, on the session's own connection (src-tauri/src/zmodem).
	 */
	import { listen, type UnlistenFn } from '@tauri-apps/api/event';
	import { open } from '@tauri-apps/plugin-dialog';
	import { downloadDir } from '@tauri-apps/api/path';
	import { zmodemCancel, zmodemDownload, zmodemUpload, type ZmodemDirection, type ZmodemEvent } from '#lib/ipc/zmodem.js';
	import { formatBytes, percent, Speed } from '#lib/terminal/zmodem.js';
	import { addToast } from '#lib/state/toasts.svelte.js';
	import { t } from '#lib/state/i18n.svelte.js';
	import { isMobile } from '#lib/platform.js';

	interface Props {
		/** The session's connection; events come as `zmodem-{id}`. */
		connectionId?: string;
	}

	let { connectionId }: Props = $props();

	interface Running {
		direction: ZmodemDirection;
		file: string;
		index: number;
		count: number | null;
		done: number;
		total: number | null;
		rate: number | null;
	}

	/** Choosing files or a folder, or the transfer going. */
	let choosing = $state<ZmodemDirection | null>(null);
	let running = $state<Running | null>(null);
	let cancelling = $state(false);
	const speed = new Speed();

	const LAST_FOLDER = 'reach.zmodem.folder';

	function lastFolder(): string | null {
		try {
			return localStorage.getItem(LAST_FOLDER);
		} catch {
			return null;
		}
	}

	function rememberFolder(folder: string): void {
		try {
			localStorage.setItem(LAST_FOLDER, folder);
		} catch {
			// Only a convenience.
		}
	}

	async function answer(id: string, direction: ZmodemDirection): Promise<void> {
		if (isMobile()) {
			addToast(t('zmodem.desktop_only'), 'info', 5000);
			await zmodemCancel(id).catch(() => {});
			return;
		}
		choosing = direction;
		try {
			if (direction === 'upload') {
				const picked = await open({ multiple: true, directory: false, title: t('zmodem.pick_files') });
				const paths = picked === null ? [] : Array.isArray(picked) ? picked : [picked];
				if (paths.length === 0) {
					await zmodemCancel(id);
					return;
				}
				running = { direction, file: '', index: 0, count: paths.length, done: 0, total: null, rate: null };
				await zmodemUpload(id, paths);
			} else {
				const defaultPath = lastFolder() ?? (await downloadDir().catch(() => undefined));
				const picked = await open({ directory: true, multiple: false, defaultPath, title: t('zmodem.pick_folder') });
				const folder = Array.isArray(picked) ? picked[0] : picked;
				if (!folder) {
					await zmodemCancel(id);
					return;
				}
				rememberFolder(folder);
				running = { direction, file: '', index: 0, count: null, done: 0, total: null, rate: null };
				await zmodemDownload(id, folder);
			}
		} catch (e) {
			running = null;
			addToast(t('zmodem.failed', { error: String(e) }), 'error', 6000);
			await zmodemCancel(id).catch(() => {});
		} finally {
			choosing = null;
		}
	}

	function onEvent(id: string, e: ZmodemEvent): void {
		switch (e.kind) {
			case 'offer':
				cancelling = false;
				void answer(id, e.direction);
				break;
			case 'progress':
				running = {
					direction: e.direction,
					file: e.file,
					index: e.index,
					count: e.count,
					done: e.done,
					total: e.total,
					rate: speed.add(`${e.index}:${e.file}`, e.done, performance.now())
				};
				break;
			case 'finished': {
				running = null;
				const files = e.files.join(', ');
				if (e.direction === 'download') {
					addToast(t('zmodem.received', { files, folder: e.folder ?? '' }), 'success', 6000);
				} else {
					addToast(t('zmodem.sent', { files }), 'success', 4000);
				}
				break;
			}
			case 'cancelled':
				running = null;
				addToast(t('zmodem.cancelled'), 'info', 3000);
				break;
			case 'failed':
				running = null;
				addToast(t('zmodem.failed', { error: e.error }), 'error', 8000);
				break;
		}
	}

	$effect(() => {
		const id = connectionId;
		if (!id) return;
		let unlisten: UnlistenFn | undefined;
		let gone = false;
		void listen<ZmodemEvent>(`zmodem-${id}`, (event) => onEvent(id, event.payload)).then((u) => {
			if (gone) u();
			else unlisten = u;
		});
		return () => {
			gone = true;
			unlisten?.();
			running = null;
			choosing = null;
		};
	});

	function cancel(): void {
		if (!connectionId) return;
		cancelling = true;
		zmodemCancel(connectionId).catch(() => {});
	}

	let shown = $derived(running ?? (choosing ? { direction: choosing } : null));
	let pct = $derived(running ? percent(running.done, running.total) : null);
</script>

{#if shown}
	<div class="zmodem-bar" role="status" aria-live="polite">
		<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
			{#if shown.direction === 'upload'}
				<path d="M12 19V5" /><path d="m5 12 7-7 7 7" />
			{:else}
				<path d="M12 5v14" /><path d="m19 12-7 7-7-7" />
			{/if}
		</svg>
		<div class="body">
			{#if running && running.file}
				<div class="line">
					<span class="name" title={running.file}>
						{t(running.direction === 'upload' ? 'zmodem.sending' : 'zmodem.receiving', { file: running.file })}
					</span>
					{#if running.count && running.count > 1}
						<span class="muted">{t('zmodem.file_of', { index: String(running.index), count: String(running.count) })}</span>
					{/if}
				</div>
				<div class="meter" class:unknown={pct === null}>
					<div class="fill" style:width={pct === null ? '100%' : `${pct}%`}></div>
				</div>
				<div class="line muted">
					<span>
						{formatBytes(running.done)}{running.total !== null ? ` / ${formatBytes(running.total)}` : ''}{pct !== null ? ` · ${pct} %` : ''}
					</span>
					{#if running.rate !== null}
						<span>{formatBytes(running.rate)}/s</span>
					{/if}
				</div>
			{:else if running}
				<span class="name">{t('zmodem.starting')}</span>
			{:else}
				<span class="name">{t(shown.direction === 'upload' ? 'zmodem.pick_files' : 'zmodem.pick_folder')}</span>
			{/if}
		</div>
		{#if running}
			<button class="bar-btn" onclick={cancel} disabled={cancelling} title={t('zmodem.cancel_hint')}>{t('common.cancel')}</button>
		{/if}
	</div>
{/if}

<style>
	.zmodem-bar {
		position: absolute;
		left: 12px;
		right: 12px;
		bottom: 12px;
		z-index: 5;
		display: flex;
		align-items: center;
		gap: 10px;
		max-width: 560px;
		margin: 0 auto;
		padding: 8px 12px;
		border-radius: 10px;
		font-size: var(--text-sm, 0.8125rem);
		color: var(--color-text-primary);
		background: color-mix(in srgb, var(--color-bg-secondary, var(--color-bg-primary)) 94%, transparent);
		border: 1px solid var(--color-border, rgba(127, 127, 127, 0.3));
		box-shadow: 0 6px 24px rgba(0, 0, 0, 0.25);
		backdrop-filter: blur(8px);
	}

	.zmodem-bar svg {
		flex-shrink: 0;
		color: var(--color-accent);
	}

	.body {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.line {
		display: flex;
		justify-content: space-between;
		gap: 8px;
		min-width: 0;
	}

	.name {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-weight: 600;
	}

	.muted {
		color: var(--color-text-secondary);
		font-size: var(--text-xs, 0.75rem);
		white-space: nowrap;
		font-variant-numeric: tabular-nums;
	}

	.meter {
		height: 4px;
		border-radius: 2px;
		background: color-mix(in srgb, var(--color-text-secondary) 20%, transparent);
		overflow: hidden;
	}

	.fill {
		height: 100%;
		background: var(--color-accent);
		transition: width 0.15s linear;
	}

	.meter.unknown .fill {
		opacity: 0.5;
	}

	.bar-btn {
		flex-shrink: 0;
		padding: 3px 12px;
		border-radius: 6px;
		border: 1px solid var(--color-border, currentColor);
		background: transparent;
		color: inherit;
		font: inherit;
		cursor: pointer;
	}

	.bar-btn:hover:not(:disabled) {
		background: color-mix(in srgb, var(--color-error, #e5484d) 14%, transparent);
	}

	.bar-btn:disabled {
		opacity: 0.6;
		cursor: default;
	}

	.bar-btn:focus-visible {
		outline: 2px solid var(--color-accent);
		outline-offset: 1px;
	}
</style>
