/**
 * Reading connections out of an mRemoteNG (or the older mRemote) connection
 * file, confCons.xml, or an XML export of it.
 *
 * Every connection is a `Node` element, with all its settings as attributes;
 * a `Type="Container"` node is a folder. A connection can inherit its
 * username, password and domain from the folder it sits in
 * (`InheritUsername="true"` and so on): a folder login used that way is
 * saved once as a Reach credential and set on the folder.
 *
 * Passwords are encrypted with a key made from a password. By default that
 * is mRemoteNG's built-in "mR3m", so they can be read; a file protected with
 * a password of the person's own asks for it. Two ciphers exist:
 *
 * - Older files (no EncryptionEngine attribute): AES-128-CBC keyed with the
 *   MD5 of the password, the IV in front of the ciphertext.
 * - mRemoteNG 1.75 and later: AES-256-GCM keyed with PBKDF2-HMAC-SHA1 of the
 *   password (KdfIterations rounds, 1000 by default); each value is
 *   salt (16) + nonce (16) + ciphertext and tag, with the salt as associated
 *   data. CCM and EAX modes exist too but are rare; their passwords are left
 *   out.
 *
 * The root's Protected attribute holds "ThisIsNotProtected" (default key)
 * or "ThisIsProtected" (own password), encrypted the same way: it tells
 * whether a password is right. With FullFileEncryption the whole tree is
 * one encrypted value in the root's text.
 */

import { parseXml, plainNotes, splitAddress, type XmlElement } from '../rdm-import.ts';
import {
	aesCbcDecrypt,
	aesGcmDecrypt,
	blankHost,
	Dedupe,
	fromBase64,
	md5,
	pbkdf2,
	portOr,
	strictText,
	type ImportCredential,
	type ImportHost,
	type ImportKind,
	type ImportParseResult,
	type ImportSkipped
} from './common.ts';

export const MREMOTE_DEFAULT_PASSWORD = 'mR3m';

interface Cipher {
	mode: 'cbc' | 'gcm' | 'other';
	iterations: number;
}

/** The value of an attribute regardless of case, trimmed. */
function attr(el: XmlElement, name: string): string {
	if (name in el.attrs) return el.attrs[name].trim();
	const lower = name.toLowerCase();
	for (const [k, v] of Object.entries(el.attrs)) if (k.toLowerCase() === lower) return v.trim();
	return '';
}

function isTrue(el: XmlElement, name: string): boolean {
	return attr(el, name).toLowerCase() === 'true';
}

/** Decrypt one value; null when the password does not fit or the value is not one. */
export async function mremoteDecrypt(value: string, password: string, cipher: Cipher): Promise<string | null> {
	const data = fromBase64(value);
	if (!data) return null;
	if (cipher.mode === 'cbc') {
		if (data.length < 32 || data.length % 16 !== 0) return null;
		const plain = await aesCbcDecrypt(md5(new TextEncoder().encode(password)), data.subarray(0, 16), data.subarray(16));
		return plain ? strictText(plain) : null;
	}
	if (cipher.mode === 'gcm') {
		if (data.length < 48) return null;
		const salt = data.subarray(0, 16);
		// BouncyCastle's PKCS#5 password bytes: each character's low byte.
		const pw = Uint8Array.from(password, (c) => c.charCodeAt(0) & 0xff);
		const key = await pbkdf2(pw, salt, cipher.iterations, 32, 'SHA-1');
		const plain = await aesGcmDecrypt(key, data.subarray(16, 32), data.subarray(32), salt);
		return plain ? strictText(plain) : null;
	}
	return null;
}

const KINDS: Record<string, { kind: ImportKind; raw?: boolean }> = {
	rdp: { kind: 'rdp' },
	vnc: { kind: 'vnc' },
	ssh1: { kind: 'ssh' },
	ssh2: { kind: 'ssh' },
	telnet: { kind: 'telnet' },
	raw: { kind: 'telnet', raw: true }
};

interface Login {
	username: string;
	password: string | null;
	domain: string;
}

/**
 * Read an mRemoteNG file. `password` is the file's own password, when the
 * dialog asked for one (null: go on without passwords); without it the
 * built-in one is tried.
 */
