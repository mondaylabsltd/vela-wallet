/**
 * The node test project's core, initialized before every test file.
 *
 * Since spec 068's speed rules moved into the core (2026-09-21), view-model
 * code that used to be plain TypeScript — the speed picker, the gas-price
 * figures, the free-speed steps — asks `feeSpeedRule`, a pure wasm kernel.
 * In the app every such call runs behind a surface the core already drives;
 * in a unit test nothing has loaded it. Initializing it here, once per file,
 * is what lets those suites keep testing the real rules without each growing
 * a line of plumbing. `initSync` over the committed module costs a few
 * milliseconds (compilation is lazy), and a test that already imports this
 * module itself is unaffected: Node caches it, so it runs once.
 */
import '$lib/i18n/wasm-init.server';
