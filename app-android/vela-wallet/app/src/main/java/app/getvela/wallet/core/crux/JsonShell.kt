package app.getvela.wallet.core.crux

import kotlinx.serialization.KSerializer
import org.json.JSONObject

/**
 * The seam between the driver's JSON and an executor's types.
 *
 * `CoreDriver` speaks `org.json` because the onboarding executors — 1,400 lines
 * of ceremony handling written in spec 019 — take a `JSONObject`. Everything
 * written since decodes into the machine's own sealed `…Operation` hierarchy,
 * so its `when` is exhaustive and a Rust wire change is a **compile error**
 * rather than a string nobody matched.
 *
 * That difference is worth naming, because the onboarding suite exists to
 * compensate for not having it: `OnboardingExecutorTest`'s header explains that
 * its eighteen-way branch is a `when` over strings which "the compiler will not
 * notice an operation nobody handled", and the test is that check. Machines
 * wired through this adapter get the check from the compiler instead.
 *
 * The web sibling is `app-web/vela-wallet/src/lib/core/json-shell.ts`.
 */
object JsonShell {

    /**
     * Wrap a typed executor as the `perform` a [CoreDriver] takes.
     *
     * The contract the wrapper preserves (spec 040
     * `contracts/shell-operations.md` §0): the executor answers every operation
     * exactly once and throws for none. A decode failure here is not an
     * expected failure — it is a wire drift the drift test exists to prevent —
     * so it propagates to the driver, which reports it and still answers the
     * core through the machine's `escapedFailure`.
     */
    fun <O, R> perform(
        operations: KSerializer<O>,
        results: KSerializer<R>,
        execute: suspend (O) -> R,
    ): suspend (JSONObject) -> String = { json ->
        val operation = Wire.json.decodeFromString(operations, json.toString())
        Wire.json.encodeToString(results, execute(operation))
    }

    /**
     * Wrap a typed "what do we answer when the shell itself broke" mapping.
     *
     * Decoding is attempted so the mapping can be an exhaustive `when` over the
     * sealed hierarchy. When even that fails — an operation this build has no
     * name for — there is no honest answer, only a guess that keeps the core
     * from waiting forever; [fallback] is that guess, and reaching it means the
     * drift gate was bypassed.
     */
    fun <O, R> escapedFailure(
        operations: KSerializer<O>,
        results: KSerializer<R>,
        fallback: R,
        answer: (O) -> R,
    ): (JSONObject, Throwable) -> String = { json, _ ->
        val result = runCatching {
            answer(Wire.json.decodeFromString(operations, json.toString()))
        }.getOrDefault(fallback)
        Wire.json.encodeToString(results, result)
    }
}
