import { invoke } from '@tauri-apps/api/core';
import type { SecretRef } from './pwmanager';
import type { TelnetOptions } from './telnet';
import type { SerialOptions } from './serial';
import type { HistoryMode } from './history';
import type { CursorStyle } from '#lib/terminal/cursor.js';
import { t } from '#lib/state/i18n.svelte.js';

export interface AuthMethod {
  type: 'Password' | 'Key' | 'Agent';
  password?: string; // for Password type - stored encrypted in vault
  path?: string; // for Key type - a key file on this machine
  passphrase?: string; // for Key type - stored encrypted in vault
  /**
   * For Key type - a key imported into the vault, used instead of `path`.
   * Set when the user picked an imported key, so the session also works on a
   * machine that has no copy of the key file.
   */
  key_id?: string;
  /** For Password type - an entry in a password manager; wins over `password`. */
  password_ref?: SecretRef | null;
  /** For Key type - an entry in a password manager; wins over `passphrase`. */
  passphrase_ref?: SecretRef | null;
}

export interface JumpHostConfig {
  host: string;
  port: number;
  username: string;
  auth_method: AuthMethod;
  /** A saved credential this hop logs in with, from the session's vault. */
  credential_id?: string | null;
  /** Read only: the saved credential the hop's login was filled in from. */
  credential?: CredentialUse | null;
}

export interface SessionConfig {
  id: string;
  name: string;
  host: string;
  port: number;
  username: string;
  auth_method: AuthMethod;
  folder_id: string | null;
  tags: string[];
  detected_os?: string | null;
  /** detected_os was picked by hand, so connecting never replaces it. */
  os_manual?: boolean;
  vault_id?: string | null; // Which vault this session belongs to
  jump_chain?: JumpHostConfig[] | null; // ProxyJump chain
  proxy?: ProxySessionConfig | null; // Proxy config (SOCKS5/Tor, HTTP)
  shell?: string | null; // Optional per-session login shell (e.g. "fish -l")
  /** What the session connects with. Absent means SSH: older saves have no kind. */
  kind?: SessionKind;
  /** Windows logon domain, RDP only. */
  domain?: string | null;
  /** A local folder shown inside the remote desktop as a drive, RDP only. */
  share_path?: string | null;
  /** RDP only: what the clipboard carries; absent for text, pictures and files. */
  rdp_clipboard?: 'text' | 'off' | null;
  /** VNC only: a saved SSH session to reach the server through. */
  via_session_id?: string | null;
  /** Key sessions: also offer the SSH agent's keys if this key is refused. Off by default. */
  try_agent_keys?: boolean | null;
  /** ssh_config settings: imported files, lines set in Reach, approvals. */
  ssh_options?: SshOptions | null;
  /** WSL only: the distribution name, e.g. "Ubuntu" */
  wsl_distro?: string | null;
  /** Telnet only: how it talks to the device. Absent: the defaults. */
  telnet?: TelnetOptions | null;
  /** Serial only: speed, framing, flow control and keys. The port is `host`. */
  serial?: SerialOptions | null;
  /** A saved credential this session logs in with (see savedCredentials.ts).
   *  Unset: its folder's, if one applies. */
  credential_id?: string | null;
  /** Read only: the saved credential the login was filled in from. */
  credential?: CredentialUse | null;
  /** On the start screen's Favorites. Stored on the session, so it syncs. */
  favorite?: boolean;
  /** Its place among the sessions of its folder (or those outside any),
   *  lowest first; unset: after those with one. Stored on the session, so
   *  it syncs. Changed only by sessionSetOrder. */
  order?: number | null;
  /** Free text about the server (RDM's Description and Notes). */
  notes?: string | null;
  /** Named values kept with the session, in the user's order. */
  custom_fields?: CustomField[] | null;
  /** The terminal colours its tabs use, by terminal theme id; unset: the
   *  default from Settings (see #lib/state/terminal-themes). */
  terminal_theme?: string | null;
  /** The terminal cursor's shape in its tabs; unset: the default from
   *  Settings (see #lib/terminal/cursor). */
  cursor_style?: CursorStyle | null;
  /** Whether that cursor blinks; unset: the default from Settings. */
  cursor_blink?: boolean | null;
  /** Server monitoring for its tabs, on or off; unset: the default from
   *  Settings. */
  monitoring?: boolean | null;
  /** What its tabs record into the session history; unset: the default
   *  from Settings. */
  history?: HistoryMode | null;
  /** Start its terminals in tmux on the server; unset: as its folder says
   *  (see #lib/terminal/tmux). */
  tmux?: TmuxSetting | null;
}

