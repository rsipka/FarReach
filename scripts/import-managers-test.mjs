/**
 * Tests the session importers for mRemoteNG / mRemote, MobaXterm, PuTTY,
 * WinSCP and Tabby (src/lib/sessions/import), in Node. Encrypted fixtures
 * are made here with Node's crypto, the way each app makes them, so the
 * WebCrypto readers are checked against an independent implementation.
 *
 * Run: node --import ./scripts/resolve-lib.mjs scripts/import-managers-test.mjs
 */

import crypto from 'node:crypto';
import { md5, readIni, decodeFile, unpercent } from '../src/lib/sessions/import/common.ts';
import { parseMremoteNg, mremoteDecrypt } from '../src/lib/sessions/import/mremoteng.ts';
import { parsePutty } from '../src/lib/sessions/import/putty.ts';
import { parseWinScp, winscpPassword } from '../src/lib/sessions/import/winscp.ts';
import { parseMobaXterm, mobaDecryptLegacy } from '../src/lib/sessions/import/mobaxterm.ts';
import { parseTabby } from '../src/lib/sessions/import/tabby.ts';

let failures = 0;
const check = (name, cond, extra = '') => {
	if (cond) console.log(`  PASS  ${name}`);
	else {
		failures++;
		console.log(`  FAIL  ${name}${extra ? '\n        ' + extra : ''}`);
	}
};
const section = (s) => console.log(`\n${s}`);
const eq = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const byName = (r, n) => r.hosts.find((h) => h.name === n);

// ---------------------------------------------------------------------------
section('common');
check('md5 of empty', Buffer.from(md5(new Uint8Array())).toString('hex') === 'd41d8cd98f00b204e9800998ecf8427e');
check('md5 of mR3m', Buffer.from(md5(new TextEncoder().encode('mR3m'))).toString('hex') === crypto.createHash('md5').update('mR3m').digest('hex'));
const long = crypto.randomBytes(200);
check('md5 over several blocks', Buffer.from(md5(long)).toString('hex') === crypto.createHash('md5').update(long).digest('hex'));
check('UTF-16 .reg decoded', decodeFile(Buffer.concat([Buffer.from([0xff, 0xfe]), Buffer.from('[A]\r\n', 'utf16le')])) === '[A]\r\n');
check('Windows-1252 decoded', decodeFile(Buffer.from([0x63, 0xe9])) === 'cé');
check('percent escapes', unpercent('My%20Folder%2Fx') === 'My Folder/x');
const ini = readIni('Windows Registry Editor Version 5.00\r\n\r\n[HKEY_CURRENT_USER\\X]\r\n"A"="C:\\\\keys\\\\a \\"q\\""\r\n"B"=dword:00000016\r\n"C"=hex:01,02,\\\r\n  03\r\n');
check('.reg strings unescaped', ini[0].values.get('A') === 'C:\\keys\\a "q"');
check('.reg dword in decimal', ini[0].values.get('B') === '22');
check('.reg binary left out', !ini[0].values.has('C'));

// ---------------------------------------------------------------------------
section('mRemoteNG');

function gcmEncrypt(plain, password, iterations = 1000) {
	const salt = crypto.randomBytes(16);
	const nonce = crypto.randomBytes(16);
	const key = crypto.pbkdf2Sync(Buffer.from(password, 'latin1'), salt, iterations, 32, 'sha1');
	const c = crypto.createCipheriv('aes-256-gcm', key, nonce);
	c.setAAD(salt);
	const ct = Buffer.concat([c.update(plain, 'utf8'), c.final(), c.getAuthTag()]);
	return Buffer.concat([salt, nonce, ct]).toString('base64');
}
function cbcEncrypt(plain, password) {
	const iv = crypto.randomBytes(16);
	const key = crypto.createHash('md5').update(password, 'utf8').digest();
	const c = crypto.createCipheriv('aes-128-cbc', key, iv);
	return Buffer.concat([iv, c.update(plain, 'utf8'), c.final()]).toString('base64');
}

