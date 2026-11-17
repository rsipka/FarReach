/**
 * Reading sessions out of MobaXterm: a sessions export (.mxtsessions), the
 * portable edition's MobaXterm.ini, or the installed edition's
 * MobaXterm.ini together with its registry key (HKCU\Software\Mobatek\
 * MobaXterm, where it keeps passwords), which the dialog reads on Windows.
 *
 * Sessions sit in [Bookmarks], [Bookmarks_1] and so on, one section per
 * folder (SubRep, levels split by a backslash). Each line is
 * `name=#icon#type%host%port%user%...#font settings#...#comment#...`; the
 * fields after the type depend on it. SSH, Telnet, RDP and VNC become
 * sessions; an SSH session's gateway list (fields 8 to 10, hops split by
 * __PIPE__) becomes its jump hosts.
 *
 * Passwords are in [Passwords] (or the P registry key) by
 * `proto port:user@host` or `user@host`, and named credentials (a session
 * whose user is `[name]`) in [Credentials] (or C), as `user:password`.
 * With a master password set (MobaXterm asks for one before saving
 * passwords), each is AES-256-CFB8 encrypted with the first 32 bytes of the
 * SHA-512 of the master password, the IV being the key's encryption of a
 * zero block; the dialog asks for it. Without one (older versions), they are
 * scrambled with a key made from the SessionP value, which is read back here.
 * Exports (.mxtsessions) hold no passwords.
 */

import { splitAddress } from '../rdm-import.ts';
import {
	aesCfb8Decrypt,
	aesEcbBlock,
	blankHost,
	Dedupe,
	fromBase64,
	iniGet,
	portOr,
	readIni,
	sha512,
	strictText,
	type IniSection,
	type ImportCredential,
	type ImportHop,
	type ImportHost,
	type ImportKind,
	type ImportParseResult,
	type ImportSkipped
} from './common.ts';

const TYPES: Record<string, string> = {
	'0': 'SSH',
	'1': 'Telnet',
	'2': 'Rlogin',
	'3': 'XDMCP',
	'4': 'RDP',
	'5': 'VNC',
	'6': 'FTP',
	'7': 'SFTP',
	'8': 'Serial',
	'9': 'File',
	'10': 'Shell',
	'11': 'Browser',
	'12': 'Mosh',
	'13': 'S3',
	'14': 'WSL'
};
const KINDS: Record<string, ImportKind> = { '0': 'ssh', '1': 'telnet', '4': 'rdp', '5': 'vnc' };

/** MobaXterm's escapes for characters its line format uses. */
export function mobaUnescape(s: string): string {
	return s
		.replace(/__PIPE__/g, '|')
		.replace(/__DBLQUO__/g, '"')
		.replace(/__PTVIRG__/g, ';')
		.replace(/__DIEZE__/g, '#')
		.replace(/__PERCENT__/g, '%')
		.replace(/_CurrentDrive_/g, 'C:')
		.replace(/_ProfileDir_/g, '~')
		.replace(/_DesktopDir_/g, '~/Desktop')
		.trim();
}

function field(fields: string[], i: number): string {
	const v = mobaUnescape(fields[i] ?? '');
	return v === '<none>' || v === '<default>' ? '' : v;
}

/** The last path part of a registry key or INI section name. */
function leaf(section: IniSection): string {
	return section.name.split('\\').pop()!.toLowerCase();
}

function isMobaKey(section: IniSection, name: string | null): boolean {
	const m = /\\Mobatek\\MobaXterm(?:\\([^\\]+))?$/i.exec(section.name);
	if (!m) return name !== null && section.name.toLowerCase() === name;
	return (m[1]?.toLowerCase() ?? null) === name;
}

// ---------------------------------------------------------------------------
// Passwords
// ---------------------------------------------------------------------------

