/**
 * The hero globe's coordinate convention, in one place.
 *
 * The bug this file exists to prevent: the canvas draws the sphere as seen from
 * +z with +x running right across the screen, so EAST has to run towards −z. Get
 * that sign wrong and everything still renders — a plausible, smoothly turning
 * globe with Africa and the Americas swapped, spinning the wrong way. It shipped
 * that way for half a day.
 *
 * `scripts/gen-globe-data.mjs` builds the land mask against this same
 * convention; it cannot import this file (it is a plain node script), so the two
 * are kept honest by globe-geometry.test.ts, which checks the generated mask
 * against places whose longitudes we know.
 */

export type Vec = { x: number; y: number; z: number };

/** Radians. The axis leans, the way a desk globe does. */
export const TILT = 0.42;

/**
 * The i-th point of an n-point Fibonacci lattice: n points spread evenly over
 * the sphere by the golden angle. The generator walks the same sequence, and the
 * land mask is indexed by i, so this must not drift.
 */
export function latticePoint(i: number, n: number): Vec {
	const golden = Math.PI * (3 - Math.sqrt(5));
	const y = 1 - (i / (n - 1)) * 2;
	const r = Math.sqrt(Math.max(0, 1 - y * y));
	const theta = golden * i;
	return { x: Math.cos(theta) * r, y, z: Math.sin(theta) * r };
}

/** Degrees → the unit sphere. East runs towards −z; north is +y. */
export function fromLonLat(lon: number, lat: number): Vec {
	const a = (lon * Math.PI) / 180;
	const b = (lat * Math.PI) / 180;
	const r = Math.cos(b);
	return { x: Math.cos(a) * r, y: Math.sin(b), z: -Math.sin(a) * r };
}

/** The inverse, in degrees — the mapping the generator applies to each lattice
 *  point before asking whether that spot is land. */
export function toLonLat(p: Vec): { lon: number; lat: number } {
	return {
		lon: (Math.atan2(-p.z, p.x) * 180) / Math.PI,
		lat: (Math.asin(Math.max(-1, Math.min(1, p.y))) * 180) / Math.PI
	};
}

/**
 * Spin about the pole by `angle`, then lean the pole towards the viewer. The
 * result is in view space: +x right, +y up, +z towards the reader, so a point is
 * on the visible half exactly when z > 0.
 */
export function project(p: Vec, angle: number): Vec {
	const cosA = Math.cos(angle);
	const sinA = Math.sin(angle);
	const cosT = Math.cos(TILT);
	const sinT = Math.sin(TILT);
	const x = p.x * cosA + p.z * sinA;
	const z1 = p.z * cosA - p.x * sinA;
	return { x, y: p.y * cosT - z1 * sinT, z: z1 * cosT + p.y * sinT };
}

/** The spin angle that puts `lon` in the middle of the visible face. */
export function angleFacing(lon: number): number {
	// z is largest when sin(lon + angle) = -1, i.e. lon + angle = -90°.
	const a = ((-90 - lon) * Math.PI) / 180;
	return (a + 2 * Math.PI * 2) % (2 * Math.PI);
}