const ngNodes = (enc) => `
  <Node Name="Linux" Type="Container" Expanded="True" Username="ops" Password="${enc('folder-pw')}" Domain="" InheritUsername="false" InheritPassword="false">
    <Node Name="web1" Type="Connection" Hostname="web1.example.com" Protocol="SSH2" Port="2222" Username="" Password="" Descr="Front &amp; web" InheritUsername="true" InheritPassword="true" />
    <Node Name="db1" Type="Connection" Hostname="db1" Protocol="SSH2" Port="22" Username="root" Password="${enc('root-pw')}" InheritUsername="false" InheritPassword="false" />
    <Node Name="Inner" Type="Container" Username="" InheritUsername="true" InheritPassword="true">
      <Node Name="app1" Type="Connection" Hostname="app1" Protocol="SSH2" Port="22" InheritUsername="true" InheritPassword="true" />
    </Node>
  </Node>
  <Node Name="DC01" Type="Connection" Hostname="dc01.corp:3390" Protocol="RDP" Port="3389" Username="admin" Domain="CORP" Password="${enc('Pässwörd')}" />
  <Node Name="switch" Type="Connection" Hostname="10.0.0.1" Protocol="Telnet" Port="23" />
  <Node Name="rawport" Type="Connection" Hostname="10.0.0.2" Protocol="RAW" Port="4001" />
  <Node Name="kiosk" Type="Connection" Hostname="kiosk" Protocol="VNC" Port="5900" Username="x" />
  <Node Name="intranet" Type="Connection" Hostname="intra" Protocol="HTTPS" Port="443" />
  <Node Name="nohost" Type="Connection" Hostname="" Protocol="SSH2" />`;

const ngDefault = `<?xml version="1.0" encoding="utf-8"?>
<mrng:Connections xmlns:mrng="http://mremoteng.org" Name="Connections" Export="false" EncryptionEngine="AES" BlockCipherMode="GCM" KdfIterations="1000" FullFileEncryption="false" Protected="${gcmEncrypt('ThisIsNotProtected', 'mR3m')}" ConfVersion="2.6">${ngNodes((p) => gcmEncrypt(p, 'mR3m'))}
</mrng:Connections>`;
{
	const r = await parseMremoteNg(ngDefault);
	check('default-key file reads', r.ok, JSON.stringify(r));
	if (r.ok) {
		const web1 = byName(r, 'web1');
		check('folder login becomes a folder credential', web1.username === 'ops' && eq(web1.credential, { key: 'folder:Linux', from: ['Linux'] }) && !web1.password);
		check('credential keeps the folder password', r.credentials.length === 1 && r.credentials[0].password === 'folder-pw' && r.credentials[0].name === 'Linux');
		check('folder path and notes', eq(web1.folder, ['Linux']) && web1.notes === 'Front & web' && web1.port === 2222);
		const app1 = byName(r, 'app1');
		check('nested inherit points at the folder that set the login', eq(app1.credential, { key: 'folder:Linux', from: ['Linux'] }) && eq(app1.folder, ['Linux', 'Inner']));
		const db1 = byName(r, 'db1');
		check('own password decrypted', db1.password === 'root-pw' && db1.credential === null);
		const dc = byName(r, 'DC01');
		check('RDP with domain, port from Port attribute, UTF-8 password', dc.kind === 'rdp' && dc.domain === 'CORP' && dc.port === 3389 && dc.host === 'dc01.corp' && dc.password === 'Pässwörd');
		check('Telnet and RAW', byName(r, 'switch').kind === 'telnet' && byName(r, 'rawport').raw === true);
		check('VNC has no username', byName(r, 'kiosk').username === '');
		check('HTTPS skipped as unsupported', r.skipped.some((s) => s.label === 'intranet' && s.reason === 'unsupported' && s.type === 'HTTPS'));
		check('no host skipped', r.skipped.some((s) => s.label === 'nohost' && s.reason === 'no_host'));
		check('no notice when every password reads', r.notice === undefined);
	}
}

