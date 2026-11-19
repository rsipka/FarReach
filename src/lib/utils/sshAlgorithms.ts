/**
 * The SSH algorithms choice in the session editor, as ssh_config lines.
 *
 * The lines in the session's SSH options stay the one source of truth: the
 * choice reads them (Default when no algorithm line is set, Compatible when
 * they are exactly the old-device set, Custom otherwise) and writes them
 * back. Lists are read the way OpenSSH reads them ("+", "-", "^" or plain),
 * as apply.rs `assemble` does.
 */

/** One algorithm a session can offer (apply.rs `CatalogEntry`). */
export interface CatalogEntry {
	name: string;
	default: boolean;
	weak: string | null;
}

/** apply.rs `CatalogGroup`. */
export interface CatalogGroup {
	keyword: string;
	algorithms: CatalogEntry[];
}

export type AlgorithmMode = 'default' | 'compatible' | 'custom';

/** Keywords this choice writes, by their lower-case spelling. */
const ALGO_KEYWORDS: Record<string, string> = {
	kexalgorithms: 'KexAlgorithms',
	hostkeyalgorithms: 'HostKeyAlgorithms',
	ciphers: 'Ciphers',
	macs: 'MACs',
	pubkeyacceptedalgorithms: 'PubkeyAcceptedAlgorithms',
	pubkeyacceptedkeytypes: 'PubkeyAcceptedAlgorithms'
};

export const PUBKEY = 'PubkeyAcceptedAlgorithms';

/**
 * What old switches, routers, NAS boxes and appliances still need: SHA-1
 * key exchange, CBC ciphers, SHA-1 and MD5 MACs, and SHA-1 RSA signatures
 * for key logins. Reach's defaults stay first, so a modern server still
 * gets them.
 */
export const COMPATIBLE_LINES: string[] = [
	'KexAlgorithms +diffie-hellman-group-exchange-sha1,diffie-hellman-group14-sha1,diffie-hellman-group1-sha1',
	'Ciphers +aes128-cbc,aes192-cbc,aes256-cbc,3des-cbc',
	'MACs +hmac-sha1,hmac-sha1-96,hmac-md5',
	'PubkeyAcceptedAlgorithms +ssh-rsa'
];

/** A line's keyword (as written) and value; `null` for a blank line or comment. */
export function parseLine(line: string): { keyword: string; value: string } | null {
	const t = line.trim();
	if (!t || t.startsWith('#')) return null;
	const m = /^([A-Za-z0-9]+)(?:\s*=\s*|\s+)(.*)$/.exec(t);
	if (!m) return { keyword: t, value: '' };
	return { keyword: m[1], value: m[2].trim().replace(/^"(.*)"$/, '$1') };
}

/** The canonical keyword of an algorithm line, or `null`. */
export function algorithmKeyword(line: string): string | null {
	const p = parseLine(line);
	return p ? (ALGO_KEYWORDS[p.keyword.toLowerCase()] ?? null) : null;
}

function norm(line: string): string {
	const p = parseLine(line);
	if (!p) return '';
	const kw = ALGO_KEYWORDS[p.keyword.toLowerCase()] ?? p.keyword;
	return `${kw.toLowerCase()} ${p.value.replace(/\s+/g, '')}`;
}

/** Which choice the lines amount to. */
export function algorithmMode(lines: string[]): AlgorithmMode {
	const algo = lines.filter((l) => algorithmKeyword(l));
	if (algo.length === 0) return 'default';
	const want = COMPATIBLE_LINES.map(norm).sort();
	const have = algo.map(norm).sort();
	return want.length === have.length && want.every((w, i) => w === have[i]) ? 'compatible' : 'custom';
}

/** The lines without any algorithm line. */
export function withoutAlgorithmLines(lines: string[]): string[] {
	return lines.filter((l) => !algorithmKeyword(l));
}

function globMatch(name: string, pattern: string): boolean {
	const re = new RegExp('^' + pattern.replace(/[.+^${}()|[\]\\]/g, '\\$&').replace(/\*/g, '.*').replace(/\?/g, '.') + '$');
	return re.test(name);
}

/** OpenSSH's `kex_assemble_names`, as apply.rs `assemble`. */
export function assemble(spec: string, defaults: string[], all: string[]): string[] {
	const out: string[] = [];
	const keep = (n: string) => {
		if (all.includes(n) && !out.includes(n)) out.push(n);
	};
	if (spec.startsWith('+')) {
		defaults.forEach(keep);
		spec.slice(1).split(',').forEach(keep);
	} else if (spec.startsWith('-')) {
		const pats = spec.slice(1).split(',');
		defaults.filter((d) => !pats.some((p) => globMatch(d, p))).forEach(keep);
	} else if (spec.startsWith('^')) {
		spec.slice(1).split(',').forEach(keep);
		defaults.forEach(keep);
	} else {
		spec.split(',').forEach(keep);
	}
	return out;
}