/** A session's own tmux choice. */
export interface TmuxSetting {
  /** `attach`: attach to the named tmux session, or create it; `ask`: pick
   *  from the server's tmux sessions at connect; `inherit`: as the folder
   *  says (kept to hold a name). */
  mode: 'off' | 'attach' | 'ask' | 'inherit';
  /** The tmux session's name; unset: made from the session's name. */
  name?: string | null;
  /** tmux's mouse mode for this session: the wheel scrolls tmux's history. */
  mouse?: boolean;
}

/** One named value of a session's documentation. */
export interface CustomField {
  name: string;
  value: string;
  /** Shown masked until revealed. */
  hidden?: boolean;
}

/** Which saved credential a session's login came from, as read. */
export interface CredentialUse {
  id: string;
  /** Empty when the credential is missing. */
  name: string;
  /** The folder it is inherited from; absent when the session links it. */
  folder_id?: string | null;
  /** Not in the session's vault (deleted, or not synced yet): the session's
   *  own stored login is used. */
  missing?: boolean;
}

/** A config file as it was read at import. */
export interface SshConfigFile {
  path: string;
  text: string;
  role: 'user' | 'system' | 'included';
}

/** A session's ssh_config settings (see src-tauri/src/ssh/sshconf). */
export interface SshOptions {
  /** What the session was imported from; resolved again at every connect. */
  imported?: { alias: string; files: SshConfigFile[]; at: number } | null;
  /** Lines set in Reach ("MACs +hmac-sha1"); they win over the files. */
  lines?: string[];
  /** Local commands the user allowed for this session, exactly as written. */
  approved_commands?: string[];
  /** Weakening settings the user has seen and kept ("Keyword value"). */
  accepted_weakenings?: string[];
  /** This person's approvals in plain words: filled from the signed ones when
   *  a session is read, signed by the backend when it is saved. */
  my_approved_commands?: string[] | null;
  my_accepted_weakenings?: string[] | null;
}

export type SessionKind = 'ssh' | 'rdp' | 'vnc' | 'wsl' | 'telnet' | 'serial';

export function sessionKind(session: Pick<SessionConfig, 'kind'>): SessionKind {
  return session.kind ?? 'ssh';
}

export interface ProxySessionConfig {
  proxy_type: string;
  host: string;
  port: number;
  username?: string | null;
  password?: string | null;
  /** An entry in a password manager; wins over `password`. */
  password_ref?: SecretRef | null;
}

export interface Folder {
  id: string;
  name: string;
  parent_id: string | null;
  vault_id: string | null;
  /** A saved credential the sessions inside log in with. */
  credential_id?: string | null;
  /** Start the sessions inside in tmux, unless they or a nearer folder say otherwise. */
  tmux?: boolean | null;
}

export async function sessionList(): Promise<SessionConfig[]> {
  return invoke<SessionConfig[]>('session_list');
}

export async function sessionGet(sessionId: string): Promise<SessionConfig> {
  return invoke<SessionConfig>('session_get', { sessionId });
}

