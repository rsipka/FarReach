<script lang="ts">
	/**
	 * Which of a session's addresses to connect to: shown on Connect for a
	 * session set to Ask. The addresses reached straight are checked as it
	 * opens, so a dot says which answer from here; one that goes through the
	 * session's jump hosts or proxy cannot be checked without logging in.
	 */
	import Modal from '#lib/components/shared/Modal.svelte';
	import { t } from '#lib/state/i18n.svelte.js';
	import { sshProbeAddresses } from '#lib/ipc/ssh.js';
	import type { SessionConfig } from '#lib/ipc/sessions.js';
	import { addressText, lastAddress, addressKey, sessionAddresses, type ResolvedAddress } from '#lib/sessions/addresses.js';

	interface Props {
		session: SessionConfig;
		onpick: (address: ResolvedAddress) => void;
		oncancel: () => void;
	}

	let { session, onpick, oncancel }: Props = $props();

	let addresses = $derived(sessionAddresses(session));
	/** undefined: checking; null: no answer; a number: milliseconds. */
	let probes = $state<(number | null | undefined)[]>([]);
	let last = $derived(lastAddress(session.id));

	$effect(() => {
		const list = addresses;
		probes = list.map(() => undefined);
		let gone = false;
		sshProbeAddresses(list.map((a) => (a.routed ? { host: '', port: 0 } : { host: a.host, port: a.port })))
			.then((r) => {
				if (!gone) probes = r;
			})
			.catch(() => {
				if (!gone) probes = list.map(() => null);
			});
		return () => {
			gone = true;
		};
	});

	function status(a: ResolvedAddress, i: number): { cls: string; text: string } {
		if (a.routed) return { cls: 'routed', text: t('session.address_routed') };
		const p = probes[i];
		if (p === undefined) return { cls: 'checking', text: t('session.address_checking') };
		if (p === null) return { cls: 'down', text: t('session.address_no_answer') };
		return { cls: 'up', text: t('session.address_answers', { ms: String(p) }) };
	}
</script>

<Modal open={true} onclose={oncancel} title={t('session.connect_to', { name: session.name })} maxWidth="420px">
	<div class="picker">
		{#each addresses as a, i (a.index)}
			{@const st = status(a, i)}
			<button class="addr" type="button" onclick={() => onpick(a)}>
				<span class="dot {st.cls}" title={st.text}></span>
				<span class="main">
					<span class="name">{a.label || addressText(a, session.port)}</span>
					{#if a.label}<span class="host">{addressText(a, session.port)}</span>{/if}
				</span>
				<span class="meta">
					{#if last && addressKey(a) === last}<span class="last">{t('session.address_last')}</span>{/if}
					<span class="state">{st.text}</span>
				</span>
			</button>
		{/each}
	</div>
</Modal>

<style>
	.picker {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.addr {
		display: flex;
		align-items: center;
		gap: 10px;
		width: 100%;
		padding: 8px 10px;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-md, 6px);
		background: transparent;
		color: var(--color-text-primary);
		font: inherit;
		text-align: left;
		cursor: pointer;
	}

	.addr:hover,
	.addr:focus-visible {
		background: var(--color-bg-hover, rgba(127, 127, 127, 0.12));
	}

	.dot {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		flex-shrink: 0;
		background: var(--color-text-secondary);
		opacity: 0.5;
	}

	.dot.up {
		background: var(--color-success, #30d158);
		opacity: 1;
	}

	.dot.down {
		background: var(--color-danger, #ff453a);
		opacity: 1;
	}

	.dot.checking {
		animation: pulse 1s ease-in-out infinite;
	}

	@keyframes pulse {
		50% {
			opacity: 0.15;
		}
	}

	.main {
		display: flex;
		flex-direction: column;
		min-width: 0;
		flex: 1;
	}

	.name {
		font-size: var(--text-sm);
		font-weight: 500;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.host {
		font-family: var(--font-mono);
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.meta {
		display: flex;
		flex-direction: column;
		align-items: flex-end;
		font-size: var(--text-xs);
		color: var(--color-text-secondary);
		flex-shrink: 0;
	}

	.last {
		color: var(--color-accent);
	}
</style>
