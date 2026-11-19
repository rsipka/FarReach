/** How a connection reaches its server when not directly, as short text for
 *  tooltips: the session list and the terminal bar say it the same way. */

export interface Hop {
	host: string;
	port?: number | null;
	username?: string | null;
}

/** `user@host → user@host2:2222`: the port only when it is not ssh's own. */
export function formatHops(hops: Hop[]): string {
	return hops.map((j) => `${j.username ? `${j.username}@` : ''}${j.host}${j.port && j.port !== 22 ? `:${j.port}` : ''}`).join(' → ');
}

export interface ProxyLike {
	proxy_type: string;
	host: string;
	port: number;
}

export function formatProxyAddress(p: ProxyLike): string {
	return `${p.host}:${p.port}`;
}

/** SVG paths of the two marks: a route for jump hosts, a globe for a proxy. */
export const ROUTE_ICON = {
	jump: 'M9 19h8.5a3.5 3.5 0 0 0 0-7h-11a3.5 3.5 0 0 1 0-7H15M6 16a3 3 0 1 0 0 6 3 3 0 0 0 0-6zM18 2a3 3 0 1 0 0 6 3 3 0 0 0 0-6z',
	proxy: 'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM3 12h18M12 3a14 14 0 0 1 0 18a14 14 0 0 1 0-18'
};