export async function parseMremoteNg(raw: string, password?: string | null): Promise<ImportParseResult> {
	const trimmed = raw.replace(/^﻿/, '').trim();
	if (!trimmed.startsWith('<')) return { ok: false, error: 'no_hosts' };
	let doc: XmlElement;
	try {
		doc = parseXml(trimmed);
	} catch (e) {
		return { ok: false, error: 'bad_file', detail: e instanceof Error ? e.message : String(e) };
	}
	const root = doc.children.find((c) => /(^|:)connections$/i.test(c.name));
	if (!root) return { ok: false, error: 'no_hosts' };

	const engine = attr(root, 'EncryptionEngine');
	const mode = attr(root, 'BlockCipherMode').toLowerCase();
	const cipher: Cipher = {
		mode: !engine ? 'cbc' : engine.toLowerCase() === 'aes' && (mode === '' || mode === 'gcm') ? 'gcm' : 'other',
		iterations: Number(attr(root, 'KdfIterations')) || 1000
	};

	// Which password opens the file: the built-in one, or the person's.
	let key: string | null = MREMOTE_DEFAULT_PASSWORD;
	const protectedValue = attr(root, 'Protected');
	if (protectedValue && cipher.mode !== 'other') {
		const builtIn = await mremoteDecrypt(protectedValue, MREMOTE_DEFAULT_PASSWORD, cipher);
		if (builtIn !== 'ThisIsNotProtected') {
			if (password === undefined) return { ok: false, error: 'password', wrong: false };
			if (password === null) {
				// Go on without the passwords, unless they are all the file holds.
				if (isTrue(root, 'FullFileEncryption')) return { ok: false, error: 'password', wrong: false };
				key = null;
			} else {
				const mine = await mremoteDecrypt(protectedValue, password, cipher);
				if (mine === null) return { ok: false, error: 'password', wrong: true };
				key = password;
			}
		}
	} else if (cipher.mode === 'other') key = null;

	let top = root;
	if (isTrue(root, 'FullFileEncryption')) {
		const inner = key === null ? null : await mremoteDecrypt(root.text.trim(), key, cipher);
		if (inner === null) return { ok: false, error: 'encrypted' };
		try {
			top = { ...root, children: parseXml(`<x>${inner}</x>`).children[0].children };
		} catch (e) {
			return { ok: false, error: 'bad_file', detail: e instanceof Error ? e.message : String(e) };
		}
	}

	const hosts: ImportHost[] = [];
	const skipped: ImportSkipped[] = [];
	const credentials = new Map<string, ImportCredential>();
	const dedupe = new Dedupe();
	let unreadable = false;
	let index = 0;

	const decrypt = async (value: string): Promise<string | null> => {
		if (!value) return null;
		const plain = key === null ? null : await mremoteDecrypt(value, key, cipher);
		if (plain === null) unreadable = true;
		return plain || null;
	};

	/** A node's own login, with the parts it inherits taken from `parent`. */
	const loginOf = async (el: XmlElement, parent: Login | null): Promise<Login> => ({
		username: isTrue(el, 'InheritUsername') && parent ? parent.username : attr(el, 'Username'),
		password: isTrue(el, 'InheritPassword') && parent ? parent.password : await decrypt(attr(el, 'Password')),
		domain: isTrue(el, 'InheritDomain') && parent ? parent.domain : attr(el, 'Domain')
	});

	/**
	 * `owner` is the folder path whose own login the parent's is (the
	 * outermost folder that set it), for a folder default credential.
	 */
	const walk = async (nodes: XmlElement[], path: string[], parent: Login | null, owner: string[] | null): Promise<void> => {
		for (const el of nodes) {
			if (!/^node$/i.test(el.name)) continue;
			const name = attr(el, 'Name');
			const type = attr(el, 'Type').toLowerCase();
			if (type === 'container') {
				const login = await loginOf(el, parent);
				const inherits = isTrue(el, 'InheritUsername') && parent !== null;
				const sub = [...path, name || '?'];
				await walk(el.children, sub, login, inherits ? owner : login.username ? sub : null);
				continue;
			}
			if (type !== 'connection') continue;
			index++;
			const label = name || `#${index}`;
			const protocol = attr(el, 'Protocol');
			const cls = KINDS[protocol.toLowerCase()];
			if (!cls) {
				skipped.push({ label, reason: 'unsupported', type: protocol || '?' });
				continue;
			}
			const address = splitAddress(attr(el, 'Hostname'));
			if (!address.host) {
				skipped.push({ label, reason: 'no_host' });
				continue;
			}
			const port = portOr(attr(el, 'Port') || address.port, cls.kind);
			if (port === null) {
				skipped.push({ label, reason: 'bad_port' });
				continue;
			}
			const login = await loginOf(el, parent);
			const username = cls.kind === 'vnc' ? '' : login.username;
			const host = blankHost(String(index), cls.kind, name, address.host, port);
			host.username = username;
			if (!dedupe.add(host)) {
				skipped.push({ label, reason: 'duplicate' });
				continue;
			}
			host.folder = path;
			host.raw = cls.raw === true;
			host.domain = cls.kind === 'rdp' ? login.domain || null : null;
			host.notes = plainNotes(attr(el, 'Descr')) || null;
			// The whole login from a folder: saved once, as that folder's credential.
			const fromFolder = owner && parent && isTrue(el, 'InheritUsername') && (isTrue(el, 'InheritPassword') || !attr(el, 'Password'));
			if (fromFolder && username && cls.kind !== 'vnc') {
				const credKey = `folder:${owner.join('\\')}`;
				if (!credentials.has(credKey)) {
					credentials.set(credKey, { key: credKey, name: owner.join(' / '), username: parent.username, domain: parent.domain || null, password: parent.password });
				}
				host.credential = { key: credKey, from: owner };
			} else {
				host.password = login.password;
			}
			hosts.push(host);
		}
	};
	await walk(top.children, [], null, null);

	if (hosts.length === 0 && skipped.length === 0) return { ok: false, error: 'no_hosts' };
	return { ok: true, hosts, skipped, credentials: [...credentials.values()], notice: unreadable ? 'passwords_skipped' : undefined };
}
