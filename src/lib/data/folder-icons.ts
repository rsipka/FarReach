/**
 * The icons a session folder can be given: the app's own symbols (Font
 * Awesome, tinted with a colour of the user's choice) and logos (the OS
 * logos of distro-icons.ts and a few vendors from brand-icons.ts).
 *
 * A folder stores its icon as `fa:<key>` or `os:<key>` and its colour by
 * name, so the stored value stays short, syncs as plain text, and the
 * colours can be tuned here without touching anyone's folders.
 */
import type { IconDefinition } from '@fortawesome/free-solid-svg-icons';
import {
	faBolt, faBook, faBookmark, faBox, faBoxArchive, faBriefcase, faBug, faBuilding, faCalendar, faCamera, faCar,
	faChartLine, faCircle, faClock, faCloud, faCode, faCodeBranch, faCube, faCubes, faDatabase, faDesktop, faEnvelope,
	faEthernet, faFilm, faFire, faFlag, faFlask, faFolder, faFolderOpen, faGamepad, faGear, faGhost, faGlobe,
	faGraduationCap, faHardDrive, faHeadset, faHeart, faHospital, faHouse, faIndustry, faKey, faLaptop, faLayerGroup,
	faLeaf, faLocationDot, faLock, faMap, faMemory, faMicrochip, faMobileScreen, faMoon, faMusic, faNetworkWired, faPhone,
	faPlane, faPlug, faPrint, faRobot, faRocket, faSatelliteDish, faSchool, faScrewdriverWrench, faServer, faShieldHalved,
	faShip, faSitemap, faSkull, faSquare, faStar, faStore, faSun, faTag, faTerminal, faToolbox, faTowerBroadcast, faTrain,
	faTree, faTriangleExclamation, faTv, faUser, faUserShield, faUsers, faVideo, faWarehouse, faWifi, faWrench
} from '@fortawesome/free-solid-svg-icons';
import { getDistroIcon, icons as osIcons, osChoices, osIdToSlug, type DistroIconData } from './distro-icons.js';
import { brandIcons } from './brand-icons.js';

export interface FolderSymbol {
	key: string;
	icon: IconDefinition;
	/** English words the picker's search also matches. */
	words: string;
}