export async function sessionCreate(params: {
  name: string;
  host: string;
  port: number;
  username: string;
  authMethod: AuthMethod;
  folderId: string | null;
  tags: string[];
  vaultId?: string | null;
  jumpChain?: JumpHostConfig[] | null;
  proxy?: ProxySessionConfig | null;
  shell?: string | null;
  kind?: SessionKind;
  domain?: string | null;
  /** For RDP: what the machine is, since the protocol cannot say. */
  detectedOs?: string | null;
  osManual?: boolean;
  sharePath?: string | null;
  rdpClipboard?: 'text' | 'off' | null;
  viaSessionId?: string | null;
  tryAgentKeys?: boolean | null;
  sshOptions?: SshOptions | null;
  wslDistro?: string | null;
  credentialId?: string | null;
  telnet?: TelnetOptions | null;
  serial?: SerialOptions | null;
  notes?: string | null;
  customFields?: CustomField[] | null;
  terminalTheme?: string | null;
  cursorStyle?: CursorStyle | null;
  cursorBlink?: boolean | null;
  monitoring?: boolean | null;
  history?: HistoryMode | null;
  tmux?: TmuxSetting | null;
}): Promise<SessionConfig> {
  return invoke<SessionConfig>('session_create', {
    name: params.name,
    host: params.host,
    port: params.port,
    username: params.username,
    authMethod: params.authMethod,
    folderId: params.folderId,
    tags: params.tags,
    vaultId: params.vaultId ?? null,
    jumpChain: params.jumpChain ?? null,
    proxy: params.proxy ?? null,
    shell: params.shell?.trim() ? params.shell.trim() : null,
    kind: params.kind ?? 'ssh',
    domain: params.domain?.trim() ? params.domain.trim() : null,
    detectedOs: params.detectedOs ?? null,
    osManual: params.osManual || null,
    sharePath: params.sharePath?.trim() ? params.sharePath.trim() : null,
    rdpClipboard: params.kind === 'rdp' ? (params.rdpClipboard ?? null) : null,
    viaSessionId: params.viaSessionId || null,
    tryAgentKeys: params.tryAgentKeys || null,
    sshOptions: params.sshOptions ?? null,
    wslDistro: params.wslDistro || null,
    credentialId: params.credentialId || null,
    telnet: params.kind === 'telnet' ? (params.telnet ?? null) : null,
    serial: params.kind === 'serial' ? (params.serial ?? null) : null,
    notes: params.notes?.trim() ? params.notes : null,
    customFields: params.customFields?.length ? params.customFields : null,
    terminalTheme: params.terminalTheme || null,
    cursorStyle: params.cursorStyle || null,
    cursorBlink: params.cursorBlink ?? null,
    monitoring: params.monitoring ?? null,
    history: params.history || null,
    tmux: params.tmux ?? null,
  });
}

export async function sessionUpdate(session: SessionConfig): Promise<SessionConfig> {
  return invoke<SessionConfig>('session_update', { session });
}

/** One password manager entry of a session waiting for the user's approval. */
export interface RefSlot {
  /** `password`, `passphrase`, `jump:<n>:password`, `jump:<n>:passphrase` or `proxy`. */
  slot: string;
  provider: string;
  entry_id: string;
}

/**
 * A session's password manager entries the current user has not approved,
 * with where they would go, read from the stored session in one go. The
 * confirmation dialog shows this and sends `context` and `refs` back.
 */
export interface UntrustedRefs {
  session_id: string;
  session_name: string;
  context: string;
  host: string;
  port: number;
  username: string;
  /** `user@host:port` for each jump host. */
  jumps: string[];
  /** `type user@host:port` of the proxy, if one is used. */
  proxy?: string | null;
  /** The session's own ssh_config lines that change where it connects (HostName, ProxyJump, ProxyCommand). */
  route_lines: string[];
  /** The SSH session a VNC session goes through, as stored now. */
  via?: ViaRoute | null;
  refs: RefSlot[];
}

/** Where the SSH session a VNC session goes through connects. */
export interface ViaRoute {
  session_id: string;
  session_name: string;
  host: string;
  port: number;
  username: string;
  jumps: string[];
  proxy?: string | null;
  route_lines: string[];
}