/** The scrambling of versions without a master password, keyed by SessionP. */
export function mobaDecryptLegacy(cipher: string, sessionP: string): string | null {
	let s = sessionP;
	while (s.length < 20) s += s;
	const space = [s.toUpperCase(), s.toUpperCase(), s.toLowerCase(), s.toLowerCase()];
	const alphabet = '0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz+/';
	const key = [...'0d5e9n1348/U2+67'];
	for (let i = 0; i < key.length; i++) {
		const b = space[(i + 1) % space.length][i];
		if (!key.includes(b) && alphabet.includes(b)) key[i] = b;
	}
	const ct = [...cipher].filter((c) => key.includes(c));
	if (ct.length % 2 !== 0) return null;
	const bytes = new Uint8Array(ct.length / 2);
	for (let i = 0; i < ct.length; i += 2) {
		const l = key.indexOf(ct[i]);
		key.unshift(key.pop()!);
		const h = key.indexOf(ct[i + 1]);
		key.unshift(key.pop()!);
		bytes[i / 2] = 16 * h + l;
	}
	return strictText(bytes);
}

/** A decryptor for master-password values. */
export async function mobaMasterDecryptor(master: string): Promise<(cipher: string) => Promise<string | null>> {
	const key = (await sha512(new TextEncoder().encode(master))).slice(0, 32);
	const iv = await aesEcbBlock(key, new Uint8Array(16));
	return async (cipher: string) => {
		const data = fromBase64(cipher);
		if (!data || data.length === 0) return null;
		return strictText(await aesCfb8Decrypt(key, iv, data));
	};
}

// ---------------------------------------------------------------------------

