package app.getvela.wallet.feature.settings.core

import uniffi.vela_core_uniffi.builtinNetworkDisplayName

/**
 * The names of Vela's own networks that are out of the person's lists
 * (`NetView.hidden_vela_chain_ids`), as the core names them — for the network
 * list's "Also supported, not in your list: Stable, MegaETH…" (the web's
 * `moreNetworks`).
 *
 * The view carries those networks by chain id only; the name is the core's
 * own table's (`builtin_network_display_name`), asked directly — no machine
 * started for it, and no table of names kept here to drift from the core's.
 */
object VelaNetworkNames {
    /** Each of [chainIds] the core names, in their order; a chain it does not name is left out. */
    fun of(chainIds: List<Long>): Map<Long, String> = chainIds.mapNotNull { id ->
        id.takeIf { it in 0..UInt.MAX_VALUE.toLong() }
            ?.let { runCatching { builtinNetworkDisplayName(it.toUInt()) }.getOrNull() }
            ?.takeIf { it.isNotBlank() }
            ?.let { id to it }
    }.toMap()
}
