/**
 * The intents' judgements over the wallet's state (spec 103, spec 104 part A):
 * whether what an action names exists, whether it is already so, and whether
 * the change it asked for has been SEEN. Decided once, in the core
 * (`vela_core::app::intent_rules`), for every shell; this is the web's door
 * to it.
 */
import { intentBaseline, intentExamine, intentVerdict, intentWorldNeeds } from '$lib/core/client';
import type { IntentAction } from '$lib/core/generated/IntentAction';
import type { IntentBaseline } from '$lib/core/generated/IntentBaseline';
import type { IntentExamined } from '$lib/core/generated/IntentExamined';
import type { IntentVerdict } from '$lib/core/generated/IntentVerdict';
import { worldJson, type World } from './world';

export type Examined = IntentExamined;
export type Verdict = IntentVerdict;
/** What was true when the call began, to tell its change from others'. */
export type Baseline = IntentBaseline;

/** The action and the views its judgement reads, as the core takes them. */
function args(action: IntentAction, world: World): [string, string] {
	const json = JSON.stringify(action);
	return [json, worldJson(world, JSON.parse(intentWorldNeeds(json)) as string[])];
}

/** Does what the action names exist, and is it already so? */
export function examine(action: IntentAction, world: World): Examined {
	return JSON.parse(intentExamine(...args(action, world))) as Examined;
}

export function baselineOf(action: IntentAction, world: World): Baseline {
	return JSON.parse(intentBaseline(...args(action, world))) as Baseline;
}

/**
 * Has the change held? `baseline` is updated as the call goes on — the
 * sign-out sheet coming up is part of the call, not its end.
 */
export function verdict(action: IntentAction, world: World, baseline: Baseline): Verdict {
	const out = JSON.parse(intentVerdict(...args(action, world), JSON.stringify(baseline))) as {
		verdict: Verdict;
		baseline: Baseline;
	};
	Object.assign(baseline, out.baseline);
	return out.verdict;
}