{
	const own = `<?xml version="1.0"?>
<mrng:Connections xmlns:mrng="http://mremoteng.org" Name="Connections" EncryptionEngine="AES" BlockCipherMode="GCM" KdfIterations="5000" Protected="${gcmEncrypt('ThisIsProtected', 'hunter2', 5000)}" ConfVersion="2.6">${ngNodes((p) => gcmEncrypt(p, 'hunter2', 5000))}
</mrng:Connections>`;
	const first = await parseMremoteNg(own);
	check('own password is asked for', !first.ok && first.error === 'password' && first.wrong === false);
	const wrong = await parseMremoteNg(own, 'nope');
	check('wrong password is said so', !wrong.ok && wrong.error === 'password' && wrong.wrong === true);
	const right = await parseMremoteNg(own, 'hunter2');
	check('right password reads, with KdfIterations', right.ok && byName(right, 'db1').password === 'root-pw');
	const skip = await parseMremoteNg(own, null);
	check('skipping passwords still imports, with a notice', skip.ok && byName(skip, 'db1').password === null && skip.notice === 'passwords_skipped');
}
{
	const legacy = `<?xml version="1.0" encoding="utf-8"?>
<Connections Name="Connections" Export="False" Protected="${cbcEncrypt('ThisIsNotProtected', 'mR3m')}" ConfVersion="2.4">${ngNodes((p) => cbcEncrypt(p, 'mR3m'))}
</Connections>`;
	const r = await parseMremoteNg(legacy);
	check('old mRemote CBC file reads', r.ok && byName(r, 'db1').password === 'root-pw' && byName(r, 'DC01').password === 'Pässwörd');
}
{
	const inner = ngNodes((p) => gcmEncrypt(p, 'mR3m'));
	const full = `<?xml version="1.0"?>
<mrng:Connections xmlns:mrng="http://mremoteng.org" Name="Connections" EncryptionEngine="AES" BlockCipherMode="GCM" KdfIterations="1000" FullFileEncryption="true" Protected="${gcmEncrypt('ThisIsNotProtected', 'mR3m')}" ConfVersion="2.6">${gcmEncrypt(inner, 'mR3m')}</mrng:Connections>`;
	const r = await parseMremoteNg(full);
	check('full-file encryption reads', r.ok && r.hosts.length === 7, JSON.stringify(r).slice(0, 200));
	check('mremoteDecrypt rejects a wrong key', (await mremoteDecrypt(gcmEncrypt('x', 'a'), 'b', { mode: 'gcm', iterations: 1000 })) === null);
}
check('not XML is no hosts', (await parseMremoteNg('hello')).error === 'no_hosts');