/** The first line that sets `keyword` (ssh_config: the first one wins). */
function specFor(lines: string[], keyword: string): string | null {
	for (const l of lines) {
		if (algorithmKeyword(l) === keyword) return parseLine(l)?.value ?? null;
	}
	return null;
}

/** The algorithms a group offers with these lines. */
export function enabledIn(lines: string[], group: CatalogGroup): string[] {
	const all = group.algorithms.map((a) => a.name);
	const defaults = group.algorithms.filter((a) => a.default).map((a) => a.name);
	const spec = specFor(lines, group.keyword);
	return spec === null ? defaults : assemble(spec, defaults, all);
}

/** Whether key logins may sign with ssh-rsa (SHA-1). */
export function pubkeySha1(lines: string[]): boolean {
	const spec = specFor(lines, PUBKEY);
	if (spec === null) return false;
	return assemble(spec, [], ['ssh-rsa']).includes('ssh-rsa');
}

/** Replace the line that sets `keyword` (or add one); `value` null removes it. */
function setLine(lines: string[], keyword: string, value: string | null): string[] {
	const out: string[] = [];
	let done = false;
	for (const l of lines) {
		if (algorithmKeyword(l) === keyword) {
			if (!done && value !== null) out.push(`${keyword} ${value}`);
			done = true;
			continue;
		}
		out.push(l);
	}
	if (!done && value !== null) out.push(`${keyword} ${value}`);
	return out;
}

/**
 * Offer exactly `names` for a group, in the engine's order. Reach's own
 * defaults are written as no line at all; defaults plus extras as "+extras",
 * so the session keeps up with later defaults.
 */
export function setEnabled(lines: string[], group: CatalogGroup, names: string[]): string[] {
	const ordered = group.algorithms.map((a) => a.name).filter((n) => names.includes(n));
	const defaults = group.algorithms.filter((a) => a.default).map((a) => a.name);
	if (ordered.length === defaults.length && defaults.every((d) => ordered.includes(d))) {
		return setLine(lines, group.keyword, null);
	}
	if (defaults.every((d) => ordered.includes(d))) {
		return setLine(lines, group.keyword, '+' + ordered.filter((n) => !defaults.includes(n)).join(','));
	}
	return setLine(lines, group.keyword, ordered.join(','));
}

/** Turn ssh-rsa signatures for key logins on or off. */
export function setPubkeySha1(lines: string[], on: boolean): string[] {
	return setLine(lines, PUBKEY, on ? '+ssh-rsa' : null);
}

/**
 * The weakenings the choice itself approves ("Keyword name", as apply.rs
 * `weakening_key`): every weak algorithm it offers beyond the defaults.
 */
export function weakKeys(lines: string[], catalog: CatalogGroup[]): string[] {
	const keys: string[] = [];
	for (const g of catalog) {
		for (const n of enabledIn(lines, g)) {
			const a = g.algorithms.find((x) => x.name === n);
			if (a?.weak && !a.default) keys.push(`${g.keyword} ${n}`);
		}
	}
	if (pubkeySha1(lines)) {
		// The plan holds the name back; the report names the whole value.
		keys.push(`${PUBKEY} ssh-rsa`, `${PUBKEY} ${specFor(lines, PUBKEY)}`);
	}
	return keys;
}

/** Whether an algorithm weakening key belongs to this choice. */
export function isAlgorithmKey(key: string): boolean {
	const kw = key.split(' ')[0];
	return Object.values(ALGO_KEYWORDS).includes(kw);
}

/** The approvals after a change: other approvals kept, this choice's replaced. */
export function syncAccepted(accepted: string[], lines: string[], catalog: CatalogGroup[]): string[] {
	return [...new Set([...accepted.filter((k) => !isAlgorithmKey(k)), ...weakKeys(lines, catalog)])];
}

/** `Keyword yes` set (ForwardAgent, ForwardX11). */
export function flagOn(lines: string[], keyword: string): boolean {
	for (const l of lines) {
		const p = parseLine(l);
		if (p && p.keyword.toLowerCase() === keyword.toLowerCase()) return p.value.toLowerCase() === 'yes';
	}
	return false;
}

/** Write `Keyword yes`, or drop the keyword's lines. */
export function setFlag(lines: string[], keyword: string, on: boolean): string[] {
	const out = lines.filter((l) => parseLine(l)?.keyword.toLowerCase() !== keyword.toLowerCase());
	if (on) out.push(`${keyword} yes`);
	return out;
}
