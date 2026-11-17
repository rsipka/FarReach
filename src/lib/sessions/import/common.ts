/**
 * What every session importer hands the import dialog: hosts in one shape,
 * whatever app they come from, plus the small readers several of them share
 * (INI and .reg files, Base64, MD5 and AES through WebCrypto).
 *
 * The RDM importer set this shape first; the others fill the same fields,
 * and add what their files keep and RDM's does not: passwords.
 *
 * Kept apart from the dialog so it can be tested in Node.
 */

import type { SessionKind } from '#lib/ipc/sessions.js';

export type ImportKind = Extract<SessionKind, 'ssh' | 'rdp' | 'vnc' | 'telnet'>;

export const DEFAULT_PORT: Record<ImportKind, number> = { ssh: 22, rdp: 3389, vnc: 5900, telnet: 23 };

/** One hop on the way to an SSH host. */
export interface ImportHop {
	host: string;
	port: number;
	/** Absent: the hop logs in as the host does. */
	username?: string;
	password?: string | null;
	keyPath?: string | null;
}

export interface ImportField {
	name: string;
	value: string;
	/** Masked until revealed. */
	hidden: boolean;
}

/** A login kept apart from the hosts, saved once as a Reach credential. */
export interface ImportCredential {
	key: string;
	name: string;
	username: string;
	domain: string | null;
	/** Null when the file does not hold it, or holds it in a form that cannot be read. */
	password?: string | null;
}

export interface ImportHostCredential {
	key: string;
	/** Inherited: the folder path it comes from, for a folder default. Null when the host links it itself. */
	from: string[] | null;
}

export interface ImportHost {
	/** Position in the file, unique within one file. */
	id: string;
	kind: ImportKind;
	name: string;
	host: string;
	port: number;
	username: string;
	/** Windows logon domain, RDP only. */
	domain: string | null;
	/** SSH only: a private key file, on the machine the other app ran on. */
	keyPath: string | null;
	/** The folder path, outermost first; empty at the top level. */
	folder: string[];
	/** SSH only: the jump hosts, outermost first. */
	jump: ImportHop[];
	/** The saved credential the host logs in with. */
	credential: ImportHostCredential | null;
	/** Telnet only: plain TCP with no Telnet. */
	raw: boolean;
	notes: string | null;
	fields: ImportField[];
	/** The host's own password, when the file holds one that could be read. */
	password?: string | null;
}

/** Why an entry in the file could not become a session. */
export type ImportSkipReason = 'no_host' | 'bad_port' | 'unsupported' | 'duplicate';

export interface ImportSkipped {
	label: string;
	reason: ImportSkipReason;
	/** For `unsupported`: the connection type, as the file names it. */
	type?: string;
}

export type ImportParseResult =
	| { ok: true; hosts: ImportHost[]; skipped: ImportSkipped[]; credentials: ImportCredential[]; notice?: ImportNotice }
	| { ok: false; error: 'bad_file'; detail: string }
	| { ok: false; error: 'encrypted' }
	| { ok: false; error: 'no_hosts' }
	/** The file is sealed with a password the person has to type; `wrong` when one was typed and did not fit. */
	| { ok: false; error: 'password'; wrong: boolean };

/** Something the dialog says above the list, such as why no passwords came along. */
export type ImportNotice = 'passwords_skipped' | 'passwords_keychain';

/** A trimmed string, or '' for anything that is not a string or a number. */
export function text(value: unknown): string {
	if (typeof value === 'string') return value.trim();
	if (typeof value === 'number' && Number.isFinite(value)) return String(value);
	return '';
}

/** A TCP port, or null when the value is not one. */
export function parsePort(value: string): number | null {
	const n = Number(value);
	return Number.isInteger(n) && n >= 1 && n <= 65535 ? n : null;
}