/** What a connect error starts with when the session has unapproved entries; JSON follows. */
export const UNTRUSTED_REFS_PREFIX = 'PWMANAGER_UNTRUSTED:';

/** The unapproved entries a connect was refused for, or null for any other error. */
export function untrustedRefsFromError(err: unknown): UntrustedRefs | null {
  const text = String(err);
  const at = text.indexOf(UNTRUSTED_REFS_PREFIX);
  if (at < 0) return null;
  try {
    return JSON.parse(text.slice(at + UNTRUSTED_REFS_PREFIX.length)) as UntrustedRefs;
  } catch {
    return null;
  }
}

/**
 * For a caller that cannot ask (a database, container host or cluster that
 * goes through a saved session): the unapproved-entries error as a sentence
 * pointing at the session list. Other errors pass through unchanged.
 */
export function explainUntrustedRefs<T>(call: Promise<T>): Promise<T> {
  return call.catch((err: unknown) => {
    throw untrustedRefsFromError(err) ? t('pwmanager.untrusted_reopen') : err;
  });
}

/** The session's unapproved password manager entries a connect would use, or null. */
export async function sessionUntrustedSecretRefs(sessionId: string): Promise<UntrustedRefs | null> {
  return invoke<UntrustedRefs | null>('session_untrusted_secret_refs', { sessionId });
}

/**
 * Approve entries, as the user just did in the confirmation dialog. Refused
 * if the session changed since `asked` was read.
 */
export async function sessionApproveSecretRefs(asked: UntrustedRefs): Promise<SessionConfig> {
  return invoke<SessionConfig>('session_approve_secret_refs', {
    sessionId: asked.session_id,
    context: asked.context,
    refs: asked.refs,
  });
}

export async function sessionDelete(sessionId: string): Promise<void> {
  return invoke('session_delete', { sessionId });
}

/** Move sessions into a folder (`null`: out of any folder) in one call.
 *  Only the folder changes; returns how many moved. */
export async function sessionMoveToFolder(sessionIds: string[], folderId: string | null): Promise<number> {
  return invoke<number>('session_move_to_folder', { sessionIds, folderId });
}

/** Mark sessions as favorites (or not) in one call. Only that flag
 *  changes; returns how many changed. */
export async function sessionSetFavorite(sessionIds: string[], favorite: boolean): Promise<number> {
  return invoke<number>('session_set_favorite', { sessionIds, favorite });
}

/** Put sessions in this order: each gets its index as its place. Give
 *  all the sessions of one folder (or those outside any). Only that place
 *  changes; returns how many changed. */
export async function sessionSetOrder(sessionIds: string[]): Promise<number> {
  return invoke<number>('session_set_order', { sessionIds });
}

export async function sessionListFolders(): Promise<Folder[]> {
  return invoke<Folder[]>('session_list_folders');
}

export async function sessionCreateFolder(name: string, parentId: string | null, vaultId?: string | null): Promise<Folder> {
  return invoke<Folder>('session_create_folder', { name, parentId, vaultId: vaultId ?? null });
}

export async function sessionUpdateFolderParent(folderId: string, parentId: string | null): Promise<Folder> {
  return invoke<Folder>('session_update_folder_parent', { folderId, parentId });
}

/** Turn tmux on or off for a folder's sessions; null: as the folder above says. */
export async function sessionSetFolderTmux(folderId: string, tmux: boolean | null): Promise<Folder> {
  return invoke<Folder>('session_set_folder_tmux', { folderId, tmux });
}

/** Set or clear the saved credential a folder's sessions log in with. */
export async function sessionSetFolderCredential(folderId: string, credentialId: string | null): Promise<Folder> {
  return invoke<Folder>('session_set_folder_credential', { folderId, credentialId });
}

/** Rename a folder; what is inside stays put. */
export async function sessionRenameFolder(folderId: string, name: string): Promise<Folder> {
  return invoke<Folder>('session_rename_folder', { folderId, name });
}

