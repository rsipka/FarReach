/**
 * Reading profiles out of Tabby's config.yaml (the app turns the YAML into
 * JSON first; see `import_yaml_to_json`).
 *
 * SSH and Telnet profiles become sessions, in folders made from Tabby's
 * profile groups (nested through parentGroupId; very old configs name the
 * group instead). An SSH profile's jumpHost names another profile, which
 * may have its own: the chain becomes the session's jump hosts.
 *
 * Passwords: with Tabby's vault on, they are vault secrets
 * (`ssh:password`, keyed by user, host and port), and with "encrypt config"
 * on, the profiles themselves are in the vault too. The vault is
 * AES-256-CBC with a key made by PBKDF2-SHA512 (100,000 rounds) from the
 * vault passphrase, which the dialog asks for. Without the vault Tabby
 * keeps passwords in the system keychain, which cannot be read from here.
 */

import {
	aesCbcDecrypt,
	blankHost,
	Dedupe,
	fromBase64,
	fromHex,
	pbkdf2,
	portOr,
	text,
	type ImportHop,
	type ImportHost,
	type ImportKind,
	type ImportParseResult,
	type ImportSkipped
} from './common.ts';
import { splitAddress } from '../rdm-import.ts';

type Obj = Record<string, unknown>;
const isObj = (v: unknown): v is Obj => typeof v === 'object' && v !== null && !Array.isArray(v);
const list = (v: unknown): Obj[] => (Array.isArray(v) ? v.filter(isObj) : []);

interface VaultContent {
	config?: Obj;
	secrets?: { type?: string; key?: Obj; value?: string }[];
}

/** Open Tabby's vault; null when the passphrase is wrong. */
export async function openTabbyVault(vault: Obj, passphrase: string): Promise<VaultContent | null> {
	const salt = fromHex(text(vault.keySalt));
	const iv = fromHex(text(vault.iv));
	const data = fromBase64(text(vault.contents));
	if (!data || iv.length !== 16) return null;
	const key = await pbkdf2(new TextEncoder().encode(passphrase), salt, 100000, 32, 'SHA-512');
	const plain = await aesCbcDecrypt(key, iv, data);
	if (!plain) return null;
	try {
		const content = JSON.parse(new TextDecoder().decode(plain));
		return isObj(content) ? (content as VaultContent) : null;
	} catch {
		return null;
	}
}

/** A key path from Tabby's `file://` URL; vault-held keys have none. */
function keyPathOf(options: Obj): string | null {
	for (const k of Array.isArray(options.privateKeys) ? options.privateKeys : []) {
		const s = text(k);
		if (!s || s.startsWith('vault://')) continue;
		if (!s.startsWith('file://')) return s;
		let path = s.slice('file://'.length);
		try {
			path = decodeURIComponent(path);
		} catch {
			// Keep it as written.
		}
		return /^\/[A-Za-z]:/.test(path) ? path.slice(1) : path;
	}
	return null;
}

/**
 * Read a Tabby config. `passphrase` is the vault's, when the dialog asked;
 * null means go on without the vault's passwords.
 */
export async function parseTabby(data: unknown, passphrase?: string | null): Promise<ImportParseResult> {
	if (!isObj(data)) return { ok: false, error: 'no_hosts' };
	let config: Obj = data;
	let secrets: VaultContent['secrets'] = [];
	const vault = isObj(data.vault) ? data.vault : null;
	const sealed = data.encrypted === true;
	if (vault) {
		if (passphrase === undefined || (passphrase === null && sealed)) return { ok: false, error: 'password', wrong: false };
		if (passphrase !== null) {
			const content = await openTabbyVault(vault, passphrase);
			if (!content) return { ok: false, error: 'password', wrong: true };
			if (sealed && isObj(content.config)) config = content.config;
			secrets = Array.isArray(content.secrets) ? content.secrets : [];
		}
	}

	const profiles = list(config.profiles);
	if (profiles.length === 0) return { ok: false, error: 'no_hosts' };

	const groups = new Map<string, Obj>();
	for (const g of list(config.groups)) groups.set(text(g.id), g);
	const folderOf = (group: string): string[] => {
		const out: string[] = [];
		const seen = new Set<string>();
		let g = groups.get(group);
		// Before Tabby kept group objects, `group` was the name itself.
		if (!g) return group ? [group] : [];
		while (g && !seen.has(text(g.id))) {
			seen.add(text(g.id));
			out.unshift(text(g.name) || '?');
			g = groups.get(text(g.parentGroupId));
		}
		return out;
	};
	const byId = new Map<string, Obj>();
	for (const p of profiles) if (text(p.id)) byId.set(text(p.id), p);

	const passwordOf = (user: string, host: string, port: number): string | null => {
		for (const s of secrets ?? []) {
			if (s.type !== 'ssh:password' || !isObj(s.key)) continue;
			const k = s.key;
			const kp = text(k.port);
			if (text(k.user) === user && text(k.host) === host && (!kp || Number(kp) === port)) return typeof s.value === 'string' ? s.value : null;
		}
		return null;
	};

	/** The jump chain to a profile, outermost first. */
	const chain = (options: Obj): ImportHop[] => {
		const hops: ImportHop[] = [];
		const seen = new Set<string>();
		let id = text(options.jumpHost);
		while (id && !seen.has(id)) {
			seen.add(id);
			const p = byId.get(id);
			const o = p && isObj(p.options) ? p.options : null;
			if (!o) break;
			const address = splitAddress(text(o.host));
			if (!address.host) break;
			const port = portOr(text(o.port) || address.port, 'ssh') ?? 22;
			const user = text(o.user);
			hops.unshift({ host: address.host, port, username: user || undefined, keyPath: keyPathOf(o), password: user ? passwordOf(user, address.host, port) : null });
			id = text(o.jumpHost);
		}
		return hops;
	};

	const hosts: ImportHost[] = [];
	const skipped: ImportSkipped[] = [];
	const dedupe = new Dedupe();
	let index = 0;
	let anySsh = false;
	for (const p of profiles) {
		index++;
		const type = text(p.type);
		if (p.isBuiltin === true || p.isTemplate === true) continue;
		const name = text(p.name) || `#${index}`;
		const kind: ImportKind | null = type === 'ssh' ? 'ssh' : type === 'telnet' ? 'telnet' : null;
		if (!kind) {
			skipped.push({ label: name, reason: 'unsupported', type: type || '?' });
			continue;
		}
		const options = isObj(p.options) ? p.options : {};
		const address = splitAddress(text(options.host));
		if (!address.host) {
			skipped.push({ label: name, reason: 'no_host' });
			continue;
		}
		const port = portOr(text(options.port) || address.port, kind);
		if (port === null) {
			skipped.push({ label: name, reason: 'bad_port' });
			continue;
		}
		const host = blankHost(String(index), kind, name, address.host, port);
		host.username = kind === 'ssh' ? text(options.user) : '';
		if (!dedupe.add(host)) {
			skipped.push({ label: name, reason: 'duplicate' });
			continue;
		}
		host.folder = folderOf(text(p.group));
		if (kind === 'ssh') {
			anySsh = true;
			host.keyPath = keyPathOf(options);
			host.jump = chain(options);
			host.password = (typeof options.password === 'string' && options.password) || (host.username ? passwordOf(host.username, address.host, port) : null);
		}
		hosts.push(host);
	}
	if (hosts.length === 0 && skipped.length === 0) return { ok: false, error: 'no_hosts' };
	const notice = !vault && anySsh ? 'passwords_keychain' : vault && passphrase === null ? 'passwords_skipped' : undefined;
	return { ok: true, hosts, skipped, credentials: [], notice };
}
