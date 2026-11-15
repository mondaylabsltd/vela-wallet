// Server-side tests run with rust/vela-site-crypto loaded, as every page that
// uses #lib/evm loads it before its first hash or signature.
import { loadEvm } from './wasm';

await loadEvm();