// ---------------------------------------------------------------------------
section('PuTTY');
{
	const reg = `Windows Registry Editor Version 5.00

[HKEY_CURRENT_USER\\Software\\SimonTatham\\PuTTY\\Sessions]

[HKEY_CURRENT_USER\\Software\\SimonTatham\\PuTTY\\Sessions\\Default%20Settings]
"Protocol"="ssh"

[HKEY_CURRENT_USER\\Software\\SimonTatham\\PuTTY\\Sessions\\bastion]
"HostName"="jump@bastion.example.com"
"PortNumber"=dword:00000016
"Protocol"="ssh"
"PublicKeyFile"="C:\\\\keys\\\\jump.ppk"

[HKEY_CURRENT_USER\\Software\\SimonTatham\\PuTTY\\Sessions\\My%20Web]
"HostName"="web1.internal"
"PortNumber"=dword:000008ae
"Protocol"="ssh"
"UserName"="deploy"
"PublicKeyFile"="C:\\\\keys\\\\web.ppk"
"ProxyMethod"=dword:00000006
"ProxyHost"="bastion"

[HKEY_CURRENT_USER\\Software\\SimonTatham\\PuTTY\\Sessions\\db]
"HostName"="db1"
"PortNumber"=dword:00000016
"Protocol"="ssh"
"UserName"="pg"
"ProxyMethod"=dword:00000006
"ProxyHost"="gw.example.com"
"ProxyPort"=dword:00000801
"ProxyUsername"="gwuser"
"ProxyPassword"="gwpass"

[HKEY_CURRENT_USER\\Software\\SimonTatham\\PuTTY\\Sessions\\router]
"HostName"="10.0.0.1"
"PortNumber"=dword:00000017
"Protocol"="telnet"

[HKEY_CURRENT_USER\\Software\\SimonTatham\\PuTTY\\Sessions\\console]
"HostName"=""
"Protocol"="serial"
"SerialLine"="COM3"
`;
	// As `reg export` writes it: UTF-16 with a byte order mark.
	const r = parsePutty(decodeFile(Buffer.concat([Buffer.from([0xff, 0xfe]), Buffer.from(reg, 'utf16le')])));
	check('reg export reads', r.ok, JSON.stringify(r));
	const web = byName(r, 'My Web');
	check('name unescaped, port from dword, key file', web.host === 'web1.internal' && web.port === 2222 && web.username === 'deploy' && web.keyPath === 'C:\\keys\\web.ppk');
	check('SSH proxy naming a saved session becomes its jump host', eq(web.jump, [{ host: 'bastion.example.com', port: 22, username: 'jump', keyPath: 'C:\\keys\\jump.ppk' }]), JSON.stringify(web.jump));
	check('SSH proxy by host, with its plain password', eq(byName(r, 'db').jump, [{ host: 'gw.example.com', port: 2049, username: 'gwuser', password: 'gwpass' }]));
	check('user@host in HostName', byName(r, 'bastion').username === 'jump');
	check('telnet session', byName(r, 'router').kind === 'telnet' && byName(r, 'router').port === 23);
	check('serial skipped, Default Settings not listed', r.skipped.length === 1 && r.skipped[0].type === 'serial' && !r.hosts.some((h) => h.name === 'Default Settings'));
	const unix = parsePutty('[Sessions\\box%20one]\nHostName=box\nProtocol=ssh\nPortNumber=2200\nUserName=me\n');
	check('Linux session files (as the app joins them)', unix.ok && unix.hosts[0].name === 'box one' && unix.hosts[0].port === 2200);
}

// ---------------------------------------------------------------------------
section('WinSCP');
function winscpEncrypt(password, user, host) {
	const enc = (b) => ((~b ^ 0xa3) & 0xff).toString(16).toUpperCase().padStart(2, '0');
	const s = Buffer.from(user + host + password, 'utf8');
	let out = enc(0xff) + enc(0) + enc(s.length) + enc(2) + 'AAAA';
	for (const b of s) out += enc(b);
	return out + '1234';
}
{
	check('password deobfuscated', winscpPassword(winscpEncrypt('s3cr€t', 'deploy', 'web1'), 'deploy', 'web1') === 's3cr€t');
	check('wrong user/host gives null', winscpPassword(winscpEncrypt('x', 'a', 'b'), 'c', 'd') === null);
	const iniText = `[Sessions\\Servers/Web%20one]
HostName=web1.example.com
PortNumber=2222
UserName=deploy
Password=${winscpEncrypt('pw1', 'deploy', 'web1.example.com')}
FSProtocol=2
PublicKeyFile=C:%5Ckeys%5Cweb.ppk
Tunnel=1
TunnelHostName=gw.example.com
TunnelUserName=gw
TunnelPassword=${winscpEncrypt('gwpw', 'gw', 'gw.example.com')}
[Sessions\\ftp%20site]
HostName=ftp.example.com
FSProtocol=5
[Sessions\\scp]
HostName=old.example.com
UserName=root
FSProtocol=0
[Sessions\\Default%20Settings]
HostName=
[Configuration\\Interface]
X=1
`;
	const r = parseWinScp(iniText);
	check('INI reads', r.ok, JSON.stringify(r));
	const web = byName(r, 'Web one');
	check('folder, port, key path unescaped', eq(web.folder, ['Servers']) && web.port === 2222 && web.keyPath === 'C:\\keys\\web.ppk');
	check('password read', web.password === 'pw1');
	check('tunnel becomes jump host with password', eq(web.jump, [{ host: 'gw.example.com', port: 22, username: 'gw', keyPath: null, password: 'gwpw' }]), JSON.stringify(web.jump));
	check('SCP site is SSH; FTP skipped', byName(r, 'scp').kind === 'ssh' && r.skipped.some((s) => s.label === 'ftp site' && s.type === 'FTP'));
	const master = parseWinScp(iniText + '[Configuration\\Security]\nUseMasterPassword=1\n');
	check('master password: no passwords, notice', master.ok && byName(master, 'Web one').password === undefined && master.notice === 'passwords_skipped');
	const reg = `Windows Registry Editor Version 5.00\r\n\r\n[HKEY_CURRENT_USER\\Software\\Martin Prikryl\\WinSCP 2\\Sessions\\My%20Folder/box]\r\n"HostName"="box.example.com"\r\n"UserName"="me"\r\n"PortNumber"=dword:00000016\r\n"PublicKeyFile"="C:\\\\k%20ey.ppk"\r\n`;
	const rr = parseWinScp(reg);
	check('registry export: folder from key name, values not unescaped', rr.ok && eq(rr.hosts[0].folder, ['My Folder']) && rr.hosts[0].keyPath === 'C:\\k%20ey.ppk');
}