/** The symbols, grouped loosely: infrastructure, places, devices, work, the rest. */
export const folderSymbols: FolderSymbol[] = [
	{ key: 'server', icon: faServer, words: 'server rack' },
	{ key: 'database', icon: faDatabase, words: 'database db sql' },
	{ key: 'cloud', icon: faCloud, words: 'cloud' },
	{ key: 'network', icon: faNetworkWired, words: 'network lan switch' },
	{ key: 'sitemap', icon: faSitemap, words: 'sitemap topology tree network' },
	{ key: 'ethernet', icon: faEthernet, words: 'ethernet port router switch' },
	{ key: 'wifi', icon: faWifi, words: 'wifi wireless wlan access point' },
	{ key: 'tower', icon: faTowerBroadcast, words: 'tower broadcast radio antenna' },
	{ key: 'satellite', icon: faSatelliteDish, words: 'satellite dish' },
	{ key: 'globe', icon: faGlobe, words: 'globe internet web world public' },
	{ key: 'shield', icon: faShieldHalved, words: 'shield firewall security' },
	{ key: 'lock', icon: faLock, words: 'lock secure vpn' },
	{ key: 'key', icon: faKey, words: 'key auth' },
	{ key: 'hard-drive', icon: faHardDrive, words: 'hard drive disk storage nas backup' },
	{ key: 'microchip', icon: faMicrochip, words: 'microchip cpu embedded iot' },
	{ key: 'memory', icon: faMemory, words: 'memory ram' },
	{ key: 'plug', icon: faPlug, words: 'plug power pdu ups' },
	{ key: 'bolt', icon: faBolt, words: 'bolt power fast' },
	{ key: 'terminal', icon: faTerminal, words: 'terminal shell console' },
	{ key: 'code', icon: faCode, words: 'code dev development' },
	{ key: 'branch', icon: faCodeBranch, words: 'branch git ci' },
	{ key: 'bug', icon: faBug, words: 'bug debug test' },
	{ key: 'flask', icon: faFlask, words: 'flask lab test staging' },
	{ key: 'rocket', icon: faRocket, words: 'rocket production deploy launch' },
	{ key: 'cube', icon: faCube, words: 'cube container vm' },
	{ key: 'cubes', icon: faCubes, words: 'cubes containers cluster' },
	{ key: 'box', icon: faBox, words: 'box package' },
	{ key: 'archive', icon: faBoxArchive, words: 'archive old legacy' },
	{ key: 'layers', icon: faLayerGroup, words: 'layers stack group' },
	{ key: 'robot', icon: faRobot, words: 'robot automation bot' },
	{ key: 'house', icon: faHouse, words: 'house home homelab' },
	{ key: 'building', icon: faBuilding, words: 'building office company' },
	{ key: 'industry', icon: faIndustry, words: 'industry factory plant' },
	{ key: 'warehouse', icon: faWarehouse, words: 'warehouse datacenter' },
	{ key: 'store', icon: faStore, words: 'store shop branch' },
	{ key: 'hospital', icon: faHospital, words: 'hospital clinic' },
	{ key: 'school', icon: faSchool, words: 'school' },
	{ key: 'graduation', icon: faGraduationCap, words: 'graduation university education' },
	{ key: 'location', icon: faLocationDot, words: 'location place site pin' },
	{ key: 'map', icon: faMap, words: 'map region' },
	{ key: 'desktop', icon: faDesktop, words: 'desktop pc workstation' },
	{ key: 'laptop', icon: faLaptop, words: 'laptop notebook' },
	{ key: 'mobile', icon: faMobileScreen, words: 'mobile phone' },
	{ key: 'tv', icon: faTv, words: 'tv screen display media' },
	{ key: 'print', icon: faPrint, words: 'print printer' },
	{ key: 'camera', icon: faCamera, words: 'camera cctv' },
	{ key: 'video', icon: faVideo, words: 'video camera nvr' },
	{ key: 'phone', icon: faPhone, words: 'phone voip pbx' },
	{ key: 'headset', icon: faHeadset, words: 'headset support helpdesk' },
	{ key: 'envelope', icon: faEnvelope, words: 'envelope mail email' },
	{ key: 'briefcase', icon: faBriefcase, words: 'briefcase work client customer' },
	{ key: 'user', icon: faUser, words: 'user person personal' },
	{ key: 'users', icon: faUsers, words: 'users team group' },
	{ key: 'user-shield', icon: faUserShield, words: 'admin user shield' },
	{ key: 'gear', icon: faGear, words: 'gear settings config' },
	{ key: 'wrench', icon: faWrench, words: 'wrench maintenance' },
	{ key: 'tools', icon: faScrewdriverWrench, words: 'tools service repair' },
	{ key: 'toolbox', icon: faToolbox, words: 'toolbox utilities' },
	{ key: 'chart', icon: faChartLine, words: 'chart monitoring metrics' },
	{ key: 'clock', icon: faClock, words: 'clock time cron' },
	{ key: 'calendar', icon: faCalendar, words: 'calendar' },
	{ key: 'book', icon: faBook, words: 'book docs' },
	{ key: 'folder', icon: faFolder, words: 'folder filled' },
	{ key: 'folder-open', icon: faFolderOpen, words: 'folder open' },
	{ key: 'star', icon: faStar, words: 'star favourite favorite' },
	{ key: 'heart', icon: faHeart, words: 'heart love' },
	{ key: 'flag', icon: faFlag, words: 'flag' },
	{ key: 'tag', icon: faTag, words: 'tag label' },
	{ key: 'bookmark', icon: faBookmark, words: 'bookmark' },
	{ key: 'warning', icon: faTriangleExclamation, words: 'warning danger critical' },
	{ key: 'fire', icon: faFire, words: 'fire hot critical' },
	{ key: 'skull', icon: faSkull, words: 'skull danger dead' },
	{ key: 'ghost', icon: faGhost, words: 'ghost' },
	{ key: 'leaf', icon: faLeaf, words: 'leaf green' },
	{ key: 'tree', icon: faTree, words: 'tree' },
	{ key: 'sun', icon: faSun, words: 'sun day' },
	{ key: 'moon', icon: faMoon, words: 'moon night' },
	{ key: 'gamepad', icon: faGamepad, words: 'gamepad games' },
	{ key: 'music', icon: faMusic, words: 'music audio' },
	{ key: 'film', icon: faFilm, words: 'film movie media' },
	{ key: 'car', icon: faCar, words: 'car vehicle' },
	{ key: 'plane', icon: faPlane, words: 'plane travel' },
	{ key: 'ship', icon: faShip, words: 'ship boat' },
	{ key: 'train', icon: faTrain, words: 'train' },
	{ key: 'circle', icon: faCircle, words: 'circle dot' },
	{ key: 'square', icon: faSquare, words: 'square' }
];