/** The port to use: the protocol's default when absent or 0, null when not a port. */
export function portOr(value: string, kind: ImportKind): number | null {
	const v = value.trim();
	return !v || v === '0' ? DEFAULT_PORT[kind] : parsePort(v);
}

/**
 * Hosts get the same key when they would be the same session; the second is
 * listed as skipped.
 */
export class Dedupe {
	private seen = new Set<string>();
	/** True the first time this session is seen. */
	add(h: Pick<ImportHost, 'kind' | 'host' | 'port' | 'username'>): boolean {
		const key = JSON.stringify([h.kind, h.host.toLowerCase(), h.port, h.username]);
		if (this.seen.has(key)) return false;
		this.seen.add(key);
		return true;
	}
}

/** A host with every optional field at its empty value. */
export function blankHost(id: string, kind: ImportKind, name: string, host: string, port: number): ImportHost {
	return { id, kind, name: name || host, host, port, username: '', domain: null, keyPath: null, folder: [], jump: [], credential: null, raw: false, notes: null, fields: [] };
}

/** "user@host" as some apps write it in the host field. */
export function splitUserHost(value: string): { user: string; host: string } {
	const at = value.lastIndexOf('@');
	return at > 0 ? { user: value.slice(0, at), host: value.slice(at + 1) } : { user: '', host: value };
}

/** A file's text whatever its encoding: UTF-16 (as `reg export` writes), UTF-8, or else Windows-1252. */
export function decodeFile(bytes: Uint8Array): string {
	if (bytes.length >= 2 && bytes[0] === 0xff && bytes[1] === 0xfe) return new TextDecoder('utf-16le').decode(bytes.subarray(2));
	if (bytes.length >= 2 && bytes[0] === 0xfe && bytes[1] === 0xff) return new TextDecoder('utf-16be').decode(bytes.subarray(2));
	try {
		return new TextDecoder('utf-8', { fatal: true }).decode(bytes).replace(/^﻿/, '');
	} catch {
		return new TextDecoder('windows-1252').decode(bytes);
	}
}

// ---------------------------------------------------------------------------
// INI and .reg
// ---------------------------------------------------------------------------

export interface IniSection {
	/** As written, without the brackets (a .reg key path for .reg files). */
	name: string;
	/** Values by name, in file order; names as written. */
	values: Map<string, string>;
}

/**
 * Sections and values of an INI file or a Windows .reg export. In a .reg
 * file, names and strings are quoted with `\\` and `\"` escaped, numbers are
 * `dword:` hex (given back in decimal), and binary values are left out.
 */
export function readIni(raw: string): IniSection[] {
	const sections: IniSection[] = [];
	let current: IniSection | null = null;
	const lines = raw.replace(/^﻿/, '').split(/\r\n|\r|\n/);
	for (let i = 0; i < lines.length; i++) {
		let line = lines[i];
		// A .reg value continued on the next line (long hex values).
		while (line.endsWith('\\') && /^"[^"]*"=hex/.test(line) && i + 1 < lines.length) line = line.slice(0, -1) + lines[++i].trim();
		const trimmed = line.trim();
		if (!trimmed || trimmed.startsWith(';') || trimmed.startsWith('#')) continue;
		if (trimmed.startsWith('[') && trimmed.endsWith(']')) {
			current = { name: trimmed.slice(1, -1), values: new Map() };
			sections.push(current);
			continue;
		}
		if (!current) continue;
		if (trimmed.startsWith('"')) {
			const m = /^"((?:[^"\\]|\\.)*)"\s*=\s*(.*)$/.exec(trimmed);
			if (!m) continue;
			const name = unescapeReg(m[1]);
			const value = m[2];
			if (value.startsWith('"')) {
				const s = /^"((?:[^"\\]|\\.)*)"/.exec(value);
				if (s) current.values.set(name, unescapeReg(s[1]));
			} else if (/^dword:[0-9a-fA-F]{1,8}$/.test(value)) {
				current.values.set(name, String(parseInt(value.slice(6), 16)));
			}
			continue;
		}
		const eq = trimmed.indexOf('=');
		if (eq <= 0) continue;
		current.values.set(trimmed.slice(0, eq).trim(), trimmed.slice(eq + 1));
	}
	return sections;
}

