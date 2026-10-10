/**
 * The account's deployment could not be read — no chain node answered
 * `eth_getCode` (spec 082 RJ13). Retryable; neither "deployed" nor
 * "undeployed" may be guessed from it.
 *
 * `rateLimited`: the nodes turned the read away for load (the pool's fact), so
 * the fee row says "rate-limited · retrying", never "can't reach Vela" (G48).
 * A module of its own so the fee session can tell it apart without importing
 * the whole send path.
 */
export class DeploymentReadError extends Error {
	constructor(readonly rateLimited: boolean) {
		super(
			'Could not verify the account deployment status — the network may be unstable. Please try again.'
		);
		this.name = 'DeploymentReadError';
	}
}

/**
 * The account read never left the app: something inside it failed — the
 * request pool could not boot, or its core faulted — before any chain node
 * was asked (issue 483). Never told as "can't reach the chain": the fee says
 * it is Vela's own fault (`DeploymentRead::Internal`). `kind` is diagnostics
 * for the report, never shown.
 */
export class DeploymentInternalError extends Error {
	constructor(readonly kind: string) {
		super(`The account read never left the app (${kind}).`);
		this.name = 'DeploymentInternalError';
	}
}