// ---------------------------------------------------------------------------
section('MobaXterm');
function mobaMasterEncrypt(plain, master) {
	const key = crypto.createHash('sha512').update(master).digest().subarray(0, 32);
	const ecb = crypto.createCipheriv('aes-256-ecb', key, null);
	ecb.setAutoPadding(false);
	const iv = ecb.update(Buffer.alloc(16));
	const c = crypto.createCipheriv('aes-256-cfb8', key, iv);
	return Buffer.concat([c.update(plain, 'utf8'), c.final()]).toString('base64');
}
function mobaLegacyEncrypt(plain, sessionP) {
	let s = sessionP;
	while (s.length < 20) s += s;
	const space = [s.toUpperCase(), s.toUpperCase(), s.toLowerCase(), s.toLowerCase()];
	const alphabet = '0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz+/';
	const key = [...'0d5e9n1348/U2+67'];
	for (let i = 0; i < key.length; i++) {
		const b = space[(i + 1) % space.length][i];
		if (!key.includes(b) && alphabet.includes(b)) key[i] = b;
	}
	let out = '';
	for (const ch of Buffer.from(plain, 'utf8')) {
		out += key[ch & 0x0f];
		key.unshift(key.pop());
		out += key[ch >> 4];
		key.unshift(key.pop());
	}
	// Noise characters outside the key are dropped by the reader.
	return out.split('').join(alphabet.split('').find((c) => !key.includes(c)) ?? '');
}
{
	const sessions = `[Bookmarks]
SubRep=
ImgNum=42
web1=#109#0%web1.example.com%2222%deploy%%-1%-1%%bastion.example.com__PIPE__inner.example.com%22__PIPE__2200%jump__PIPE__%0%0%0%_ProfileDir_\\.ssh\\id_rsa%%-1%0%0%0%%1080%%0%0%1#MobaFont%10%0%0%-1%15%236,236,236%30,30,30%180,180,192%0%-1%0%%xterm%-1%-1%_Std_Colors_0_%80%24%0%1%-1%<none>%%0%1%-1#0#Front end#-1
[Bookmarks_1]
SubRep=Windows\\Servers
ImgNum=41
DC01=#91#4%dc01.corp%3389%[Domain admin]%0%-1%-1%-1%-1%0%0%-1%%%%%0%0%%-1%%-1%-1%0%-1%0%-1#MobaFont%10%0%0%-1%15%236,236,236%30,30,30%180,180,192%0%-1%0%%xterm%-1%-1%_Std_Colors_0_%80%24%0%1%-1%<none>%%0%1%-1#0# #-1
switch=#98#1%10.0.0.1%23%%%2%%%%%0%0%%1080%#MobaFont%10%0%0%-1%15%236,236,236%30,30,30%180,180,192%0%-1%0%%xterm%-1%-1%_Std_Colors_0_%80%24%0%1%-1%<none>%%0%1%-1#0# #-1
console=#131#8%-1%100%3%0%0%1%2%COM3#MobaFont%10#0# #-1
`;
	const plain = await parseMobaXterm(sessions);
	check('export (no passwords) reads without asking', plain.ok && plain.notice === undefined, JSON.stringify(plain).slice(0, 300));
	const web = byName(plain, 'web1');
	check('SSH host, port, user, key', web.host === 'web1.example.com' && web.port === 2222 && web.username === 'deploy' && web.keyPath === '~\\.ssh\\id_rsa');
	check('two-hop gateway', eq(web.jump, [{ host: 'bastion.example.com', port: 22, username: 'jump' }, { host: 'inner.example.com', port: 2200, username: undefined }]), JSON.stringify(web.jump));
	check('comment becomes notes', web.notes === 'Front end');
	const dc = byName(plain, 'DC01');
	check('RDP in nested folder', dc.kind === 'rdp' && eq(dc.folder, ['Windows', 'Servers']));
	check('telnet', byName(plain, 'switch').kind === 'telnet');
	check('serial skipped', plain.skipped.some((s) => s.label === 'console' && s.type === 'Serial'));

	const withMaster = `${sessions}[Passwords]
ssh22:jump@bastion.example.com=${mobaMasterEncrypt('jumppw', 'master!')}
ssh2222:deploy@web1.example.com=${mobaMasterEncrypt('webpw', 'master!')}
[Credentials]
Domain admin=administrator:${mobaMasterEncrypt('dapw', 'master!')}
[Sesspass]
me@PC=AQAAAN...
`;
	const ask = await parseMobaXterm(withMaster);
	check('master password asked for', !ask.ok && ask.error === 'password' && !ask.wrong);
	const wrong = await parseMobaXterm(withMaster, 'nope');
	check('wrong master password said so', !wrong.ok && wrong.error === 'password' && wrong.wrong);
	const r = await parseMobaXterm(withMaster, 'master!');
	check('master password reads', r.ok, JSON.stringify(r).slice(0, 300));
	if (r.ok) {
		const w = byName(r, 'web1');
		check('host password by proto+port key', w.password === 'webpw');
		check('gateway password', w.jump[0].password === 'jumppw');
		const d = byName(r, 'DC01');
		check('named credential linked', d.username === 'administrator' && eq(d.credential, { key: 'moba:Domain admin', from: null }));
		check('credential password', r.credentials.length === 1 && r.credentials[0].password === 'dapw');
	}
	const skip = await parseMobaXterm(withMaster, null);
	check('skip passwords', skip.ok && skip.notice === 'passwords_skipped' && !byName(skip, 'web1').password);

	const sessionP = '165821882556840';
	check('legacy scramble round trip', mobaDecryptLegacy(mobaLegacyEncrypt('Lw3+cZ2s', sessionP), sessionP) === 'Lw3+cZ2s');
	const legacy = `${sessions}[Misc]\nSessionP=${sessionP}\n[Passwords]\ndeploy@web1.example.com=${mobaLegacyEncrypt('oldpw', sessionP)}\n`;
	const lr = await parseMobaXterm(legacy);
	check('no master password: SessionP scramble read, user@host key', lr.ok && byName(lr, 'web1').password === 'oldpw');
	const regd = `${sessions}\nWindows Registry Editor Version 5.00\n\n[HKEY_CURRENT_USER\\Software\\Mobatek\\MobaXterm]\n"SessionP"="${sessionP}"\n\n[HKEY_CURRENT_USER\\Software\\Mobatek\\MobaXterm\\P]\n"ssh2222:deploy@web1.example.com"="${mobaLegacyEncrypt('regpw', sessionP)}"\n`;
	const rg = await parseMobaXterm(regd);
	check('installed edition: passwords from the registry', rg.ok && byName(rg, 'web1').password === 'regpw', JSON.stringify(rg).slice(0, 200));
}