function unescapeReg(s: string): string {
	return s.replace(/\\(.)/g, '$1');
}

/** A value by name regardless of case. */
export function iniGet(section: IniSection, name: string): string {
	const direct = section.values.get(name);
	if (direct !== undefined) return direct;
	const lower = name.toLowerCase();
	for (const [k, v] of section.values) if (k.toLowerCase() === lower) return v;
	return '';
}

/** Undo `%XX` escapes (PuTTY session names, WinSCP's INI values). */
export function unpercent(s: string): string {
	return s.replace(/(%[0-9a-fA-F]{2})+/g, (run) => {
		const bytes = Uint8Array.from(run.split('%').slice(1), (h) => parseInt(h, 16));
		try {
			return new TextDecoder('utf-8', { fatal: true }).decode(bytes);
		} catch {
			return new TextDecoder('windows-1252').decode(bytes);
		}
	});
}

// ---------------------------------------------------------------------------
// Bytes and ciphers
// ---------------------------------------------------------------------------

export function fromBase64(s: string): Uint8Array | null {
	const clean = s.replace(/\s+/g, '');
	if (!/^[A-Za-z0-9+/]*={0,2}$/.test(clean)) return null;
	try {
		return Uint8Array.from(atob(clean), (c) => c.charCodeAt(0));
	} catch {
		return null;
	}
}

export function fromHex(s: string): Uint8Array {
	const out = new Uint8Array(s.length >> 1);
	for (let i = 0; i < out.length; i++) out[i] = parseInt(s.substr(i * 2, 2), 16);
	return out;
}

/** UTF-8 that must be text: null for bytes that are not UTF-8, or hold control characters. */
export function strictText(bytes: Uint8Array): string | null {
	try {
		const s = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
		// eslint-disable-next-line no-control-regex
		return /[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f]/.test(s) ? null : s;
	} catch {
		return null;
	}
}

function subtle(): SubtleCrypto {
	const c = globalThis.crypto?.subtle;
	if (!c) throw new Error('WebCrypto is not available');
	return c;
}

/** A plain ArrayBuffer copy, which WebCrypto's typings want. */
function buf(b: Uint8Array): ArrayBuffer {
	return b.slice().buffer as ArrayBuffer;
}

export async function sha512(data: Uint8Array): Promise<Uint8Array> {
	return new Uint8Array(await subtle().digest('SHA-512', buf(data)));
}

export async function pbkdf2(password: Uint8Array, salt: Uint8Array, iterations: number, bytes: number, hash: 'SHA-1' | 'SHA-512'): Promise<Uint8Array> {
	const key = await subtle().importKey('raw', buf(password), 'PBKDF2', false, ['deriveBits']);
	return new Uint8Array(await subtle().deriveBits({ name: 'PBKDF2', salt: buf(salt), iterations, hash }, key, bytes * 8));
}

/** AES-CBC with PKCS#7 padding; null when the key does not fit (bad padding). */
export async function aesCbcDecrypt(key: Uint8Array, iv: Uint8Array, data: Uint8Array): Promise<Uint8Array | null> {
	try {
		const k = await subtle().importKey('raw', buf(key), 'AES-CBC', false, ['decrypt']);
		return new Uint8Array(await subtle().decrypt({ name: 'AES-CBC', iv: buf(iv) }, k, buf(data)));
	} catch {
		return null;
	}
}