const symbolsByKey = new Map(folderSymbols.map((s) => [s.key, s]));

export interface FolderColor {
	key: string;
	/** Bright enough on a dark sidebar, deep enough on a light one. */
	hex: string;
}

export const folderColors: FolderColor[] = [
	{ key: 'red', hex: '#ef4444' },
	{ key: 'orange', hex: '#f97316' },
	{ key: 'amber', hex: '#f59e0b' },
	{ key: 'yellow', hex: '#d4b106' },
	{ key: 'lime', hex: '#65a30d' },
	{ key: 'green', hex: '#22c55e' },
	{ key: 'teal', hex: '#14b8a6' },
	{ key: 'cyan', hex: '#06b6d4' },
	{ key: 'blue', hex: '#3b82f6' },
	{ key: 'indigo', hex: '#6366f1' },
	{ key: 'purple', hex: '#a855f7' },
	{ key: 'pink', hex: '#ec4899' },
	{ key: 'brown', hex: '#a16207' },
	{ key: 'grey', hex: '#94a3b8' }
];

const colorsByKey = new Map(folderColors.map((c) => [c.key, c.hex]));

/** A colour name as CSS, or undefined for none or one this version does not know. */
export function folderColorCss(color: string | null | undefined): string | undefined {
	return color ? colorsByKey.get(color) : undefined;
}

export interface FolderLogo {
	/** As stored: `os:<key>`. */
	value: string;
	title: string;
	data: DistroIconData;
}

/** Every logo a folder can take, by title: the OS logos and the vendor ones. */
export function folderLogos(): FolderLogo[] {
	const logos: FolderLogo[] = [];
	for (const { id, title } of osChoices()) {
		const data = getDistroIcon(id);
		if (data) logos.push({ value: `os:${id}`, title, data });
	}
	for (const [key, data] of Object.entries(brandIcons)) logos.push({ value: `os:${key}`, title: data.title, data });
	return logos.sort((a, b) => a.title.localeCompare(b.title));
}

/** What a stored icon draws: a symbol, a logo, or (none, or one this
 *  version does not know) the plain folder. */
export type ResolvedFolderIcon =
	| { kind: 'symbol'; icon: IconDefinition }
	| { kind: 'logo'; data: DistroIconData }
	| { kind: 'folder' };

export function resolveFolderIcon(value: string | null | undefined): ResolvedFolderIcon {
	if (value?.startsWith('fa:')) {
		const symbol = symbolsByKey.get(value.slice(3));
		if (symbol) return { kind: 'symbol', icon: symbol.icon };
	} else if (value?.startsWith('os:')) {
		const key = value.slice(3);
		const own = <T>(map: Record<string, T>, k: string | undefined): T | undefined =>
			k !== undefined && Object.hasOwn(map, k) ? map[k] : undefined;
		const data = own(brandIcons, key) ?? own(osIcons, own(osIdToSlug, key));
		if (data) return { kind: 'logo', data };
	}
	return { kind: 'folder' };
}
