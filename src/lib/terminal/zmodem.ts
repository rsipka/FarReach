/** Sizes and speeds for the ZMODEM progress bar. */

const UNITS = ['B', 'KB', 'MB', 'GB'];

/** 1536 -> "1.5 KB". */
export function formatBytes(n: number): string {
	let v = n;
	let u = 0;
	while (v >= 1024 && u < UNITS.length - 1) {
		v /= 1024;
		u++;
	}
	return `${u === 0 || v >= 100 ? Math.round(v) : v.toFixed(1)} ${UNITS[u]}`;
}

/** Whole percent done, or null when the size is not known. */
export function percent(done: number, total: number | null): number | null {
	if (!total) return total === 0 ? 100 : null;
	return Math.min(100, Math.floor((done / total) * 100));
}

/**
 * Bytes per second over the last few seconds of samples, so the figure
 * follows the line rather than averaging the whole transfer.
 */
export class Speed {
	private samples: { t: number; done: number }[] = [];
	private file = '';

	/** Note `done` bytes of `file` at time `t` (ms); the rate, once known. */
	add(file: string, done: number, t: number): number | null {
		if (file !== this.file) {
			this.file = file;
			this.samples = [];
		}
		this.samples.push({ t, done });
		while (this.samples.length > 2 && t - this.samples[0].t > 3000) this.samples.shift();
		const first = this.samples[0];
		const dt = t - first.t;
		if (dt < 400) return null;
		return ((done - first.done) * 1000) / dt;
	}
}
