/**
 * Reading saved sites out of WinSCP.
 *
 * WinSCP keeps them in the registry (HKCU\Software\Martin Prikryl\WinSCP 2,
 * which the dialog reads directly on Windows, or a .reg export of it) or, in
 * portable mode, in WinSCP.ini. Both have one key or section per site,
 * `Sessions\<folder>/<name>`, %-escaped. In the INI file the values are
 * %-escaped too.
 *
 * SFTP and SCP sites become SSH sessions (Reacher CM browses files over the
 * SSH session); FTP, WebDAV and S3 sites are skipped. A site's "Tunnel"
 * (connect through an SSH tunnel) becomes its jump host.
 *
 * Passwords: without a master password WinSCP only obfuscates them, and
 * they are read back here. With a master password set they are encrypted
 * with it and are left out (the dialog says so).
 */

import { splitAddress } from '../rdm-import.ts';
import { blankHost, Dedupe, iniGet, parsePort, readIni, unpercent, type IniSection, type ImportHost, type ImportParseResult, type ImportSkipped } from './common.ts';

const PWALG_MAGIC = 0xa3;
const PWALG_FLAG = 0xff;

/**
 * Undo WinSCP's password obfuscation: hex pairs, each byte XORed with 0xA3
 * and inverted; a length, a run of padding to skip, then the password,
 * which (in the usual form) starts with the user name and host name.
 * Null when the value does not decode to that.
 */
export function winscpPassword(encrypted: string, username: string, hostname: string): string | null {
	if (!/^[0-9A-Fa-f]*$/.test(encrypted) || encrypted.length < 6) return null;
	let pos = 0;
	const next = (): number => {
		if (pos + 2 > encrypted.length) throw new Error('short');
		const a = parseInt(encrypted.substr(pos, 2), 16);
		pos += 2;
		return ~(a ^ PWALG_MAGIC) & 0xff;
	};
	try {
		const flag = next();
		let length: number;
		if (flag === PWALG_FLAG) {
			next();
			length = next();
		} else length = flag;
		const skip = next();
		pos += skip * 2;
		const bytes = new Uint8Array(length);
		for (let i = 0; i < length; i++) bytes[i] = next();
		let s: string;
		try {
			s = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
		} catch {
			return null;
		}
		if (flag === PWALG_FLAG) {
			const key = username + hostname;
			if (!s.startsWith(key)) return null;
			s = s.slice(key.length);
		}
		return s;
	} catch {
		return null;
	}
}

const SSH_PROTOCOLS = new Set(['0', '1', '2']);
const PROTOCOL_NAMES: Record<string, string> = { '5': 'FTP', '6': 'WebDAV', '7': 'S3' };

function siteName(section: IniSection): string | null {
	const m = /(?:^|\\)Sessions\\([^\\]+)$/i.exec(section.name);
	return m ? m[1] : null;
}

export function parseWinScp(raw: string): ImportParseResult {
	const sections = readIni(raw);
	const isReg = sections.some((s) => /^HKEY_/i.test(s.name));
	const get = (s: IniSection, name: string): string => {
		const v = iniGet(s, name);
		return isReg ? v : unpercent(v);
	};
	const masterPassword = sections.some((s) => /(?:^|\\)Configuration\\Security$/i.test(s.name) && iniGet(s, 'UseMasterPassword') === '1');

	const hosts: ImportHost[] = [];
	const skipped: ImportSkipped[] = [];
	const dedupe = new Dedupe();
	let any = false;
	let unreadable = masterPassword;
	let index = 0;
	for (const s of sections) {
		const escaped = siteName(s);
		if (escaped === null) continue;
		any = true;
		index++;
		const path = unpercent(escaped).split('/').filter(Boolean);
		const name = path.pop() ?? '';
		if ((name === 'Default Settings' && path.length === 0) || iniGet(s, 'IsWorkspace') === '1') continue;
		const label = [...path, name].join(' / ');
		const protocol = iniGet(s, 'FSProtocol') || '2';
		if (!SSH_PROTOCOLS.has(protocol)) {
			skipped.push({ label, reason: 'unsupported', type: PROTOCOL_NAMES[protocol] ?? protocol });
			continue;
		}
		const address = splitAddress(get(s, 'HostName'));
		if (!address.host) {
			skipped.push({ label, reason: 'no_host' });
			continue;
		}
		const portText = iniGet(s, 'PortNumber') || address.port;
		const port = portText ? parsePort(portText) : 22;
		if (port === null) {
			skipped.push({ label, reason: 'bad_port' });
			continue;
		}
		const host = blankHost(String(index), 'ssh', name, address.host, port);
		host.username = get(s, 'UserName');
		if (!dedupe.add(host)) {
			skipped.push({ label, reason: 'duplicate' });
			continue;
		}
		host.folder = path;
		host.keyPath = get(s, 'PublicKeyFile') || null;
		host.notes = get(s, 'Note') || null;
		const stored = iniGet(s, 'Password');
		if (stored && !masterPassword) {
			host.password = winscpPassword(stored, host.username, address.host);
			if (host.password === null) unreadable = true;
		}
		if (iniGet(s, 'Tunnel') === '1') {
			const tunnel = splitAddress(get(s, 'TunnelHostName'));
			if (tunnel.host) {
				const user = get(s, 'TunnelUserName');
				const tunnelPassword = iniGet(s, 'TunnelPassword');
				host.jump = [
					{
						host: tunnel.host,
						port: parsePort(iniGet(s, 'TunnelPortNumber') || tunnel.port) ?? 22,
						username: user || undefined,
						keyPath: get(s, 'TunnelPublicKeyFile') || null,
						password: tunnelPassword && !masterPassword ? winscpPassword(tunnelPassword, user, tunnel.host) : null
					}
				];
			}
		}
		hosts.push(host);
	}
	if (!any || (hosts.length === 0 && skipped.length === 0)) return { ok: false, error: 'no_hosts' };
	return { ok: true, hosts, skipped, credentials: [], notice: unreadable ? 'passwords_skipped' : undefined };
}
