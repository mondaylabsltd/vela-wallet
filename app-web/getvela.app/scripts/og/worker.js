// One render worker for renderAll (render-all.js): its own wasm instance.
import { parentPort, workerData } from 'node:worker_threads';
import { renderOg } from './render.js';

const out = /** @type {{ path: string, png: Uint8Array }[]} */ ([]);
for (const card of workerData.cards) out.push({ path: card.path, png: renderOg(card) });
parentPort?.postMessage(
	out,
	out.map((o) => /** @type {ArrayBuffer} */ (o.png.buffer))
);
