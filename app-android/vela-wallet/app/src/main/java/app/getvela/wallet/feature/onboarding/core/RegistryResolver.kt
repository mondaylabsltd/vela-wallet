package app.getvela.wallet.feature.onboarding.core

/**
 * The three layers — what [RegistryClient] needs to run
 * `vela_core::registry_resolve` (067): the index for speed, the chain for
 * truth, Ethereum for survival.
 *
 * WHICH is asked, in what order, and whether an index answer is believed — it
 * is not: its `contentHash` is recomputed and compared with the chain's — are
 * the core's, written once for four shells. Until 067 this shell carried its
 * own copy of the ladder (`RegistryChainReader`), which asked the chain only
 * when the index was SILENT and believed any index that answered.
 *
 * The three functions are seams so a test can run the REAL core over recorded
 * contract bytes, or script a walk.
 */
class RegistryResolver(
    /** The RAW `result` hex, or `null` when that chain did not answer. */
    val ethCall: suspend (chainId: Int, to: String, data: String) -> String?,
    val keyStep: (publicKeyHex: String, answersJson: String) -> String =
        { key, answers -> uniffi.vela_core_uniffi.registryResolveKeyStep(key, answers) },
    val unitStep: (unitId: Long, source: String, answersJson: String) -> String =
        { id, source, answers -> uniffi.vela_core_uniffi.registryResolveUnitStep(id.toUInt(), source, answers) },
)
