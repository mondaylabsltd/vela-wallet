/**
 * The one way anything on the web copies text (spec 028 Phase 9, T486).
 *
 * Until this phase four surfaces showed the copy tick and wrote nothing — the
 * receive rows, the QR's address and contract, the token facts. Each had its
 * own `copy()` that only flipped an icon. The tick is the surface's; the
 * write is this function's, so the two cannot drift apart again.
 *
 * A refused clipboard (no permission, an insecure context, a browser that has
 * none) answers `false` and is not an error worth a dialog: every caller has
 * the text on screen, which is the fallback. The identicon viewer set the
 * precedent (spec 019).
 */
export async function copyText(text: string): Promise<boolean> {
	if (text === '' || typeof navigator === 'undefined' || !navigator.clipboard) return false;
	try {
		await navigator.clipboard.writeText(text);
		return true;
	} catch {
		return false;
	}
}
