/**
 * The Safari package's background entry: the adapter first, then the very
 * worker the Chrome package runs. Bundled by `build.mjs --safari` into
 * `background.js`; ES module order guarantees `shim.js` has run before the
 * worker's top level touches `chrome.*`.
 */
import './shim.js';
import '../background.js';