/** AES-GCM; null when the tag does not check out (wrong key, or a changed file). */
export async function aesGcmDecrypt(key: Uint8Array, iv: Uint8Array, data: Uint8Array, aad: Uint8Array): Promise<Uint8Array | null> {
	try {
		const k = await subtle().importKey('raw', buf(key), 'AES-GCM', false, ['decrypt']);
		return new Uint8Array(await subtle().decrypt({ name: 'AES-GCM', iv: buf(iv), additionalData: buf(aad), tagLength: 128 }, k, buf(data)));
	} catch {
		return null;
	}
}

/**
 * One AES block encrypted on its own (ECB), done as CBC with a zero IV,
 * which WebCrypto has: the first block out is the same.
 */
async function aesBlock(key: CryptoKey, block: Uint8Array): Promise<Uint8Array> {
	const out = await subtle().encrypt({ name: 'AES-CBC', iv: new ArrayBuffer(16) }, key, buf(block));
	return new Uint8Array(out, 0, 16);
}

/** AES in 8-bit CFB mode, which WebCrypto lacks, built a byte at a time on single blocks. */
export async function aesCfb8Decrypt(key: Uint8Array, iv: Uint8Array, data: Uint8Array): Promise<Uint8Array> {
	const k = await subtle().importKey('raw', buf(key), 'AES-CBC', false, ['encrypt']);
	const shift = iv.slice();
	const out = new Uint8Array(data.length);
	for (let i = 0; i < data.length; i++) {
		const stream = await aesBlock(k, shift);
		out[i] = data[i] ^ stream[0];
		shift.copyWithin(0, 1);
		shift[15] = data[i];
	}
	return out;
}

/** AES-ECB of one block, for ciphers that derive their IV that way. */
export async function aesEcbBlock(key: Uint8Array, block: Uint8Array): Promise<Uint8Array> {
	const k = await subtle().importKey('raw', buf(key), 'AES-CBC', false, ['encrypt']);
	return aesBlock(k, block);
}

/** MD5, which WebCrypto lacks; only old mRemote files need it, as a key. */
export function md5(data: Uint8Array): Uint8Array {
	const s = [7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21];
	const K = Array.from({ length: 64 }, (_, i) => Math.floor(Math.abs(Math.sin(i + 1)) * 2 ** 32) >>> 0);
	const len = data.length;
	const padded = new Uint8Array(((len + 8) >> 6) * 64 + 64);
	padded.set(data);
	padded[len] = 0x80;
	const view = new DataView(padded.buffer);
	view.setUint32(padded.length - 8, (len * 8) >>> 0, true);
	view.setUint32(padded.length - 4, Math.floor(len / 0x20000000), true);
	let a0 = 0x67452301,
		b0 = 0xefcdab89,
		c0 = 0x98badcfe,
		d0 = 0x10325476;
	for (let off = 0; off < padded.length; off += 64) {
		const M = Array.from({ length: 16 }, (_, i) => view.getUint32(off + i * 4, true));
		let a = a0,
			b = b0,
			c = c0,
			d = d0;
		for (let i = 0; i < 64; i++) {
			let f: number, g: number;
			if (i < 16) [f, g] = [(b & c) | (~b & d), i];
			else if (i < 32) [f, g] = [(d & b) | (~d & c), (5 * i + 1) % 16];
			else if (i < 48) [f, g] = [b ^ c ^ d, (3 * i + 5) % 16];
			else [f, g] = [c ^ (b | ~d), (7 * i) % 16];
			f = (f + a + K[i] + M[g]) >>> 0;
			a = d;
			d = c;
			c = b;
			b = (b + ((f << s[i]) | (f >>> (32 - s[i])))) >>> 0;
		}
		a0 = (a0 + a) >>> 0;
		b0 = (b0 + b) >>> 0;
		c0 = (c0 + c) >>> 0;
		d0 = (d0 + d) >>> 0;
	}
	const out = new Uint8Array(16);
	const ov = new DataView(out.buffer);
	[a0, b0, c0, d0].forEach((w, i) => ov.setUint32(i * 4, w, true));
	return out;
}
