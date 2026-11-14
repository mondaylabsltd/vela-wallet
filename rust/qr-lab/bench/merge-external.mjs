// Fold results measured outside Node (e.g. apple/main.swift) into out/results.json.
//   node bench/merge-external.mjs out/apple-results.json
import { readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { LAB } from './decoders.mjs';

const file = path.join(LAB, 'out/results.json');
const results = JSON.parse(readFileSync(file, 'utf8'));
const external = JSON.parse(readFileSync(path.resolve(process.argv[2]), 'utf8'));
const byId = new Map(results.samples.map((s) => [s.id, s]));
let merged = 0;
for (const s of external.samples) {
	const target = byId.get(s.id);
	if (!target) continue;
	Object.assign(target.results, s.results);
	merged++;
}
writeFileSync(file, JSON.stringify(results, null, 1));
console.log(`merged ${merged} samples from ${process.argv[2]}`);
