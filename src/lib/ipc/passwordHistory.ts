import { invoke } from '@tauri-apps/api/core';

/** A password a session or saved credential used before it was changed. */
export interface PastPassword {
  password: string;
  /** When it was replaced, in Unix seconds. */
  replaced_at: number;
}

/** Where a history belongs: a session or a saved credential. */
export type HistoryOwner = { session: string } | { credential: string };

/** The setting (kept in the vault) that says how many old passwords each
 *  session and credential keeps; '0' keeps none. */
export const PASSWORD_HISTORY_LIMIT_SETTING = 'password_history_limit';
export const PASSWORD_HISTORY_DEFAULT_LIMIT = 10;
export const PASSWORD_HISTORY_LIMITS = [0, 3, 5, 10, 20, 50];

/** Old passwords, newest first. */
export async function passwordHistory(owner: HistoryOwner): Promise<PastPassword[]> {
  return 'session' in owner
    ? invoke<PastPassword[]>('session_password_history', { sessionId: owner.session })
    : invoke<PastPassword[]>('saved_credential_password_history', { credentialId: owner.credential });
}

export async function passwordHistoryClear(owner: HistoryOwner): Promise<void> {
  return 'session' in owner
    ? invoke<void>('session_password_history_clear', { sessionId: owner.session })
    : invoke<void>('saved_credential_password_history_clear', { credentialId: owner.credential });
}

/** Forget every old password everywhere; returns how many sessions and
 *  credentials had some. */
export async function passwordHistoryClearAll(): Promise<number> {
  return invoke<number>('password_history_clear_all');
}
