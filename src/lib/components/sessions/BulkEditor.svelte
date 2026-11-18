<script lang="ts">
	// Editing many sessions at once. Every field starts as "keep": it shows
	// the value the sessions share, or "Mixed", and nothing is sent for it.
	// Typing or picking makes it a change (an accent bar and a revert
	// button); optional fields can also be cleared on every session. Apply
	// shows what will change before anything is written.
	import Modal from '#lib/components/shared/Modal.svelte';
	import Button from '#lib/components/shared/Button.svelte';
	import TerminalThemeList from '#lib/components/terminal/TerminalThemeList.svelte';
	import { t } from '#lib/state/i18n.svelte.js';
	import { findTerminalTheme, terminalPalette } from '#lib/state/terminal-themes.svelte.js';
	import {
		sessionBulkUpdate,
		type BulkResult,
		type SessionConfig,
		type SessionPatch
	} from '#lib/ipc/sessions.js';
	import { savedCredentialCreate, credentialsFor, type SavedCredential } from '#lib/ipc/savedCredentials.js';
	import type { HistoryMode } from '#lib/ipc/history.js';
	import {
		OWN_LOGIN,
		appliesTo,
		changedFields,
		countApplying,
		describeCounts,
		hasOwnPassword,
		parseTags,
		summarize,
		vaultsOf,
		type BulkField
	} from '#lib/sessions/bulk-edit.js';

	interface Props {
		open: boolean;
		sessions: SessionConfig[];
		credentials: SavedCredential[];
		onclose: () => void;
		/** After a write: the backend's report, to reload and offer Undo. */
		onapplied: (result: BulkResult) => void;
	}

	let { open, sessions, credentials, onclose, onapplied }: Props = $props();

	const KEEP = '__keep__';
	const FOLDER = '__folder__';

	// ---- What the user changed (null / KEEP: leave as is) ------------------
	let username = $state<string | null>(null);
	let password = $state<string | null>(null);
	let passwordClear = $state(false);
	let saveAsCredential = $state(true);
	let credentialName = $state('');
	let credential = $state(KEEP);
	let port = $state<string | null>(null);
	let domain = $state<string | null>(null);
	let domainClear = $state(false);
	let tagsAdd = $state('');
	let tagsRemove = $state('');
	let notesOp = $state<'keep' | 'replace' | 'append' | 'clear'>('keep');
	let notesText = $state('');
	let theme = $state<string | null>(null);
	let history = $state(KEEP);

	let phase = $state<'edit' | 'confirm' | 'saving' | 'report'>('edit');
	let error = $state('');
	let report = $state<BulkResult | null>(null);
	let themesOpen = $state(false);

	function reset(): void {
		username = password = port = domain = theme = null;
		passwordClear = domainClear = false;
		saveAsCredential = true;
		credentialName = '';
		credential = history = KEEP;
		tagsAdd = tagsRemove = notesText = '';
		notesOp = 'keep';
		phase = 'edit';
		error = '';
		report = null;
		themesOpen = false;
	}

	// A fresh form each time the dialog opens.
	$effect(() => {
		if (open) reset();
	});

	// ---- The selection -------------------------------------------------------
	let total = $derived(sessions.length);
	let vaults = $derived(vaultsOf(sessions));
	let oneVault = $derived(vaults.length === 1);
	let vaultCredentials = $derived(oneVault ? credentialsFor(credentials, vaults[0]) : []);
	let folderCount = $derived(new Set(sessions.map((s) => s.folder_id ?? '')).size);

	/** A typed password becomes a new saved credential, linked to them all. */
	let canCreateCredential = $derived(oneVault && countApplying('credential', sessions) > 0);
	let creatingCredential = $derived(!!password && !passwordClear && saveAsCredential && canCreateCredential);
	/** The credential choice as the patch will carry it ('__new__': the one
	 *  created on apply). */
	let credentialChange = $derived.by((): SessionPatch['credential'] => {
		if (creatingCredential) return { set: '__new__' };
		if (credential === KEEP) return undefined;
		if (credential === FOLDER) return 'clear';
		return { set: credential };
	});

	const count = (field: BulkField) => countApplying(field, sessions, credentialChange);
	const shows = (field: BulkField) => sessions.some((s) => appliesTo(field, s));

	// ---- Shared values, for placeholders -------------------------------------
	let usernames = $derived(summarize(sessions.filter((s) => appliesTo('username', s)), (s) => s.username));
	let ports = $derived(summarize(sessions.filter((s) => appliesTo('port', s)), (s) => s.port));
	let domains = $derived(summarize(sessions.filter((s) => appliesTo('domain', s)), (s) => s.domain));
	let creds = $derived(
		summarize(sessions.filter((s) => appliesTo('credential', s)), (s) =>
			s.credential_id === OWN_LOGIN ? t('credential.own_login') : s.credential && !s.credential.missing ? s.credential.name : s.credential_id ? t('credential.missing') : ''
		)
	);
	let themes = $derived(
		summarize(sessions.filter((s) => appliesTo('terminal_theme', s)), (s) => (s.terminal_theme ? (findTerminalTheme(s.terminal_theme)?.name ?? t('termtheme.missing')) : ''))
	);
	let histories = $derived(summarize(sessions.filter((s) => appliesTo('history', s)), (s) => s.history ?? ''));
	let withPassword = $derived(sessions.filter((s) => appliesTo('password', s) && hasOwnPassword(s)).length);
	let withNotes = $derived(sessions.filter((s) => s.notes?.trim()).length);
	let allTags = $derived(summarize(sessions.flatMap((s) => s.tags.map((tag) => ({ tag }))), (x) => x.tag));

	function placeholderOf(summary: ReturnType<typeof summarize>, blank: string): string {
		if (summary.counts.length === 0) return '';
		return summary.same !== null ? summary.same || blank : t('bulk.mixed');
	}

	function tooltipOf(summary: ReturnType<typeof summarize>, blank: string): string {
		return summary.same === null ? describeCounts(summary, blank) : '';
	}

	// ---- The patch ------------------------------------------------------------
	let newUsername = $derived(username !== null ? username.trim() : null);
	let credentialUsername = $derived(newUsername || usernames.same || '');

	let patch = $derived.by((): SessionPatch => {
		const p: SessionPatch = {};
		if (credentialChange !== undefined) p.credential = credentialChange;
		if (newUsername !== null && newUsername !== '') p.username = newUsername;
		if (passwordClear) p.password = 'clear';
		else if (password) p.password = { set: password };
		const n = port === null ? NaN : parseInt(port, 10);
		if (n > 0 && n < 65536) p.port = n;
		if (domainClear) p.domain = 'clear';
		else if (domain !== null && domain.trim()) p.domain = { set: domain.trim() };
		const add = parseTags(tagsAdd);
		const remove = parseTags(tagsRemove);
		if (add.length) p.tags_add = add;
		if (remove.length) p.tags_remove = remove;
		if (notesOp === 'clear') p.notes = { op: 'clear' };
		else if (notesOp !== 'keep' && notesText.trim()) p.notes = { op: notesOp, text: notesText };
		if (theme !== null) p.terminal_theme = theme ? { set: theme } : 'clear';
		if (history !== KEEP) p.history = history ? { set: history as HistoryMode } : 'clear';
		return p;
	});
	let fields = $derived(changedFields(patch));
	/** The confirm list: a username and password that only go into the new
	 *  credential are shown as part of it. */
	let shownFields = $derived(
		fields.filter((f) => !(creatingCredential && (f === 'username' || f === 'password') && count(f) === 0))
	);
	let portInvalid = $derived(port !== null && port.trim() !== '' && !(parseInt(port, 10) > 0 && parseInt(port, 10) < 65536));
	let credentialNeedsName = $derived(creatingCredential && (!credentialName.trim() || !credentialUsername));
	let canApply = $derived(fields.length > 0 && !portInvalid && !credentialNeedsName);

	// ---- Confirm --------------------------------------------------------------
	const fieldLabel = (f: BulkField): string =>
		({
			username: t('session.username'),
			password: t('session.password'),
			credential: t('credential.login'),
			port: t('session.port'),
			domain: t('bulk.domain'),
			tags: t('bulk.tags'),
			notes: t('session.notes'),
			terminal_theme: t('termtheme.session_title'),
			history: t('history.session_label')
		})[f];

	function describeChange(f: BulkField): string {
		const p = patch;
		switch (f) {
			case 'username':
				return p.username ?? '';
			case 'password':
				return p.password === 'clear' ? t('bulk.cleared') : t('bulk.new_password');
			case 'credential':
				if (creatingCredential) return `${t('bulk.new_credential', { name: credentialName.trim() })} (${credentialUsername})`;
				if (p.credential === 'clear') return t('bulk.folder_credential');
				if (p.credential?.set === OWN_LOGIN) return t('credential.own_login');
				return vaultCredentials.find((c) => c.id === (p.credential as { set: string } | undefined)?.set)?.name ?? '';
			case 'port':
				return String(p.port);
			case 'domain':
				return p.domain === 'clear' ? t('bulk.cleared') : (p.domain?.set ?? '');
			case 'tags':
				return [...(p.tags_add ?? []).map((x) => `+${x}`), ...(p.tags_remove ?? []).map((x) => `−${x}`)].join(', ');
			case 'notes':
				return p.notes?.op === 'clear' ? t('bulk.cleared') : p.notes?.op === 'append' ? t('bulk.notes_appended') : t('bulk.notes_replaced');
			case 'terminal_theme':
				return p.terminal_theme === 'clear' ? t('termtheme.session_default') : (findTerminalTheme(p.terminal_theme?.set)?.name ?? '');
			case 'history':
				return p.history === 'clear' ? t('history.session_default') : t(`history.mode_${p.history?.set}`);
		}
	}

	async function apply(ids: string[] = sessions.map((s) => s.id)): Promise<void> {
		phase = 'saving';
		error = '';
		try {
			const send = { ...patch };
			if (creatingCredential) {
				const created = await savedCredentialCreate({
					name: credentialName.trim(),
					username: credentialUsername,
					domain: patch.domain && patch.domain !== 'clear' ? patch.domain.set : (domains.same ?? null),
					authMethod: { type: 'Password', password: password ?? '' },
					vaultId: vaults[0]
				});
				send.credential = { set: created.id };
				// Linked once; a retry uses the same credential.
				credential = created.id;
				saveAsCredential = false;
			}
			const result = await sessionBulkUpdate(ids, send);
			onapplied(result);
			const failed = result.results.filter((r) => r.status === 'failed');
			if (failed.length > 0) {
				report = result;
				phase = 'report';
			} else {
				onclose();
			}
		} catch (e) {
			error = String(e);
			phase = 'confirm';
		}
	}

	let failedIds = $derived(report?.results.filter((r) => r.status === 'failed').map((r) => r.id) ?? []);
