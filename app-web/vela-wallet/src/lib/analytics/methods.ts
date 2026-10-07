/**
 * The onboarding core's key method, as usage statistics name it — the same
 * three words (`platform` / `hybrid` / `security_key`). The Trusted Signer
 * page is not a method the web offers, so it has no name here and the
 * property is simply left out.
 */
import type { KeyMethod } from '$lib/onboarding/generated/KeyMethod';
import type { AnalyticsMethod } from './catalog';

export function analyticsMethod(method: KeyMethod | null | undefined): AnalyticsMethod | undefined {
	return method === 'platform' || method === 'hybrid' || method === 'security_key'
		? method
		: undefined;
}
