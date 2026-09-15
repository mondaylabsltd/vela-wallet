import { describe, expect, it } from 'vitest';
import { LAND_DOTS, LAND_MASK } from './globe-data';
import {
	angleFacing,
	fromLonLat,
	latticePoint,
	project,
	toLonLat,
	type Vec
} from './globe-geometry';

/** The land mask, as the points it marks. */
function landPoints(): Vec[] {
	const bits = Uint8Array.from(atob(LAND_MASK), (c) => c.charCodeAt(0));
	const pts: Vec[] = [];
	for (let i = 0; i < LAND_DOTS; i++) {
		if (bits[i >> 3] & (1 << (i & 7))) pts.push(latticePoint(i, LAND_DOTS));
	}
	return pts;
}

const LAND = landPoints();

/** Is there a land dot within ~2 lattice steps of this place? */
function looksLikeLand(lon: number, lat: number) {
	const here = fromLonLat(lon, lat);
	const reach = Math.sqrt((4 * Math.PI) / LAND_DOTS) * 2;
	return LAND.some((p) => Math.hypot(p.x - here.x, p.y - here.y, p.z - here.z) < reach);
}

describe('the globe is the right way round', () => {
	// The failure this guards against renders perfectly: a mirrored globe is
	// still a globe, with the Americas where Africa belongs. Only places can
	// catch it.
	it.each([
		['Paris', 2.3, 48.9],
		['Tokyo', 139.7, 35.7],
		['Chicago', -87.6, 41.9],
		['Nairobi', 36.8, -1.3],
		['Perth', 115.9, -32]
	])('%s is on land', (_name, lon, lat) => {
		expect(looksLikeLand(lon, lat)).toBe(true);
	});

	it.each([
		['the mid-Atlantic', -30, 25],
		['the Bay of Bengal', 88, 15],
		['the South Pacific', -120, -30],
		['the Southern Ocean', 60, -55]
	])('%s is open water', (_name, lon, lat) => {
		expect(looksLikeLand(lon, lat)).toBe(false);
	});
});

describe('the camera', () => {
	it('puts east to the right of west', () => {
		// Facing the Atlantic: Lisbon is east of New York, so it draws to the
		// right of it. Get this backwards and the globe is a mirror image.
		const angle = angleFacing(-40);
		const ny = project(fromLonLat(-74, 40.7), angle);
		const lisbon = project(fromLonLat(-9.1, 38.7), angle);
		expect(ny.z).toBeGreaterThan(0);
		expect(lisbon.z).toBeGreaterThan(0);
		expect(lisbon.x).toBeGreaterThan(ny.x);
	});

	it('turns the way the Earth does, carrying the face westward', () => {
		// The longitude at the centre climbs as the spin angle does: new land
		// arrives from the east, which on screen means from the right.
		const before = project(fromLonLat(20, 0), angleFacing(0));
		const after = project(fromLonLat(20, 0), angleFacing(10));
		expect(after.x).toBeLessThan(before.x);
	});

	it('faces the longitude it was asked to', () => {
		for (const lon of [-120, -40, 0, 75, 160]) {
			const v = project(fromLonLat(lon, 0), angleFacing(lon));
			expect(v.z).toBeCloseTo(Math.cos(0.42), 6);
			expect(v.x).toBeCloseTo(0, 6);
		}
	});

	it('round-trips a place through the lattice convention', () => {
		const { lon, lat } = toLonLat(fromLonLat(139.7, 35.7));
		expect(lon).toBeCloseTo(139.7, 6);
		expect(lat).toBeCloseTo(35.7, 6);
	});
});