export async function sessionDeleteFolder(folderId: string): Promise<void> {
  return invoke('session_delete_folder', { folderId });
}

export interface ShareResult {
  shareId: string;
  shareUrl: string;
}

/** Share a session with another user via X25519 key re-wrap. */
export async function sessionShare(
  sessionId: string,
  recipientUuid: string,
  recipientPublicKey: string,
  expiresInHours?: number
): Promise<ShareResult> {
  return invoke<ShareResult>('session_share', {
    session_id: sessionId,
    recipient_uuid: recipientUuid,
    recipient_public_key: recipientPublicKey,
    expires_in_hours: expiresInHours
  });
}

/** Fetch installed WSL distributions on Windows */
export async function wslListDistros(): Promise<string[]> {
  return invoke<string[]>('wsl_list_distros');
}

/** One field of a bulk edit: a value for every session, or none. */
export type BulkChange<T> = { set: T } | 'clear';

/** What a bulk edit changes; a field left out stays as each session has it. */
export interface SessionPatch {
  username?: string;
  /** Only sessions that log in with a password take it. */
  password?: BulkChange<string>;
  /** `{ set: id }` a saved credential, `{ set: 'own' }` the session's own
   *  login, `'clear'` the folder's credential, if any. */
  credential?: BulkChange<string>;
  port?: number;
  domain?: BulkChange<string>;
  tags_add?: string[];
  tags_remove?: string[];
  notes?: { op: 'replace' | 'append'; text: string } | { op: 'clear' };
  terminal_theme?: BulkChange<string>;
  history?: BulkChange<HistoryMode>;
  cursor_style?: BulkChange<CursorStyle>;
  cursor_blink?: BulkChange<boolean>;
  /** SSH: log in with a key, the agent, or a password (set by `password`). */
  auth?: AuthMethod;
  /** SSH key sessions: also offer the agent's keys. */
  try_agent_keys?: boolean;
  /** SSH: the whole jump host chain, or none. */
  jump_chain?: BulkChange<JumpHostConfig[]>;
  /** SSH: the proxy, or none. */
  proxy?: BulkChange<ProxySessionConfig>;
  /** SSH: the login shell, or the server's default. */
  shell?: BulkChange<string>;
  /** Custom fields set by name (added or overwritten). */
  fields_set?: CustomField[];
  /** Custom fields removed by name. */
  fields_remove?: string[];
  /** Telnet: the parts of its options to change. */
  telnet?: Partial<TelnetOptions>;
  /** SSH and WSL: server monitoring on or off, or the default. */
  monitoring?: BulkChange<boolean>;
  /** SSH and WSL: tmux mode (`inherit`: as the folder says) and mouse. */
  tmux?: { mode?: 'off' | 'attach' | 'ask' | 'inherit'; mouse?: boolean };
  /** VNC: the saved SSH session it goes through, or directly. */
  via_session?: BulkChange<string>;
}

export interface BulkOutcome {
  id: string;
  name: string;
  status: 'changed' | 'unchanged' | 'failed';
  /** Patch fields this session does not take, e.g. `domain` on SSH. */
  skipped: string[];
  error?: string;
}

export interface BulkResult {
  results: BulkOutcome[];
  /** For `sessionBulkUndo`; absent when nothing changed. */
  undo?: string;
}

/** Change the same fields on many sessions. Each is saved as the session
 *  editor would save it; one that fails is reported and the rest stay saved. */
export async function sessionBulkUpdate(sessionIds: string[], patch: SessionPatch): Promise<BulkResult> {
  return invoke<BulkResult>('session_bulk_update', { sessionIds, patch });
}

/** Put the sessions of the last bulk edit back as they were, except those
 *  changed since (`kept`, by name). */
export async function sessionBulkUndo(token: string): Promise<{ restored: number; kept: string[] }> {
  return invoke<{ restored: number; kept: string[] }>('session_bulk_undo', { token });
}
