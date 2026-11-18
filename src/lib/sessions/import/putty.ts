/**
 * Reading saved sessions out of PuTTY (and KiTTY).
 *
 * On Windows PuTTY keeps them in the registry, under
 * HKCU\Software\SimonTatham\PuTTY\Sessions: the dialog reads that directly,
 * or takes a .reg file exported from it (`reg export
 * HKCU\Software\SimonTatham\PuTTY\Sessions putty.reg`). On Linux and macOS
 * each session is a file in ~/.putty/sessions, which the app turns into the
 * same sections. Session names are %-escaped.
 *
 * PuTTY does not save login passwords. A proxy of the SSH kind (PuTTY 0.77
 * and later) is the session's jump host; its host may name another saved
 * session, which is then used for it. KiTTY's Folder value becomes the
 * folder.
 */

import { baudOf, blankHost, Dedupe, iniGet, portOr, serialHost, readIni, splitUserHost, unpercent, type IniSection, type ImportHop, type ImportHost, type ImportKind, type ImportParseResult, type ImportSkipped } from './common.ts';
import { splitAddress } from '../rdm-import.ts';

const KINDS: Record<string, { kind: ImportKind; raw?: boolean }> = {
	ssh: { kind: 'ssh' },
	telnet: { kind: 'telnet' },
	raw: { kind: 'telnet', raw: true }
};

/** PuTTY's SSH proxy kinds: port forwarding, a command, a subsystem. */
const SSH_PROXY = new Set(['6', '7', '8']);

/** The session name of a section, or null for other keys. */
function sessionName(section: IniSection): string | null {
	const m = /(?:^|\\)Sessions\\([^\\]+)$/i.exec(section.name);
	return m ? unpercent(m[1]) : null;
}

export function parsePutty(raw: string): ImportParseResult {
	const sections = readIni(raw);
	const sessions = new Map<string, IniSection>();
	for (const s of sections) {
		const name = sessionName(s);
		if (name !== null) sessions.set(name, s);
	}
	if (sessions.size === 0) return { ok: false, error: 'no_hosts' };

	const hosts: ImportHost[] = [];
	const skipped: ImportSkipped[] = [];
	const dedupe = new Dedupe();
	let index = 0;
	for (const [name, s] of sessions) {
		index++;
		const protocol = iniGet(s, 'Protocol').toLowerCase() || 'ssh';
		const hostValue = iniGet(s, 'HostName').trim();
		if (name === 'Default Settings' && !hostValue) continue;
		if (protocol === 'serial') {
			const serial = puttySerial(name, s, index);
			if (!serial) skipped.push({ label: name, reason: 'no_host' });
			else if (!dedupe.add(serial)) skipped.push({ label: name, reason: 'duplicate' });
			else hosts.push(serial);
			continue;
		}
		const cls = KINDS[protocol];
		if (!cls) {
			skipped.push({ label: name, reason: 'unsupported', type: protocol });
			continue;
		}
		const { user: inlineUser, host: hostPart } = splitUserHost(hostValue);
		const address = splitAddress(hostPart);
		if (!address.host) {
			skipped.push({ label: name, reason: 'no_host' });
			continue;
		}
		const port = portOr(iniGet(s, 'PortNumber') || address.port, cls.kind);
		if (port === null) {
			skipped.push({ label: name, reason: 'bad_port' });
			continue;
		}
		const host = blankHost(String(index), cls.kind, name, address.host, port);
		host.username = iniGet(s, 'UserName').trim() || inlineUser;
		if (!dedupe.add(host)) {
			skipped.push({ label: name, reason: 'duplicate' });
			continue;
		}
		host.raw = cls.raw === true;
		const folder = iniGet(s, 'Folder').trim();
		host.folder = folder && folder !== 'Default' ? folder.split(/[\\/]/).filter(Boolean) : [];
		if (cls.kind === 'ssh') {
			host.keyPath = iniGet(s, 'PublicKeyFile').trim() || null;
			const hop = proxyHop(s, sessions);
			if (hop) host.jump = [hop];
		}
		hosts.push(host);
	}
	if (hosts.length === 0 && skipped.length === 0) return { ok: false, error: 'no_hosts' };
	return { ok: true, hosts, skipped, credentials: [] };
}

/**
 * A serial session: SerialLine and its settings. PuTTY counts stop bits in
 * halves (2 = 1, 4 = 2) and has parities and flow controls Reacher CM
 * cannot set (mark, space, DSR/DTR): those keep the default.
 */
function puttySerial(name: string, s: IniSection, index: number): ImportHost | null {
	const line = iniGet(s, 'SerialLine').trim();
	if (!line) return null;
	const host = serialHost(String(index), name, line, {});
	const speed = baudOf(iniGet(s, 'SerialSpeed'));
	if (speed) host.serial!.baud_rate = speed;
	const bits = Number(iniGet(s, 'SerialDataBits'));
	if (bits >= 5 && bits <= 8) host.serial!.data_bits = bits;
	const halves = iniGet(s, 'SerialStopHalfbits');
	if (halves === '4') host.serial!.stop_bits = 2;
	else if (halves === '2') host.serial!.stop_bits = 1;
	const parity = ({ '0': 'none', '1': 'odd', '2': 'even' } as const)[iniGet(s, 'SerialParity') as '0' | '1' | '2'];
	if (parity) host.serial!.parity = parity;
	const flow = ({ '0': 'none', '1': 'software', '2': 'hardware' } as const)[iniGet(s, 'SerialFlowControl') as '0' | '1' | '2'];
	if (flow) host.serial!.flow_control = flow;
	const folder = iniGet(s, 'Folder').trim();
	host.folder = folder && folder !== 'Default' ? folder.split(/[\\/]/).filter(Boolean) : [];
	return host;
}

/** The SSH proxy as a jump host: a saved session by name, or a host. */
function proxyHop(s: IniSection, sessions: Map<string, IniSection>): ImportHop | null {
	if (!SSH_PROXY.has(iniGet(s, 'ProxyMethod'))) return null;
	const proxyHost = iniGet(s, 'ProxyHost').trim();
	if (!proxyHost) return null;
	const saved = sessions.get(proxyHost);
	if (saved) {
		const { user, host } = splitUserHost(iniGet(saved, 'HostName').trim());
		const address = splitAddress(host);
		if (address.host) {
			return {
				host: address.host,
				port: portOr(iniGet(saved, 'PortNumber') || address.port, 'ssh') ?? 22,
				username: iniGet(saved, 'UserName').trim() || user || undefined,
				keyPath: iniGet(saved, 'PublicKeyFile').trim() || null
			};
		}
	}
	const { user, host } = splitUserHost(proxyHost);
	const address = splitAddress(host);
	if (!address.host) return null;
	return {
		host: address.host,
		port: portOr(iniGet(s, 'ProxyPort') || address.port, 'ssh') ?? 22,
		username: iniGet(s, 'ProxyUsername').trim() || user || undefined,
		// PuTTY keeps the proxy password as plain text.
		password: iniGet(s, 'ProxyPassword') || null
	};
}