</script>

<Modal {open} {onclose} title={t('bulk.title', { count: String(total) })} maxWidth="560px">
	{#if phase === 'edit'}
		<div class="bulk">
			<p class="intro">
				{folderCount > 1 ? t('bulk.intro_folders', { count: String(total), folders: String(folderCount) }) : t('bulk.intro', { count: String(total) })}
				{t('bulk.intro_keep')}
			</p>

			<h3 class="section">{t('credential.login')}</h3>

			{#if shows('credential')}
				<div class="row" class:changed={credential !== KEEP || creatingCredential}>
					<label class="label" for="bulk-credential">{t('credential.login')}</label>
					<div class="field">
						<select
							id="bulk-credential"
							bind:value={credential}
							disabled={!oneVault || creatingCredential}
							title={tooltipOf(creds, t('bulk.folder_credential'))}
						>
							<option value={KEEP}>{t('bulk.keep_value', { value: placeholderOf(creds, t('bulk.folder_credential')) })}</option>
							<option value={FOLDER}>{t('bulk.folder_credential')}</option>
							<option value={OWN_LOGIN}>{t('credential.own_login')}</option>
							{#each vaultCredentials as c (c.id)}
								<option value={c.id}>{c.name} ({c.username})</option>
							{/each}
						</select>
						{#if creatingCredential}
							<p class="hint">{t('bulk.new_credential', { name: credentialName.trim() || '…' })}</p>
						{:else if !oneVault}
							<p class="hint">{t('bulk.one_vault_only')}</p>
						{/if}
						<p class="applies">{t('bulk.applies', { count: String(count('credential')), total: String(total) })}</p>
					</div>
					{#if credential !== KEEP}
						<button class="revert" type="button" onclick={() => (credential = KEEP)} title={t('bulk.revert')} aria-label={t('bulk.revert')}>↺</button>
					{/if}
				</div>
			{/if}

			{#if shows('username')}
				<div class="row" class:changed={username !== null && username.trim() !== ''}>
					<label class="label" for="bulk-username">{t('session.username')}</label>
					<div class="field">
						<input
							id="bulk-username"
							type="text"
							autocomplete="off"
							spellcheck="false"
							value={username ?? ''}
							oninput={(e) => (username = e.currentTarget.value)}
							placeholder={placeholderOf(usernames, t('bulk.none'))}
							title={tooltipOf(usernames, t('bulk.none'))}
						/>
						{#if creatingCredential}
							<p class="applies">{t('bulk.into_credential', { count: String(countApplying('credential', sessions)), total: String(total) })}</p>
						{:else}
							<p class="applies">{t('bulk.applies', { count: String(count('username')), total: String(total) })}</p>
						{/if}
					</div>
					{#if username !== null && username !== ''}
						<button class="revert" type="button" onclick={() => (username = null)} title={t('bulk.revert')} aria-label={t('bulk.revert')}>↺</button>
					{/if}
				</div>
			{/if}

			{#if shows('password')}
				<div class="row" class:changed={passwordClear || !!password}>
					<label class="label" for="bulk-password">{t('session.password')}</label>
					<div class="field">
						{#if passwordClear}
							<p class="cleared">{t('bulk.will_clear')}</p>
						{:else}
							<input
								id="bulk-password"
								type="password"
								autocomplete="new-password"
								value={password ?? ''}
								oninput={(e) => (password = e.currentTarget.value)}
								placeholder={t('bulk.password_set_on', { count: String(withPassword), total: String(count('password')) })}
							/>
						{/if}
						{#if password && !passwordClear && canCreateCredential}
							<label class="check">
								<input type="checkbox" bind:checked={saveAsCredential} />
								<span>{t('bulk.save_as_credential')}</span>
							</label>
							{#if saveAsCredential}
								<input
									class="sub-input"
									type="text"
									value={credentialName}
									oninput={(e) => (credentialName = e.currentTarget.value)}
									placeholder={t('bulk.credential_name')}
									aria-label={t('bulk.credential_name')}
								/>
								{#if !credentialUsername}
									<p class="hint warn">{t('bulk.credential_needs_username')}</p>
								{:else}
									<p class="hint">{t('bulk.credential_hint', { username: credentialUsername })}</p>
								{/if}
							{/if}
						{/if}
						{#if !creatingCredential}
							<p class="applies">{t('bulk.applies', { count: String(count('password')), total: String(total) })}</p>
						{/if}
					</div>
					{#if passwordClear || password}
						<button class="revert" type="button" onclick={() => { password = null; passwordClear = false; }} title={t('bulk.revert')} aria-label={t('bulk.revert')}>↺</button>
					{:else}
						<button class="clear" type="button" onclick={() => (passwordClear = true)}>{t('bulk.clear')}</button>
					{/if}
				</div>
			{/if}

			{#if shows('port')}
				<div class="row" class:changed={port !== null && port.trim() !== ''}>
					<label class="label" for="bulk-port">{t('session.port')}</label>
					<div class="field">
						<input
							id="bulk-port"
							type="text"
							inputmode="numeric"
							class:invalid={portInvalid}
							value={port ?? ''}
							oninput={(e) => (port = e.currentTarget.value)}
							placeholder={placeholderOf(ports, t('bulk.none'))}
							title={tooltipOf(ports, t('bulk.none'))}
						/>
						<p class="applies">{t('bulk.applies', { count: String(count('port')), total: String(total) })}</p>
					</div>
					{#if port}
						<button class="revert" type="button" onclick={() => (port = null)} title={t('bulk.revert')} aria-label={t('bulk.revert')}>↺</button>
					{/if}
				</div>
			{/if}

			{#if shows('domain')}
				<div class="row" class:changed={domainClear || !!domain?.trim()}>
					<label class="label" for="bulk-domain">{t('bulk.domain')}</label>
					<div class="field">
						{#if domainClear}
							<p class="cleared">{t('bulk.will_clear')}</p>
						{:else}
							<input
								id="bulk-domain"
								type="text"
								autocomplete="off"
								value={domain ?? ''}
								oninput={(e) => (domain = e.currentTarget.value)}
								placeholder={placeholderOf(domains, t('bulk.none'))}
								title={tooltipOf(domains, t('bulk.none'))}
							/>
						{/if}
						<p class="applies">{t('bulk.applies', { count: String(count('domain')), total: String(total) })}</p>
					</div>
					{#if domainClear || domain}
						<button class="revert" type="button" onclick={() => { domain = null; domainClear = false; }} title={t('bulk.revert')} aria-label={t('bulk.revert')}>↺</button>
					{:else}
						<button class="clear" type="button" onclick={() => (domainClear = true)}>{t('bulk.clear')}</button>
					{/if}
				</div>
			{/if}

			{#if shows('terminal_theme') || shows('history')}
				<h3 class="section">{t('bulk.section_terminal')}</h3>
			{/if}

			{#if shows('terminal_theme')}
				<div class="row" class:changed={theme !== null}>
					<span class="label">{t('termtheme.session_title')}</span>
					<div class="field">
						<button class="picker" type="button" onclick={() => (themesOpen = !themesOpen)} title={tooltipOf(themes, t('termtheme.session_default'))}>
							{theme !== null
								? (theme ? (findTerminalTheme(theme)?.name ?? theme) : t('termtheme.session_default'))
								: t('bulk.keep_value', { value: placeholderOf(themes, t('termtheme.session_default')) })}
						</button>
						{#if themesOpen}
							<div class="themes">
								<TerminalThemeList
									selected={theme ?? ''}
									onselect={(id) => { theme = id; themesOpen = false; }}
									noneLabel={t('termtheme.session_default')}
									nonePalette={terminalPalette(null)}
								/>
							</div>
						{/if}
						<p class="applies">{t('bulk.applies', { count: String(count('terminal_theme')), total: String(total) })}</p>
					</div>
					{#if theme !== null}
						<button class="revert" type="button" onclick={() => (theme = null)} title={t('bulk.revert')} aria-label={t('bulk.revert')}>↺</button>
					{/if}
				</div>
			{/if}

			{#if shows('history')}
				<div class="row" class:changed={history !== KEEP}>
					<label class="label" for="bulk-history">{t('history.session_label')}</label>
					<div class="field">
						<select id="bulk-history" bind:value={history} title={tooltipOf(histories, t('history.session_default'))}>
							<option value={KEEP}>{t('bulk.keep_value', { value: histories.same === null ? t('bulk.mixed') : histories.same ? t(`history.mode_${histories.same}`) : t('history.session_default') })}</option>
							<option value="">{t('history.session_default')}</option>
							<option value="off">{t('history.mode_off')}</option>
							<option value="commands">{t('history.mode_commands')}</option>
							<option value="output">{t('history.mode_output')}</option>
						</select>
						<p class="applies">{t('bulk.applies', { count: String(count('history')), total: String(total) })}</p>
					</div>
					{#if history !== KEEP}
						<button class="revert" type="button" onclick={() => (history = KEEP)} title={t('bulk.revert')} aria-label={t('bulk.revert')}>↺</button>
					{/if}
				</div>
			{/if}

			<h3 class="section">{t('session.docs')}</h3>

			<div class="row" class:changed={!!tagsAdd.trim() || !!tagsRemove.trim()}>
				<span class="label">{t('bulk.tags')}</span>
				<div class="field">
					<input
						type="text"
						autocomplete="off"
						value={tagsAdd}
						oninput={(e) => (tagsAdd = e.currentTarget.value)}
						placeholder={t('bulk.tags_add')}
						aria-label={t('bulk.tags_add')}
					/>
					<input
						class="sub-input"
						type="text"
						autocomplete="off"
						value={tagsRemove}
						oninput={(e) => (tagsRemove = e.currentTarget.value)}
						placeholder={t('bulk.tags_remove')}
						aria-label={t('bulk.tags_remove')}
						title={describeCounts(allTags, '')}
					/>
				</div>
				{#if tagsAdd || tagsRemove}
					<button class="revert" type="button" onclick={() => { tagsAdd = ''; tagsRemove = ''; }} title={t('bulk.revert')} aria-label={t('bulk.revert')}>↺</button>
				{/if}
			</div>

			<div class="row" class:changed={notesOp === 'clear' || (notesOp !== 'keep' && !!notesText.trim())}>
				<label class="label" for="bulk-notes-op">{t('session.notes')}</label>
				<div class="field">
					<select id="bulk-notes-op" bind:value={notesOp}>
						<option value="keep">{t('bulk.keep_value', { value: t('bulk.notes_on', { count: String(withNotes), total: String(total) }) })}</option>
						<option value="replace">{t('bulk.notes_replace')}</option>
						<option value="append">{t('bulk.notes_append')}</option>
						<option value="clear">{t('bulk.clear')}</option>
					</select>
					{#if notesOp === 'replace' || notesOp === 'append'}
						<textarea class="sub-input" rows="3" bind:value={notesText} placeholder={t('session.notes_placeholder')}></textarea>
					{/if}
				</div>
				{#if notesOp !== 'keep'}
					<button class="revert" type="button" onclick={() => { notesOp = 'keep'; notesText = ''; }} title={t('bulk.revert')} aria-label={t('bulk.revert')}>↺</button>
				{/if}
			</div>
		</div>
	{:else if phase === 'confirm' || phase === 'saving'}
		<div class="bulk">
			<p class="intro">{t('bulk.confirm_intro', { fields: String(shownFields.length), count: String(total) })}</p>
			<ul class="changes">
				{#each shownFields as f (f)}
					{@const n = f === 'credential' && creatingCredential ? countApplying('credential', sessions) : count(f)}
					<li>
						<span class="change-field">{fieldLabel(f)}</span>
						<span class="change-value">{describeChange(f)}</span>
						{#if n < total}
							<span class="change-skip">{t('bulk.skipped_n', { count: String(total - n) })}</span>
						{/if}
					</li>
				{/each}
			</ul>
			<details class="names">
				<summary>{t('bulk.sessions_list', { count: String(total) })}</summary>
				<ul>
					{#each sessions as s (s.id)}
						<li>{s.name} <span class="host">{s.host}</span></li>
					{/each}
				</ul>
			</details>
			<p class="hint">{t('bulk.undo_hint')}</p>
			{#if error}
				<p class="error" role="alert">{error}</p>
			{/if}
		</div>
	{:else if phase === 'report' && report}
		<div class="bulk">
			<p class="intro">
				{t('bulk.report', {
					changed: String(report.results.filter((r) => r.status === 'changed').length),
					failed: String(failedIds.length)
				})}
			</p>
			<ul class="changes">
				{#each report.results.filter((r) => r.status === 'failed') as r (r.id)}
					<li><span class="change-field">{r.name || r.id}</span><span class="error">{r.error}</span></li>
				{/each}
			</ul>
		</div>
	{/if}

	{#snippet actions()}
		{#if phase === 'edit'}
			<Button variant="ghost" onclick={onclose}>{t('common.cancel')}</Button>
			<Button variant="primary" disabled={!canApply} onclick={() => (phase = 'confirm')}>{t('bulk.review')}</Button>
		{:else if phase === 'confirm' || phase === 'saving'}
			<Button variant="ghost" disabled={phase === 'saving'} onclick={() => (phase = 'edit')}>{t('common.back')}</Button>
			<Button variant="primary" disabled={phase === 'saving'} onclick={() => void apply()}>
				{t('bulk.apply_n', { count: String(total) })}
			</Button>
		{:else}
			<Button variant="ghost" onclick={onclose}>{t('common.close')}</Button>
			<Button variant="primary" onclick={() => void apply(failedIds)}>{t('common.retry')}</Button>
		{/if}
	{/snippet}
</Modal>

<style>
	.bulk {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}
	.intro {
		margin: 0;
		font-size: 0.8125rem;
		color: var(--color-text-secondary);
	}
	.section {
		margin: 8px 0 0;
		font-size: 0.75rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.04em;
		color: var(--color-text-tertiary);
	}
	.row {
		display: grid;
		grid-template-columns: 130px 1fr 52px;
		align-items: start;
		gap: 8px;
		padding: 4px 0 4px 8px;
		border-left: 3px solid transparent;
	}
	.row.changed {
		border-left-color: var(--color-accent);
	}
	.label {
		padding-top: 8px;
		font-size: 0.8125rem;
		color: var(--color-text-primary);
	}
	.field {
		display: flex;
		flex-direction: column;
		gap: 4px;
		min-width: 0;
	}
	.field input[type='text'],
	.field input[type='password'],
	.field select,
	.field textarea,
	.picker {
		width: 100%;
		box-sizing: border-box;
		padding: 7px 10px;
		font-family: var(--font-sans);
		font-size: 0.8125rem;
		color: var(--color-text-primary);
		background-color: var(--color-bg-elevated);
		border: 1px solid var(--color-border-strong);
		border-radius: var(--radius-btn);
		outline: none;
		text-align: left;
	}
	.field textarea {
		resize: vertical;
	}
	.field input:focus,
	.field select:focus,
	.field textarea:focus {
		border-color: var(--color-accent);
	}
	.field input.invalid {
		border-color: var(--color-danger);
	}
	.picker {
		cursor: pointer;
	}
	.themes {
		max-height: 260px;
		overflow: auto;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		padding: 6px;
	}
	.applies,
	.hint {
		margin: 0;
		font-size: 0.6875rem;
		color: var(--color-text-tertiary);
	}
	.hint.warn {
		color: var(--color-warning);
	}
	.cleared {
		margin: 0;
		padding: 7px 0;
		font-size: 0.8125rem;
		font-style: italic;
		color: var(--color-text-secondary);
	}
	.check {
		display: flex;
		align-items: center;
		gap: 6px;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
	}
	.revert,
	.clear {
		margin-top: 4px;
		padding: 3px 6px;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
		background: transparent;
		border: 1px solid var(--color-border);
		border-radius: 6px;
		cursor: pointer;
	}
	.revert:hover,
	.clear:hover {
		background: var(--color-surface-hover);
		color: var(--color-text-primary);
	}
	.changes {
		margin: 0;
		padding: 0;
		list-style: none;
		display: flex;
		flex-direction: column;
		gap: 6px;
	}
	.changes li {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
		align-items: baseline;
		font-size: 0.8125rem;
	}
	.change-field {
		min-width: 130px;
		color: var(--color-text-secondary);
	}
	.change-value {
		color: var(--color-text-primary);
		font-weight: 500;
		overflow-wrap: anywhere;
	}
	.change-skip {
		font-size: 0.6875rem;
		color: var(--color-text-tertiary);
	}
	.names {
		font-size: 0.8125rem;
		color: var(--color-text-secondary);
	}
	.names ul {
		max-height: 180px;
		overflow: auto;
		margin: 6px 0 0;
		padding-left: 18px;
	}
	.host {
		color: var(--color-text-tertiary);
	}
	.error {
		margin: 0;
		color: var(--color-danger);
		font-size: 0.8125rem;
		overflow-wrap: anywhere;
	}
	@media (max-width: 520px) {
		.row {
			grid-template-columns: 1fr auto;
		}
		.label {
			grid-column: 1 / -1;
			padding-top: 0;
		}
	}
</style>
