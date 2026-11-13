/**
 * The browser tests draw in the faces people read in: the system's own
 * (docs/FONT-POLICY.md — no font is downloaded or bundled), so SF Pro on
 * macOS, DejaVu / Liberation on a Linux runner. A component test mounts no
 * layout, so the global styles the root layout brings (the tokens, and
 * `app.css` with the body's line height) are loaded here.
 *
 * Text metrics therefore differ from machine to machine, and a test must not
 * pin an absolute width or height measured on one of them: it asserts a
 * relation instead — fits, does not wrap, as wide as another string in the
 * same face.
 */
import './src/lib/tokens/tokens.css';
import './src/app.css';
