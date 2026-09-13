package app.getvela.wallet.feature.signing.core

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/*
 * The `approval_guard` machine's wire (spec 044) — an unlimited approval
 * never leaves the wallet. `chain_id`/`decimals`/`index`/`word_index`/
 * `amount_bits` are `u32`, `now_ms` `f64`.
 */

@Serializable
enum class GuardApprovalKind {
    @SerialName("erc20_approve") Erc20Approve,

    @SerialName("increase_allowance") IncreaseAllowance,

    @SerialName("decrease_allowance") DecreaseAllowance,

    @SerialName("set_approval_for_all") SetApprovalForAll,

    @SerialName("erc2612_permit") Erc2612Permit,

    @SerialName("dai_permit") DaiPermit,

    @SerialName("permit2_single") Permit2Single,

    @SerialName("permit2_batch") Permit2Batch,
}

@Serializable
enum class GuardAmountError {
    @SerialName("invalid_amount") InvalidAmount,

    @SerialName("unlimited_disabled") UnlimitedDisabled,
}

@Serializable
enum class GuardBlockReason {
    @SerialName("off_chain_permit") OffChainPermit,

    @SerialName("dai_permit_full_balance") DaiPermitFullBalance,
}

@Serializable
enum class GuardEditorMode {
    @SerialName("requested") Requested,

    @SerialName("balance") Balance,

    @SerialName("custom") Custom,

    @SerialName("revoke") Revoke,

    @SerialName("grant") Grant,
}

@Serializable
enum class GuardSurface {
    @SerialName("none") None,

    @SerialName("permit_sign") PermitSign,

    @SerialName("approval_editor") ApprovalEditor,

    @SerialName("batch") Batch,
}

@Serializable
sealed class GuardChoice {
    @Serializable
    @SerialName("amount")
    data class Amount(val amount_raw: String) : GuardChoice()

    @Serializable
    @SerialName("revoke")
    data object Revoke : GuardChoice()

    @Serializable
    @SerialName("grant")
    data object Grant : GuardChoice()
}

@Serializable
sealed class GuardLocus {
    @Serializable
    @SerialName("calldata_word")
    data class CalldataWord(val word_index: Int) : GuardLocus()

    @Serializable
    @SerialName("typed_path")
    data class TypedPath(val path: String) : GuardLocus()
}

@Serializable
data class GuardTokenMetaEntry(val token: String, val symbol: String, val decimals: Int)

@Serializable
data class GuardTokenMetaView(
    val symbol: String = "",
    val decimals: Int = 18,
    val verified: Boolean = false,
    val loading: Boolean = false,
)

@Serializable
data class GuardDetectedApproval(
    val kind: GuardApprovalKind,
    val token_address: String? = null,
    val spender: String,
    val amount_raw: String? = null,
    val amount_bits: Int? = null,
    val is_unbounded: Boolean = false,
    val is_boolean_grant: Boolean = false,
    val is_reducing: Boolean = false,
    val editable: Boolean = false,
    val block_reason: GuardBlockReason? = null,
    val deadline: String? = null,
    val locus: GuardLocus,
)

@Serializable
data class GuardEditorView(
    val mode: GuardEditorMode? = null,
    val custom_text: String = "",
    val error: GuardAmountError? = null,
    val choice: GuardChoice? = null,
    val display_amount_raw: String? = null,
    val requested_finite: Boolean = false,
    val has_balance_cap: Boolean = false,
    val balance_raw: String? = null,
)

@Serializable
data class GuardIncreaseTotalView(val current: String? = null, val increment: String, val total: String? = null)

@Serializable
data class GuardLegView(
    val to: String,
    val approval: GuardDetectedApproval? = null,
    val meta: GuardTokenMetaView = GuardTokenMetaView(),
    val editor: GuardEditorView? = null,
    val choice: GuardChoice? = null,
    val needs_editor: Boolean = false,
    val needs_choice: Boolean = false,
    val grants_broad: Boolean = false,
)

@Serializable
data class GuardBatchView(
    val legs: List<GuardLegView> = emptyList(),
    val any_uncapped: Boolean = false,
    val any_to_own_token: Boolean = false,
    val all_settled: Boolean = false,
)

@Serializable
data class GuardView(
    val surface: GuardSurface = GuardSurface.None,
    val detected: GuardDetectedApproval? = null,
    val meta: GuardTokenMetaView = GuardTokenMetaView(),
    val editor: GuardEditorView? = null,
    val confirm_allowed: Boolean = true,
    val rewritten_params_json: String? = null,
    val increase_total: GuardIncreaseTotalView? = null,
    val decimals_unverified: Boolean = false,
    val expired: Boolean = false,
    val batch: GuardBatchView? = null,
)

@Serializable
sealed class GuardOperation {
    @Serializable
    @SerialName("read_token_metadata")
    data class ReadTokenMetadata(val chain_id: Int, val tokens: List<String>) : GuardOperation()

    @Serializable
    @SerialName("read_erc20_allowance")
    data class ReadErc20Allowance(val chain_id: Int, val token: String, val owner: String, val spender: String) : GuardOperation()

    @Serializable
    @SerialName("read_erc20_balance")
    data class ReadErc20Balance(val chain_id: Int, val token: String, val owner: String) : GuardOperation()
}

@Serializable
sealed class GuardShellResult {
    @Serializable
    @SerialName("meta_resolved")
    data class MetaResolved(val metas: List<GuardTokenMetaEntry>? = null) : GuardShellResult()

    @Serializable
    @SerialName("allowance_read")
    data class AllowanceRead(val allowance: String? = null) : GuardShellResult()

    @Serializable
    @SerialName("balance_read")
    data class BalanceRead(val balance: String? = null) : GuardShellResult()
}

@Serializable
sealed class GuardEvent {
    @Serializable
    @SerialName("approval_detected")
    data class ApprovalDetected(
        val method: String,
        val params_json: String,
        val chain_id: Int,
        val wallet_address: String? = null,
        val read_only: Boolean = false,
        val now_ms: Double,
    ) : GuardEvent()

    @Serializable
    @SerialName("preset_selected")
    data class PresetSelected(val mode: GuardEditorMode) : GuardEvent()

    @Serializable
    @SerialName("custom_amount_changed")
    data class CustomAmountChanged(val text: String) : GuardEvent()

    @Serializable
    @SerialName("grant_deliberately_chosen")
    data object GrantDeliberatelyChosen : GuardEvent()

    @Serializable
    @SerialName("revoke_chosen")
    data object RevokeChosen : GuardEvent()

    @Serializable
    @SerialName("leg_preset_selected")
    data class LegPresetSelected(val index: Int, val mode: GuardEditorMode) : GuardEvent()

    @Serializable
    @SerialName("leg_custom_amount_changed")
    data class LegCustomAmountChanged(val index: Int, val text: String) : GuardEvent()

    @Serializable
    @SerialName("leg_grant_deliberately_chosen")
    data class LegGrantDeliberatelyChosen(val index: Int) : GuardEvent()

    @Serializable
    @SerialName("leg_revoke_chosen")
    data class LegRevokeChosen(val index: Int) : GuardEvent()

    @Serializable
    @SerialName("batch_recipients_resolved")
    data class BatchRecipientsResolved(val recipients: List<List<String>>) : GuardEvent()
}
