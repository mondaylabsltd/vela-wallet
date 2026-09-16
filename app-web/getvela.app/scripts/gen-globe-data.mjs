/**
 * Generates `src/lib/components/globe-data.ts` — the continents the hero globe
 * draws.
 *
 * The globe is a fixed Fibonacci lattice of points on a sphere; this script
 * decides, once, which of those points fall on land, and writes the answer as a
 * bitmask (one bit per lattice point, ~1.8 kB). Doing it here rather than in
 * the browser is the whole point: the page ships a bitmask instead of a world
 * map, and the runtime never parses geometry.
 *
 * Country borders were tried here — outlines, and a four-colour map with a
 * colour per country — and cut on 2026-09-15: on a wallet's home page a
 * political map is decoration competing with the page. Land or not land is the
 * whole vocabulary.
 *
 * Source: world-atlas@2 `land-110m.json`, which is Natural Earth's 110m land
 * layer — public domain (naturalearthdata.com/about/terms-of-use).
 *
 * Re-run with:
 *   curl -o /tmp/land-110m.json https://cdn.jsdelivr.net/npm/world-atlas@2.0.2/land-110m.json
 *   node scripts/gen-globe-data.mjs /tmp/land-110m.json
 */
import { readFileSync, writeFileSync } from 'node:fs';

/**
 * Must match DOTS in Globe.svelte — the mask is indexed by lattice position.
 * Chosen for the LOOK, not the detail: on a 380px globe these land a few pixels
 * apart and read as dots. Denser lattices turn into a fine grey rash.
 */
const DOTS = 11000;
const OUT = 'src/lib/components/globe-data.ts';

const topo = JSON.parse(readFileSync(process.argv[2] ?? '/tmp/land-110m.json', 'utf8'));

// ── TopoJSON → rings of [lon, lat] ──
// Arcs are delta-encoded integers in a quantized grid; every coordinate in the
// file is a cumulative sum through one arc, then scaled back to degrees.
const { scale, translate } = topo.transform;
const arcs = topo.arcs.map((arc) => {
	let x = 0;
	let y = 0;
	return arc.map(([dx, dy]) => {
		x += dx;
		y += dy;
		return [x * scale[0] + translate[0], y * scale[1] + translate[1]];
	});
});

function ring(indices) {
	const pts = [];
	for (const i of indices) {
		const arc = i < 0 ? [...arcs[~i]].reverse() : arcs[i];
		pts.push(...(pts.length ? arc.slice(1) : arc));
	}
	return pts;
}

/** Even-odd crossing test; a hole ring flips the result back to "outside". */
function inRing(r, x, y) {
	let inside = false;
	for (let i = 0, j = r.length - 1; i < r.length; j = i++) {
		const [xi, yi] = r[i];
		const [xj, yj] = r[j];
		if (yi > y !== yj > y && x < ((xj - xi) * (y - yi)) / (yj - yi) + xi) inside = !inside;
	}
	return inside;
}

const polygons = [];
for (const geom of topo.objects.land.geometries) {
	const list = geom.type === 'MultiPolygon' ? geom.arcs : [geom.arcs];
	for (const poly of list) {
		const rings = poly.map(ring);
		let [minX, minY, maxX, maxY] = [Infinity, Infinity, -Infinity, -Infinity];
		for (const [x, y] of rings[0]) {
			if (x < minX) minX = x;
			if (x > maxX) maxX = x;
			if (y < minY) minY = y;
			if (y > maxY) maxY = y;
		}
		polygons.push({ rings, bbox: [minX, minY, maxX, maxY] });
	}
}

function isLand(lon, lat) {
	for (const { rings, bbox } of polygons) {
		if (lon < bbox[0] || lon > bbox[2] || lat < bbox[1] || lat > bbox[3]) continue;
		if (!inRing(rings[0], lon, lat)) continue;
		let hole = false;
		for (let k = 1; k < rings.length; k++) if (inRing(rings[k], lon, lat)) hole = true;
		if (!hole) return true;
	}
	return false;
}

// ── The lattice ──
// Evenly spread by the golden angle — identical to the one the component builds
// at runtime.
const golden = Math.PI * (3 - Math.sqrt(5));
const hits = [];
for (let i = 0; i < DOTS; i++) {
	const y = 1 - (i / (DOTS - 1)) * 2;
	const r = Math.sqrt(Math.max(0, 1 - y * y));
	const theta = golden * i;
	const lat = (Math.asin(y) * 180) / Math.PI;
	// East runs towards −z: the canvas views the sphere from +z with +x to the
	// right, so this sign is what keeps the Americas west of Africa. It must
	// match `toLonLat` in src/lib/components/globe-geometry.ts, which this script
	// cannot import; globe-geometry.test.ts checks the mask against real places.
	const lon = (Math.atan2(-Math.sin(theta) * r, Math.cos(theta) * r) * 180) / Math.PI;
	if (isLand(lon, lat)) hits.push({ i, x: Math.cos(theta) * r, y, z: Math.sin(theta) * r });
}

// ── The lonely ones ──
// An island smaller than the lattice spacing lands as one dot in open water,
// and a dozen of those read as dirt on the screen rather than as geography. A
// point survives only if it has company: at least three other land points
// inside one and a half lattice steps. Continents do not notice; specks vanish,
// and so do the two-and-three-dot clusters that read as specks too.
const SPACING = Math.sqrt((4 * Math.PI) / DOTS);
const REACH = SPACING * 1.5;
const COMPANY = 3;
const kept = hits.filter((p) => {
	let n = 0;
	for (const q of hits) {
		if (q === p) continue;
		// Chord length, which below a tenth of a radian is the angle for our
		// purposes — and avoids an acos per pair.
		const d = Math.hypot(p.x - q.x, p.y - q.y, p.z - q.z);
		if (d < REACH && ++n >= COMPANY) return true;
	}
	return false;
});

const bits = new Uint8Array(Math.ceil(DOTS / 8));
for (const p of kept) bits[p.i >> 3] |= 1 << (p.i & 7);
const land = kept.length;
const dropped = hits.length - land;

const maskB64 = Buffer.from(bits).toString('base64');

writeFileSync(
	OUT,
	`// GENERATED by scripts/gen-globe-data.mjs — do not edit.
//
// One bit per point of the hero globe's ${DOTS}-point Fibonacci lattice: 1 where
// that point falls on land. Derived from Natural Earth's 110m land layer
// (public domain) via world-atlas@2. ${land} of ${DOTS} points are land, after
// ${dropped} isolated specks — islands smaller than the lattice — were dropped.

/** Size of the Fibonacci lattice the mask indexes. */
export const LAND_DOTS = ${DOTS};

/** One bit per lattice point: 1 where that point falls on land. */
export const LAND_MASK =
	'${maskB64}';
`
);
console.log(
	`${OUT}: ${land}/${DOTS} land points (${dropped} lone specks dropped), ${maskB64.length} base64 chars`
);
