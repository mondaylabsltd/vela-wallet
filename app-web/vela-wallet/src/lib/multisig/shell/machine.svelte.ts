/**
 * One of the core's multisig machines, running in this page: its view as
 * Svelte state, its events in, its operations out through the multisig shell
 * (`handler.ts`). The machine decides; this holds the view the screens draw.
 */
import { createJsonWasmShell, type JsonWasmCore } from '$lib/core/json-shell';
import type { EffectLoop } from '$lib/core/effect-loop';
import type { MultisigShellResult } from '../generated/MultisigShellResult';
import { multisigShell, type MultisigEffect, type MultisigShellDeps } from './handler';

export class MultisigMachine<View, Event> {
	/** The machine's view, as last committed. */
	view = $state.raw<View>() as View;
	readonly #loop: EffectLoop<Event>;

	constructor(core: JsonWasmCore, deps: MultisigShellDeps, name: string) {
		const shell = multisigShell(deps);
		this.view = JSON.parse(core.view()) as View;
		this.#loop = createJsonWasmShell<View, Event, MultisigEffect, MultisigShellResult>(core, {
			onView: (view) => {
				this.view = view;
			},
			execute: shell.execute,
			toFailure: shell.toFailure,
			onError: (error) => console.error(`[multisig ${name}] core fault:`, error)
		});
	}

	dispatch(event: Event): void {
		this.#loop.dispatch(event);
	}

	dispose(): void {
		this.#loop.dispose();
	}
}
