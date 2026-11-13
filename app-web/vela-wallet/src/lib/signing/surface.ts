/**
 * Where this build runs, as the core's signing rule takes it (spec 102 R8,
 * vela-core `signing_venue::SigningSurface`).
 *
 * The Safari package's pages are `safari-web-extension://<UUID>`, and WebKit
 * refuses them a `getvela.app` passkey — `SecurityError: The requested RPID
 * did not match the origin`, for creating a key as for using one (measured on
 * iOS 26.2, `extension/README.md` § Safari). So in that package nothing is
 * signed, and no key is made or used, in Vela's own sheet: the core answers
 * `page` for every signature (`signingRoute`, `multisigApprovalRoute`) and
 * every key ceremony (the create and sign-in machines, told by
 * `surface_known`), and this shell carries each one to the page.
 *
 * The web wallet and the Chrome extension are `standard` — exactly the route
 * they had before R8.
 */
import type { SigningSurface } from '$lib/core/generated/SigningSurface';

export const SIGNING_SURFACE: SigningSurface =
	__VELA_EXTENSION_PROTOCOL__ === 'safari-web-extension:' ? 'safari_extension' : 'standard';
