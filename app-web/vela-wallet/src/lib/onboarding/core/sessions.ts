/**
 * Constructs an onboarding core and wires it to this shell.
 *
 * Both machines take the same executor, because they share one operation
 * vocabulary — six of the eighteen operations are used by both flows, so the
 * shell implements ONE executor and the generated TypeScript has one union
 * instead of two overlapping ones.
 *
 * `loadOnboardingCore()` must have resolved before either constructor runs;
 * the screens await it behind the loading state the design already has.
 */

import { CreateWalletCore, LoginCore } from './wasm-client';
import { createJsonWasmShell } from '$lib/core/json-shell';
import type { EffectLoop } from '$lib/core/effect-loop';
import {
	createOnboardingExecutor,
	operationFailure,
	type ExecutorDeps,
	type OnboardingEffect
} from './executor';

import type { CreateView } from '../generated/CreateView';
import type { CreateWalletEvent } from '../generated/CreateWalletEvent';
import type { LoginEvent } from '../generated/LoginEvent';
import type { LoginView } from '../generated/LoginView';
import type { ShellResult } from '../generated/ShellResult';

export type CreateWalletSession = EffectLoop<CreateWalletEvent>;
export type LoginSession = EffectLoop<LoginEvent>;

export type SessionOptions<View> = {
	onView(view: View): void;
	deps: ExecutorDeps;
	/** A core-level fault — malformed event, serialization failure. Never a
	 *  user error, so it is reported rather than rendered as a flow state. */
	onError?(error: unknown): void;
};

export function createCreateWalletSession(
	options: SessionOptions<CreateView>
): CreateWalletSession {
	return createJsonWasmShell<CreateView, CreateWalletEvent, OnboardingEffect, ShellResult>(
		new CreateWalletCore(),
		{
			onView: options.onView,
			execute: createOnboardingExecutor(options.deps),
			toFailure: operationFailure,
			// Never silent (spec 048): a caller that passes no handler still gets the fault reported.
			onError: options.onError ?? ((error) => console.error('[onboarding] core fault:', error))
		}
	);
}

export function createLoginSession(options: SessionOptions<LoginView>): LoginSession {
	return createJsonWasmShell<LoginView, LoginEvent, OnboardingEffect, ShellResult>(
		new LoginCore(),
		{
			onView: options.onView,
			execute: createOnboardingExecutor(options.deps),
			toFailure: operationFailure,
			// Never silent (spec 048): a caller that passes no handler still gets the fault reported.
			onError: options.onError ?? ((error) => console.error('[onboarding] core fault:', error))
		}
	);
}
