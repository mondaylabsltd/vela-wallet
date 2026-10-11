/**
 * The sites this wallet is connected to (spec 027 T350).
 *
 * A grant is a standing permission. A wallet that can grant but not revoke is a
 * wallet that only ever gets more permissive, so the list and the way out ship
 * together.
 *
 * Reading is enumeration, not judgement: what a grant MEANS is
 * `dapp_permissions`', and revoking is the absence of one. Nothing here decides
 * anything — it lists keys and removes them.
 */
import { PERM_PREFIX } from './keys';
import { revokeGrant, type DAppGrant } from './grants';
import { revokeSitesSetAside } from './space-sites';

interface StorageAreaLike {
	get(keys: null): Promise<Record<string, unknown>>;
	remove(keys: string[]): Promise<void>;
}

function area(): StorageAreaLike | null {
	const local = (globalThis as { chrome?: { storage?: { local?: unknown } } }).chrome?.storage
		?.local;
	return (local as StorageAreaLike | undefined) ?? null;
}

/** Every granted origin, oldest grant first. Empty off the extension. */
export async function listGrants(): Promise<DAppGrant[]> {
	const store = area();
	if (!store) return [];
	try {
		const all = await store.get(null);
		return Object.entries(all)
			.filter(([key]) => key.startsWith(PERM_PREFIX))
			.map(([, value]) => value)
			.filter(isGrant)
			.sort((a, b) => a.grantedAt - b.grantedAt);
	} catch {
		return [];
	}
}

export { revokeGrant };

/**
 * Cut every site off at once — the drawn "Disconnect all". Asked from inside
 * the parallel space it takes the person's own sites too, the ones set aside
 * for the visit: "all" is about this browser, not about the list in view.
 */
export async function revokeAll(): Promise<void> {
	const store = area();
	if (!store) return;
	await revokeSitesSetAside();
	const grants = await listGrants();
	if (grants.length === 0) return;
	try {
		await store.remove(grants.map((g) => PERM_PREFIX + g.origin));
	} catch {
		/* best-effort; a grant that survives is still listed */
	}
}

function isGrant(value: unknown): value is DAppGrant {
	if (!value || typeof value !== 'object') return false;
	const v = value as Partial<DAppGrant>;
	return (
		typeof v.origin === 'string' &&
		typeof v.address === 'string' &&
		typeof v.chainId === 'number' &&
		typeof v.grantedAt === 'number'
	);
}
