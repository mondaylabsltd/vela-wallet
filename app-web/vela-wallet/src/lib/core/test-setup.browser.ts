/**
 * The browser test project's core, initialized before every test file.
 *
 * The browser twin of `test-setup.server.ts`: the speed control's decisions
 * are the core's since spec 068's rules moved there (2026-09-21), and the
 * component and effect-wiring suites that render it must run the real rules.
 * The module is fetched from the dev server exactly as `loadCore()` fetches it
 * — straight from the wasm-bindgen glue rather than through `$lib/core/client`,
 * which several of these suites replace with a mock. `init` is idempotent, so
 * a suite that also calls `loadCore()` shares this instance.
 */
import init from '../../../../../rust/pkg-web/vela_core.js';
import { WASM_URL } from '../../../../../rust/pkg-web/vela_core_wasm_url.js';

await init({ module_or_path: WASM_URL });