// ---------------------------------------------------------------------------
section('Tabby');
function tabbyVault(content, passphrase) {
	const salt = crypto.randomBytes(8);
	const iv = crypto.randomBytes(16);
	const key = crypto.pbkdf2Sync(passphrase, salt, 100000, 32, 'sha512');
	const c = crypto.createCipheriv('aes-256-cbc', key, iv);
	return { version: 1, contents: Buffer.concat([c.update(JSON.stringify(content), 'utf8'), c.final()]).toString('base64'), keySalt: salt.toString('hex'), iv: iv.toString('hex') };
}
{
	const config = {
		version: 8,
		groups: [
			{ id: 'g1', name: 'Prod' },
			{ id: 'g2', name: 'DB', parentGroupId: 'g1' }
		],
		profiles: [
			{ id: 'ssh:custom:bastion', type: 'ssh', name: 'bastion', group: 'g1', options: { host: 'bastion.example.com', user: 'jump', privateKeys: ['file:///C:/Users/me/.ssh/id_ed25519'] } },
			{ id: 'ssh:custom:db', type: 'ssh', name: 'db1', group: 'g2', options: { host: 'db1.internal', port: 2222, user: 'pg', jumpHost: 'ssh:custom:bastion' } },
			{ type: 'telnet', name: 'switch', options: { host: '10.0.0.1', port: 23 } },
			{ type: 'serial', name: 'console', options: { port: 'COM3' } },
			{ type: 'local', name: 'bash', options: { command: 'bash' } }
		]
	};
	const r = await parseTabby(config);
	check('plain config reads; keychain notice', r.ok && r.notice === 'passwords_keychain', JSON.stringify(r).slice(0, 300));
	const db = byName(r, 'db1');
	check('nested group folder', eq(db.folder, ['Prod', 'DB']));
	check('jump host from profile, with its key', eq(db.jump, [{ host: 'bastion.example.com', port: 22, username: 'jump', keyPath: 'C:/Users/me/.ssh/id_ed25519', password: null }]), JSON.stringify(db.jump));
	check('telnet; serial and local skipped', byName(r, 'switch').kind === 'telnet' && r.skipped.map((s) => s.type).join() === 'serial,local');
	const old = await parseTabby({ profiles: [{ type: 'ssh', name: 'x', group: 'Legacy group', options: { host: 'x' } }] });
	check('old config: group by name', old.ok && eq(old.hosts[0].folder, ['Legacy group']));

	const secrets = [
		{ type: 'ssh:password', key: { user: 'pg', host: 'db1.internal', port: 2222 }, value: 'pgpw' },
		{ type: 'ssh:password', key: { user: 'jump', host: 'bastion.example.com' }, value: 'jpw' }
	];
	const withVault = { ...config, vault: tabbyVault({ config: {}, secrets }, 'pass phrase') };
	const ask = await parseTabby(withVault);
	check('vault passphrase asked for', !ask.ok && ask.error === 'password');
	const wrong = await parseTabby(withVault, 'nope');
	check('wrong passphrase said so', !wrong.ok && wrong.wrong === true);
	const ok = await parseTabby(withVault, 'pass phrase');
	check('vault passwords read', ok.ok && byName(ok, 'db1').password === 'pgpw' && byName(ok, 'db1').jump[0].password === 'jpw' && ok.notice === undefined);
	const skip = await parseTabby(withVault, null);
	check('skip vault passwords', skip.ok && !byName(skip, 'db1').password && skip.notice === 'passwords_skipped');

	const sealed = { version: 8, encrypted: true, vault: tabbyVault({ config, secrets }, 'pp') };
	check('encrypted config cannot be skipped', (await parseTabby(sealed, null)).error === 'password');
	const opened = await parseTabby(sealed, 'pp');
	check('encrypted config reads with passphrase', opened.ok && opened.hosts.length === 3 && byName(opened, 'db1').password === 'pgpw');
}

console.log(failures === 0 ? '\nAll import tests passed.' : `\n${failures} test(s) failed.`);
process.exit(failures === 0 ? 0 : 1);
