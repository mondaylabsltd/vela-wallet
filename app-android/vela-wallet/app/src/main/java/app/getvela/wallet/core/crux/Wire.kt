package app.getvela.wallet.core.crux

import kotlinx.serialization.json.Json

/**
 * The one JSON configuration every machine's wire types are read and written
 * with.
 *
 * The Rust side is `#[serde(tag = "type", rename_all = "snake_case")]`, which
 * is exactly what `kotlinx.serialization` produces from a sealed hierarchy
 * whose subclasses carry `@SerialName("snake_case")` — so a Kotlin wire
 * declaration is a transcription of the Rust enum, not a parser. That is the
 * whole reason this app does not hand-walk `JSONObject`s for the twenty-four
 * machines: `app-web`'s generated mirrors number 311 types, and 311 parsers
 * are 311 places for a typo to become a silently-empty screen.
 *
 * The three settings below are the contract, and each is a deliberate answer
 * to a way a shell can lie about what the core said:
 *
 * 1. **`ignoreUnknownKeys = true`** — the core may carry fields this client
 *    does not render yet. Refusing to parse them would mean every Rust
 *    addition breaks Android before Android has any use for it.
 * 2. **`explicitNulls = false`** — an absent optional and an explicit `null`
 *    mean the same thing to serde, and should here too.
 * 3. **NO `coerceInputValues`** — this is the important one. With it, a value
 *    Kotlin does not understand (an enum variant added in Rust) would quietly
 *    become the declared default and the person would be shown the *wrong*
 *    state with no error anywhere. Without it, the decode throws and the fault
 *    is reported. Tolerant of absent, intolerant of wrong.
 *
 * The drift gate (`CoreWireDriftTest`) is the other half of rule 1: tolerance
 * of unknown keys means a *renamed* field reads as absent, so a test compares
 * every Kotlin `@SerialName` against the generated mirror in
 * `app-web/vela-wallet/src/lib/core/generated/`.
 */
object Wire {
    val json: Json = Json {
        ignoreUnknownKeys = true
        explicitNulls = false
        // classDiscriminator is "type" by default — the same tag serde uses.
    }
}
