import { invoke } from '@tauri-apps/api/core';
import type { SessionLogConfig } from './ssh.js';

/** What a tab records: commands only, or commands and output. */
export type HistoryLevel = 'commands' | 'output';
/** A session's choice; null or unset: the default from Settings. */
export type HistoryMode = 'off' | HistoryLevel;

export interface RunMeta {
	sessionId?: string | null;
	name: string;
	/** ssh, telnet, serial, local, wsl. */
	protocol: string;
	host: string;
	user: string;
}

export interface TapConfig {
	meta: RunMeta;
	level: HistoryLevel | null;
	/** Passwords this session uses, masked wherever they show up. */
	secrets: string[];
	/** The user's own patterns to mask, one regex each. */
	patterns: string[];
	/** The file log, for terminals that do not log on their own (all but SSH). */
	log: SessionLogConfig | null;
}

export interface TapStatus {
	recording: HistoryLevel | null;
	paused: boolean;
	logFile: string | null;
}

export interface HistoryQuery {
	text: string;
	sessionId?: string | null;
	host?: string | null;
	kind?: 'all' | 'commands' | 'output';
	from?: number | null;
	to?: number | null;
	limit?: number;
}

export interface HistoryHit {
	kind: 'command' | 'output';
	id: number;
	run: number;
	/** Milliseconds since 1970. */
	at: number;
	text: string;
	cwd: string | null;
	exit: number | null;
	redacted: boolean;
	sessionId: string | null;
	name: string;
	host: string;
	protocol: string;
}

export type HistoryForget =
	| { what: 'command'; id: number }
	| { what: 'output'; id: number }
	| { what: 'run'; id: number }
	| { what: 'host'; host: string }
	| { what: 'session'; sessionId: string }
	| { what: 'all' };

export interface HistoryRetention {
	outputDays: number;
	commandDays: number;
	maxMb: number;
}

export interface HistoryStats {
	runs: number;
	commands: number;
	outputs: number;
	bytes: number;
}

export function historyStart(stream: string, config: TapConfig): Promise<TapStatus> {
	return invoke<TapStatus>('history_start', { stream, config });
}

export function historyStop(stream: string): Promise<void> {
	return invoke('history_stop', { stream });
}

export function historyPause(stream: string, paused: boolean): Promise<TapStatus | null> {
	return invoke<TapStatus | null>('history_pause', { stream, paused });
}

export function historySearch(query: HistoryQuery): Promise<HistoryHit[]> {
	return invoke<HistoryHit[]>('history_search', { query });
}

export function historyTranscript(run: number): Promise<HistoryHit[]> {
	return invoke<HistoryHit[]>('history_transcript', { run });
}

export function historyForget(what: HistoryForget): Promise<void> {
	return invoke('history_forget', { what });
}

export function historyConfigure(retention: HistoryRetention): Promise<void> {
	return invoke('history_configure', { retention });
}

export function historyStats(): Promise<HistoryStats> {
	return invoke<HistoryStats>('history_stats');
}