export async function parseMobaXterm(raw: string, password?: string | null): Promise<ImportParseResult> {
	const sections = readIni(raw);
	const bookmarks = sections.filter((s) => /^bookmarks(_\d+)?$/.test(leaf(s)) && !/\\/.test(s.name));
	if (bookmarks.length === 0) return { ok: false, error: 'no_hosts' };

	const merged = (name: string): Map<string, string> => {
		const out = new Map<string, string>();
		for (const s of sections) if (isMobaKey(s, name)) for (const [k, v] of s.values) out.set(k, v);
		return out;
	};
	const passwords = merged('passwords');
	for (const [k, v] of merged('p')) passwords.set(k, v);
	const credentialValues = merged('credentials');
	for (const [k, v] of merged('c')) credentialValues.set(k, v);
	const hasSecrets = passwords.size > 0 || credentialValues.size > 0;
	const sessionP = iniGet({ name: '', values: merged('misc') }, 'SessionP') || iniGet({ name: '', values: new Map([...sections.filter((s) => isMobaKey(s, null)).flatMap((s) => [...s.values])]) }, 'SessionP');
	const masterMode = merged('sesspass').size > 0 || merged('m').size > 0 || (hasSecrets && !sessionP);

	let decrypt: ((cipher: string) => Promise<string | null>) | null = null;
	if (hasSecrets) {
		if (masterMode) {
			if (password === undefined) return { ok: false, error: 'password', wrong: false };
			if (password !== null) {
				const d = await mobaMasterDecryptor(password);
				// A wrong master password turns every value into noise.
				const sample = [...passwords.values(), ...[...credentialValues.values()].map((v) => v.slice(v.indexOf(':') + 1))].filter(Boolean);
				let good = 0;
				for (const v of sample) if ((await d(v)) !== null) good++;
				if (sample.length > 0 && good === 0) return { ok: false, error: 'password', wrong: true };
				decrypt = d;
			}
		} else {
			decrypt = async (cipher) => mobaDecryptLegacy(cipher, sessionP);
		}
	}
	let unreadable = hasSecrets && decrypt === null;
	const open = async (cipher: string | undefined): Promise<string | null> => {
		if (!cipher || !decrypt) return null;
		const plain = await decrypt(cipher);
		if (plain === null) unreadable = true;
		return plain;
	};

	const credentials = new Map<string, ImportCredential>();
	const credentialFor = async (name: string): Promise<ImportCredential | null> => {
		const key = `moba:${name}`;
		const known = credentials.get(key);
		if (known) return known;
		const value = [...credentialValues].find(([k]) => k.toLowerCase() === name.toLowerCase())?.[1];
		if (value === undefined) return null;
		const colon = value.indexOf(':');
		const username = colon >= 0 ? value.slice(0, colon) : value;
		const cred: ImportCredential = { key, name, username, domain: null, password: colon >= 0 ? await open(value.slice(colon + 1)) : null };
		credentials.set(key, cred);
		return cred;
	};
	const passwordFor = (proto: string, port: number, user: string, host: string): string | undefined => {
		for (const name of [`${proto}${port}:${user}@${host}`, `${user}@${host}`]) {
			const hit = [...passwords].find(([k]) => k.toLowerCase() === name.toLowerCase());
			if (hit) return hit[1];
		}
		return undefined;
	};

	const hosts: ImportHost[] = [];
	const skipped: ImportSkipped[] = [];
	const dedupe = new Dedupe();
	let index = 0;
	for (const section of bookmarks) {
		const folder = iniGet(section, 'SubRep').split('\\').map((p) => p.trim()).filter(Boolean);
		for (const [rawName, value] of section.values) {
			if (/^(SubRep|ImgNum)$/i.test(rawName)) continue;
			index++;
			const name = mobaUnescape(rawName);
			const parts = value.trim().split('#');
			const at = parts.findIndex((p, i) => /^\d+$/.test(p) && (parts[i + 1] ?? '').includes('%'));
			if (at < 0) {
				skipped.push({ label: name, reason: 'unsupported', type: '?' });
				continue;
			}
			const fields = parts[at + 1].split('%');
			const type = fields[0];
			const kind = KINDS[type];
			if (!kind) {
				skipped.push({ label: name, reason: 'unsupported', type: TYPES[type] ?? type });
				continue;
			}
			const address = splitAddress(field(fields, 1));
			if (!address.host) {
				skipped.push({ label: name, reason: 'no_host' });
				continue;
			}
			const port = portOr(field(fields, 2) || address.port, kind);
			if (port === null) {
				skipped.push({ label: name, reason: 'bad_port' });
				continue;
			}
			let user = kind === 'ssh' || kind === 'rdp' ? field(fields, 3) : '';
			const credName = /^\[(.+)\]$/.exec(user)?.[1];
			const cred = credName ? await credentialFor(credName) : null;
			if (credName) user = cred?.username ?? '';
			const host = blankHost(String(index), kind, name, address.host, port);
			host.username = user;
			if (!dedupe.add(host)) {
				skipped.push({ label: name, reason: 'duplicate' });
				continue;
			}
			host.folder = folder;
			host.notes = mobaUnescape(parts[at + 4] ?? '') || null;
			if (cred) host.credential = { key: cred.key, from: null };
			else if (user) host.password = await open(passwordFor(TYPES[type].toLowerCase(), port, user, address.host));
			if (kind === 'ssh') {
				host.keyPath = field(fields, 14) || null;
				host.jump = gateways(fields[8], fields[9], fields[10]);
				for (const hop of host.jump) {
					const saved = hop.username ? await open(passwordFor('ssh', hop.port, hop.username, hop.host)) : null;
					if (saved) hop.password = saved;
				}
			}
			hosts.push(host);
		}
	}
	if (hosts.length === 0 && skipped.length === 0) return { ok: false, error: 'no_hosts' };
	return { ok: true, hosts, skipped, credentials: [...credentials.values()], notice: unreadable ? 'passwords_skipped' : undefined };
}

/** The SSH gateway hops, from three parallel __PIPE__ lists. */
function gateways(hostList = '', portList = '', userList = ''): ImportHop[] {
	const hostsIn = hostList.split('__PIPE__');
	const ports = portList.split('__PIPE__');
	const users = userList.split('__PIPE__');
	const out: ImportHop[] = [];
	hostsIn.forEach((h, i) => {
		const address = splitAddress(mobaUnescape(h));
		if (!address.host || address.host === '<none>') return;
		const user = mobaUnescape(users[i] ?? '');
		out.push({ host: address.host, port: portOr(mobaUnescape(ports[i] ?? '') || address.port, 'ssh') ?? 22, username: user && user !== '<default>' ? user : undefined });
	});
	return out;
}
