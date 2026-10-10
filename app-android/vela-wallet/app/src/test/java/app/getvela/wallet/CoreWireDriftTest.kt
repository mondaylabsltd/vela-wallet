package app.getvela.wallet

import app.getvela.wallet.feature.contacts.core.Contact
import app.getvela.wallet.feature.contacts.core.ContactEvent
import app.getvela.wallet.feature.contacts.core.ContactGroupInput
import app.getvela.wallet.feature.contacts.core.ContactGroupView
import app.getvela.wallet.feature.contacts.core.ContactIdentity
import app.getvela.wallet.feature.contacts.core.ContactImportEntry
import app.getvela.wallet.feature.contacts.core.ContactImportGroup
import app.getvela.wallet.feature.contacts.core.ContactImportReport
import app.getvela.wallet.feature.contacts.core.ContactKind
import app.getvela.wallet.feature.contacts.core.ContactOperation
import app.getvela.wallet.feature.contacts.core.ContactRecipientView
import app.getvela.wallet.feature.contacts.core.ContactSaveInput
import app.getvela.wallet.feature.contacts.core.ContactShellResult
import app.getvela.wallet.feature.contacts.core.ContactSource
import app.getvela.wallet.feature.contacts.core.ContactTombstone
import app.getvela.wallet.feature.contacts.core.ContactTxKind
import app.getvela.wallet.feature.contacts.core.ContactsView
import app.getvela.wallet.feature.contacts.core.ContactExportFile
import app.getvela.wallet.feature.contacts.core.ContactExportScope
import app.getvela.wallet.feature.contacts.core.ContactFileFormat
import app.getvela.wallet.feature.contacts.core.ContactImportFailure
import app.getvela.wallet.feature.contacts.core.ContactSection
import app.getvela.wallet.feature.send.core.BatchEvent
import app.getvela.wallet.feature.send.core.BatchFileContent
import app.getvela.wallet.feature.send.core.BatchOperation
import app.getvela.wallet.feature.send.core.BatchPreviewRow
import app.getvela.wallet.feature.send.core.BatchRateStatus
import app.getvela.wallet.feature.send.core.BatchRecipient
import app.getvela.wallet.feature.send.core.BatchShellResult
import app.getvela.wallet.feature.send.core.BatchToken
import app.getvela.wallet.feature.send.core.BatchUnit
import app.getvela.wallet.feature.send.core.BatchView
import app.getvela.wallet.feature.send.core.FeeAssetKind
import app.getvela.wallet.feature.send.core.FeeAssetQuote
import app.getvela.wallet.feature.send.core.FeeAssetView
import app.getvela.wallet.feature.send.core.FeeBundlerQuote
import app.getvela.wallet.feature.send.core.FeeCall
import app.getvela.wallet.feature.send.core.FeeEstimateView
import app.getvela.wallet.feature.send.core.FeeEvent
import app.getvela.wallet.feature.send.core.FeeFailure
import app.getvela.wallet.feature.send.core.FeeGasOutcome
import app.getvela.wallet.feature.browser.core.*
import app.getvela.wallet.feature.signing.core.*
import app.getvela.wallet.feature.send.core.FeeOperation
import app.getvela.wallet.feature.send.core.FeeOptionView
import app.getvela.wallet.feature.send.core.FeeShellResult
import app.getvela.wallet.feature.send.core.FeeTier
import app.getvela.wallet.feature.send.core.FeeView
import app.getvela.wallet.feature.send.core.MtokCustomToken
import app.getvela.wallet.feature.send.core.MtokEvent
import app.getvela.wallet.feature.send.core.MtokFound
import app.getvela.wallet.feature.send.core.MtokNetwork
import app.getvela.wallet.feature.send.core.MtokOperation
import app.getvela.wallet.feature.send.core.MtokShellResult
import app.getvela.wallet.feature.send.core.MtokTokenMeta
import app.getvela.wallet.feature.send.core.MtokView
import app.getvela.wallet.feature.send.core.SendAccountRef
import app.getvela.wallet.feature.send.core.SendAddNetworkMsg
import app.getvela.wallet.feature.send.core.SendAddNetworkOutcome
import app.getvela.wallet.feature.send.core.SendAlertKind
import app.getvela.wallet.feature.send.core.SendAmountWarning
import app.getvela.wallet.feature.send.core.SendChainInfo
import app.getvela.wallet.feature.send.core.SendDisplayContext
import app.getvela.wallet.feature.send.core.SendEstimateFailure
import app.getvela.wallet.feature.send.core.SendEvent
import app.getvela.wallet.feature.send.core.SendFeeIssueView
import app.getvela.wallet.feature.send.core.SendFeeOutcome
import app.getvela.wallet.feature.send.core.SendHapticKind
import app.getvela.wallet.feature.send.core.SendHoldReason
import app.getvela.wallet.feature.send.core.SendLockError
import app.getvela.wallet.feature.send.core.SendMultiSpecView
import app.getvela.wallet.feature.send.core.SendOpenParams
import app.getvela.wallet.feature.send.core.SendOperation
import app.getvela.wallet.feature.send.core.SendQuotedFee
import app.getvela.wallet.feature.send.core.SendReceiptKind
import app.getvela.wallet.feature.send.core.SendReceiptOutcome
import app.getvela.wallet.feature.send.core.SendReceiptStatus
import app.getvela.wallet.feature.send.core.SendReceiptTransfer
import app.getvela.wallet.feature.send.core.SendReceiptView
import app.getvela.wallet.feature.send.core.SendRecipientDraft
import app.getvela.wallet.feature.send.core.SendRecipientIdentity
import app.getvela.wallet.feature.send.core.SendRecipientRisk
import app.getvela.wallet.feature.send.core.SendScan
import app.getvela.wallet.feature.send.core.SendShellResult
import app.getvela.wallet.feature.send.core.SendStage
import app.getvela.wallet.feature.send.core.SendSubmitFailure
import app.getvela.wallet.feature.send.core.SendTimerTag
import app.getvela.wallet.feature.send.core.SendToken
import app.getvela.wallet.feature.send.core.SendTokenMeta
import app.getvela.wallet.feature.send.core.SendTreasuryAsset
import app.getvela.wallet.feature.send.core.SendTreasuryCoin
import app.getvela.wallet.feature.send.core.SendTreasuryProbe
import app.getvela.wallet.feature.send.core.SendTreasuryStatus
import app.getvela.wallet.feature.send.core.SendTxErrorKey
import app.getvela.wallet.feature.send.core.SendTxRecord
import app.getvela.wallet.feature.send.core.SendTxStatus
import app.getvela.wallet.feature.send.core.SendUnitIssue
import app.getvela.wallet.feature.send.core.SendView
import app.getvela.wallet.feature.send.core.SendSplitRowIssue
import app.getvela.wallet.feature.send.core.SendDuplicateRowView
import app.getvela.wallet.feature.send.core.SendRowFieldState
import app.getvela.wallet.feature.send.core.SendNameSource
import app.getvela.wallet.feature.send.core.SendPayee
import app.getvela.wallet.feature.send.core.SendReceiptCoin
import app.getvela.wallet.feature.send.core.TrackEntryView
import app.getvela.wallet.feature.send.core.TrackOutcome
import app.getvela.wallet.feature.send.core.TrackEvent
import app.getvela.wallet.feature.send.core.TrackLifecycle
import app.getvela.wallet.feature.send.core.TrackOperation
import app.getvela.wallet.feature.send.core.TrackPendingRecord
import app.getvela.wallet.feature.send.core.TrackRecordPatch
import app.getvela.wallet.feature.send.core.TrackRecordStatus
import app.getvela.wallet.feature.send.core.TrackShellResult
import app.getvela.wallet.feature.send.core.TrackStatus
import app.getvela.wallet.feature.send.core.TrackView
import app.getvela.wallet.feature.settings.core.CurrencyEvent
import app.getvela.wallet.feature.wallet.core.BalanceCacheEntry
import app.getvela.wallet.feature.wallet.core.BalanceEvent
import app.getvela.wallet.feature.wallet.core.BalanceNotice
import app.getvela.wallet.feature.wallet.core.BalanceOperation
import app.getvela.wallet.feature.wallet.core.BalanceShellResult
import app.getvela.wallet.feature.wallet.core.BalanceSwitcherView
import app.getvela.wallet.feature.wallet.core.BalanceToken
import app.getvela.wallet.feature.wallet.core.BalanceView
import app.getvela.wallet.feature.wallet.core.FeedBatch
import app.getvela.wallet.feature.wallet.core.FeedBatchKind
import app.getvela.wallet.feature.wallet.core.FeedBatchTransfer
import app.getvela.wallet.feature.wallet.core.FeedDirection
import app.getvela.wallet.feature.wallet.core.FeedEvent
import app.getvela.wallet.feature.wallet.core.FeedItem
import app.getvela.wallet.feature.wallet.core.FeedOperation
import app.getvela.wallet.feature.wallet.core.FeedRow
import app.getvela.wallet.feature.wallet.core.FeedShellResult
import app.getvela.wallet.feature.wallet.core.FeedToast
import app.getvela.wallet.feature.wallet.core.FeedTxKind
import app.getvela.wallet.feature.wallet.core.FeedTxRecord
import app.getvela.wallet.feature.wallet.core.FeedTxStatus
import app.getvela.wallet.feature.wallet.core.DepositEntry
import app.getvela.wallet.feature.wallet.core.DepositItem
import app.getvela.wallet.feature.wallet.core.FeedView
import app.getvela.wallet.feature.wallet.core.PayRequest
import app.getvela.wallet.feature.wallet.core.PaymentRequestEvent
import app.getvela.wallet.feature.wallet.core.PaymentRequestOperation
import app.getvela.wallet.feature.wallet.core.PaymentRequestShellResult
import app.getvela.wallet.feature.wallet.core.PaymentRequestView
import app.getvela.wallet.feature.wallet.core.ReceiveAsset
import app.getvela.wallet.feature.wallet.core.ReceiveMode
import app.getvela.wallet.feature.wallet.core.ReceiveWatchEvent
import app.getvela.wallet.feature.wallet.core.ReceiveWatchOperation
import app.getvela.wallet.feature.wallet.core.ReceiveWatchShellResult
import app.getvela.wallet.feature.wallet.core.ReceiveWatchView
import app.getvela.wallet.feature.wallet.core.TokenSnapshot
import app.getvela.wallet.feature.wallet.core.TrustAssetDelta
import app.getvela.wallet.feature.wallet.core.TrustCustomToken
import app.getvela.wallet.feature.wallet.core.TrustDeltaKind
import app.getvela.wallet.feature.wallet.core.TrustEvent
import app.getvela.wallet.feature.wallet.core.TrustIncomingView
import app.getvela.wallet.feature.wallet.core.TrustLogsOutcome
import app.getvela.wallet.feature.wallet.core.TrustMetaEntry
import app.getvela.wallet.feature.wallet.core.TrustOperation
import app.getvela.wallet.feature.wallet.core.TrustRawLog
import app.getvela.wallet.feature.wallet.core.TrustReceiptLog
import app.getvela.wallet.feature.wallet.core.TrustShellResult
import app.getvela.wallet.feature.wallet.core.TrustSimJudgment
import app.getvela.wallet.feature.wallet.core.TrustSimView
import app.getvela.wallet.feature.wallet.core.TrustTokenMeta
import app.getvela.wallet.feature.wallet.core.TrustView
import app.getvela.wallet.feature.wallet.core.RpcBanEntry
import app.getvela.wallet.feature.wallet.core.RpcCallVerdict
import app.getvela.wallet.feature.wallet.core.RpcEndpointSeed
import app.getvela.wallet.feature.wallet.core.RpcErrorInfo
import app.getvela.wallet.feature.wallet.core.RpcEvent
import app.getvela.wallet.feature.wallet.core.RpcKind
import app.getvela.wallet.feature.wallet.core.RpcOperation
import app.getvela.wallet.feature.wallet.core.RpcPoolView
import app.getvela.wallet.feature.wallet.core.RpcShellResult
import app.getvela.wallet.feature.wallet.core.RpcSource
import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.feature.wallet.core.RpcTransportOutcome
import app.getvela.wallet.feature.settings.core.CurrencyOperation
import app.getvela.wallet.feature.settings.core.CurrencyShellResult
import app.getvela.wallet.feature.settings.core.CurrencyView
import app.getvela.wallet.feature.settings.core.NetChainIndexEntry
import app.getvela.wallet.feature.settings.core.NetChainInfo
import app.getvela.wallet.feature.settings.core.NetChainMismatch
import app.getvela.wallet.feature.settings.core.NetCompatibility
import app.getvela.wallet.feature.settings.core.NetContractStatus
import app.getvela.wallet.feature.settings.core.NetCustomNetwork
import app.getvela.wallet.feature.settings.core.NetEndpointField
import app.getvela.wallet.feature.settings.core.NetEndpointView
import app.getvela.wallet.feature.settings.core.NetEvent
import app.getvela.wallet.feature.settings.core.NetHealthBody
import app.getvela.wallet.feature.settings.core.NetNetworkConfig
import app.getvela.wallet.feature.settings.core.NetNetworkRow
import app.getvela.wallet.feature.settings.core.NetOperation
import app.getvela.wallet.feature.settings.core.NetOverrideField
import app.getvela.wallet.feature.settings.core.NetProbeHealth
import app.getvela.wallet.feature.settings.core.NetProviderId
import app.getvela.wallet.feature.settings.core.NetProviderKeys
import app.getvela.wallet.feature.settings.core.NetProviderNetRow
import app.getvela.wallet.feature.settings.core.NetProviderTestView
import app.getvela.wallet.feature.settings.core.NetProviderView
import app.getvela.wallet.feature.settings.core.NetRawChainData
import app.getvela.wallet.feature.settings.core.NetRpcFailureKind
import app.getvela.wallet.feature.settings.core.NetServiceEndpoints
import app.getvela.wallet.feature.settings.core.NetServiceHealth
import app.getvela.wallet.feature.settings.core.NetShellResult
import app.getvela.wallet.feature.settings.core.NetStoredEndpoints
import app.getvela.wallet.feature.settings.core.NetView
import app.getvela.wallet.feature.settings.core.NetWizardErrorKind
import app.getvela.wallet.feature.settings.core.NetWizardPhase
import app.getvela.wallet.feature.settings.core.NetWizardView
import java.io.File
import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.KSerializer
import kotlinx.serialization.descriptors.elementNames
import kotlinx.serialization.descriptors.PolymorphicKind
import kotlinx.serialization.descriptors.SerialDescriptor
import kotlinx.serialization.serializer
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The gate that makes a Rust rename fail an Android build (spec 040 FR-008).
 *
 * `Wire.json` is configured with `ignoreUnknownKeys = true`, which is the only
 * sane setting for a client that renders a subset of what twenty-four machines
 * emit — and it has exactly one dangerous consequence: a field RENAMED in Rust
 * stops arriving, reads as absent, and the screen quietly shows a default. No
 * exception, no log, no failing test. Just a wrong number.
 *
 * So tolerance is paired with this: every Kotlin wire declaration is compared
 * against the generated mirror of the same Rust type in
 * `app-web/vela-wallet/src/lib/core/generated/`, which `ts-rs` writes from the
 * `vela-core` source and the repository commits. Same idea as
 * [DesignTokenDriftTest], same `vela.repo.root` property, different model of
 * record.
 *
 * Two rules, and the asymmetry between them is the point:
 *
 * - **Views may be a subset.** Android need not render every field the core
 *   offers. What it must not do is name a field the core does not have.
 * - **Operations may not.** An operation variant the shell cannot decode is an
 *   effect nobody answers, which the person experiences as a spinner that never
 *   stops. Coverage there must be total.
 */
@OptIn(ExperimentalSerializationApi::class)
class CoreWireDriftTest {

    // -- the registry --------------------------------------------------------
    //
    // One line per Kotlin wire declaration. A machine wired in a later spec
    // adds its types here; a type NOT listed here is a type with no gate.

    @Test
    fun currencyViewMatchesTheGeneratedMirror() {
        assertFieldsExist<CurrencyView>("CurrencyView")
        // The stored choice on its way, for a label that names its currency while the figure waits.
        assertTrue("pending" in serializer<CurrencyView>().descriptor.elementNames)
    }

    @Test
    fun currencyRateStaysNullable() {
        // Not decoration. `null` is not `1`: the core's own comment says a
        // fiat amount multiplied by a defaulted rate is a real mispayment.
        // A future edit that "tidies" this to a non-null Double with a default
        // would compile, pass every other test, and quietly reintroduce it.
        val rate = elementDescriptor<CurrencyView>("rate")
        assertTrue("CurrencyView.rate must stay nullable", rate.isNullable)
        assertTrue("CurrencyView.rate is nullable in the mirror too", tsFieldIsNullable("CurrencyView", "rate"))
    }

    @Test
    fun currencyOperationsAreExhaustive() {
        assertVariantsExhaustive<CurrencyOperation>("CurrencyOperation")
    }

    @Test
    fun currencyResultsAreExhaustive() {
        // The shell must be able to SAY everything the core can hear, too: a
        // result variant Kotlin cannot encode is an answer that never arrives.
        assertVariantsExhaustive<CurrencyShellResult>("CurrencyShellResult")
    }

    @Test
    fun currencyEventsExist() {
        assertVariantsExist<CurrencyEvent>("CurrencyEvent")
    }

    // -- contacts ------------------------------------------------------------

    @Test
    fun contactViewsMatchTheGeneratedMirrors() {
        assertFieldsExist<ContactsView>("ContactsView")
        assertFieldsExist<Contact>("Contact")
        assertFieldsExist<ContactGroupView>("ContactGroupView")
        assertFieldsExist<ContactRecipientView>("ContactRecipientView")
        assertFieldsExist<ContactIdentity>("ContactIdentity")
        assertFieldsExist<ContactImportReport>("ContactImportReport")
        assertFieldsExist<ContactTombstone>("ContactTombstone")
        assertFieldsExist<ContactSaveInput>("ContactSaveInput")
        assertFieldsExist<ContactGroupInput>("ContactGroupInput")
        assertFieldsExist<ContactImportEntry>("ContactImportEntry")
        assertFieldsExist<ContactImportGroup>("ContactImportGroup")
        assertFieldsExist<ContactSection>("ContactSection")
        assertFieldsExist<ContactExportFile>("ContactExportFile")
        assertVariantsExhaustive<ContactExportScope>("ContactExportScope")
        assertVariantsExhaustive<ContactImportFailure>("ContactImportFailure")
        assertEquals(listOf("json", "csv"), ContactFileFormat.serializer().descriptor.elementNames.toList())
    }

    @Test
    fun contactOperationsAreExhaustive() {
        assertVariantsExhaustive<ContactOperation>("ContactOperation")
    }

    @Test
    fun contactResultsAreExhaustive() {
        assertVariantsExhaustive<ContactShellResult>("ContactShellResult")
    }

    @Test
    fun contactEventsExist() {
        assertVariantsExist<ContactEvent>("ContactEvent")
    }

    @Test
    fun contactEnumsMatchTheGeneratedMirrors() {
        assertStringUnion<ContactKind>("ContactKind")
        assertStringUnion<ContactSource>("ContactSource")
        assertStringUnion<ContactTxKind>("ContactTxKind")
    }

    @Test
    fun theMirrorCannotTellU32FromF64AndThisSaysSo() {
        // A gate has to know what it does not check. ts-rs writes every Rust
        // number as TypeScript `number`, so `tx_count: u32` and
        // `last_used_ms: f64` are indistinguishable here — and serde is not
        // indistinguishable about them: sending `0.0` for a u32 is rejected
        // outright ("invalid type: floating point `0.0`, expected u32"), which
        // is how the mistake was found in spec 040 phase 7, on the real
        // machine rather than in this file.
        //
        // What this test pins is the SHAPE of that blind spot, so the next
        // person to add a numeric field looks at the Rust rather than at the
        // mirror.
        assertEquals("number", tsFieldType("Contact", "tx_count"))
        assertEquals("number", tsFieldType("Contact", "last_used_ms"))
        val txCount = elementDescriptor<Contact>("tx_count")
        val lastUsed = elementDescriptor<Contact>("last_used_ms")
        assertEquals("the u32 is an Int in Kotlin", "kotlin.Int", txCount.serialName)
        assertEquals("the f64 is a Double", "kotlin.Double", lastUsed.serialName)
    }

    // -- balance_dashboard (spec 041) ------------------------------------------

    @Test
    fun balanceViewsMatchTheGeneratedMirrors() {
        assertFieldsExist<BalanceView>("BalanceView")
        assertFieldsExist<BalanceToken>("BalanceToken")
        assertFieldsExist<BalanceCacheEntry>("BalanceCacheEntry")
        // Every field: `hidden` is what masks the switcher's rows and total.
        assertFieldsExhaustive<BalanceSwitcherView>("BalanceSwitcherView")
        // Every field: `cause` and `rpc_fixable` decide whether a row may
        // offer its RPC editor (a token list that did not load is no RPC's fault).
        assertFieldsExhaustive<app.getvela.wallet.feature.wallet.core.UnreachableNetwork>("UnreachableNetwork")
    }

    @Test
    fun balanceOperationsAndResultsAreExhaustive() {
        assertVariantsExhaustive<BalanceOperation>("BalanceOperation")
        assertVariantsExhaustive<BalanceShellResult>("BalanceShellResult")
        // PR 2 note 11: a read that never left the app, told as that.
        assertVariantFieldsExhaustive(BalanceShellResult.serializer(), "BalanceShellResult")
        assertTrue("internal_chain_ids" in serializer<BalanceView>().descriptor.elementNames)
        assertTrue("internal_key" in serializer<BalanceView>().descriptor.elementNames)
        // The final core round (F19): "Checking…" and "live" are the core's to say.
        assertTrue("checking_key" in serializer<BalanceView>().descriptor.elementNames)
        assertTrue("live_key" in serializer<BalanceView>().descriptor.elementNames)
    }

    @Test
    fun balanceEventsExist() {
        assertVariantsExist<BalanceEvent>("BalanceEvent")
    }

    @Test
    fun balanceNoticesMatchTheGeneratedMirror() {
        assertStringUnion<BalanceNotice>("BalanceNotice")
    }

    @Test
    fun aBalanceIsAStringAndATotalIsNullable() {
        // Two type choices the whole read path rests on, pinned because both
        // would compile if they were wrong.
        //
        // `balance` is a STRING because the core parses it as a human decimal
        // and multiplies it by a price; a numeric field here would invite
        // handing over raw units, which is a total 10^18 times too large and
        // invisible until a price exists.
        //
        // `display_total_usd` is NULLABLE because "we do not know" is not zero,
        // and a money screen that renders unknown as 0 has told somebody their
        // wallet is empty.
        assertEquals("kotlin.String", elementDescriptor<BalanceToken>("balance").serialName)
        assertTrue(
            "display_total_usd must stay nullable",
            elementDescriptor<BalanceView>("display_total_usd").isNullable,
        )
        assertTrue(
            "price_usd must stay nullable",
            elementDescriptor<BalanceToken>("price_usd").isNullable,
        )
    }

    // -- activity_feed (spec 041) ---------------------------------------------

    @Test
    fun feedViewsMatchTheGeneratedMirrors() {
        assertFieldsExist<FeedView>("FeedView")
        // Balance privacy: the feed's own flag, and each row's own word on its figure.
        assertTrue("hidden" in serializer<FeedView>().descriptor.elementNames)
        // Issue #469: the home's newest three, the core's cut.
        assertTrue("home_rows" in serializer<FeedView>().descriptor.elementNames)
        assertTrue("figure_maskable" in serializer<FeedItem>().descriptor.elementNames)
        assertFieldsExist<FeedItem>("FeedItem")
        assertFieldsExist<FeedTxRecord>("FeedTxRecord")
        assertFieldsExist<FeedBatch>("FeedBatch")
        assertFieldsExist<FeedBatchTransfer>("FeedBatchTransfer")
        assertFieldsExist<FeedToast>("FeedToast")
    }

    @Test
    fun feedOperationsAndResultsAreExhaustive() {
        // Six operations. `read_tx_store` and `scan_incoming_transfers` are
        // issued together on a tick, which is why `read_id` exists — an
        // unanswered operation here does not hang one screen, it strands a
        // celebration.
        assertVariantsExhaustive<FeedOperation>("FeedOperation")
        assertVariantsExhaustive<FeedShellResult>("FeedShellResult")
    }

    @Test
    fun feedEventsExist() {
        assertVariantsExist<FeedEvent>("FeedEvent")
    }

    @Test
    fun feedRowsAndEnumsMatchTheGeneratedMirrors() {
        assertVariantsExhaustive<FeedRow>("FeedRow")
        assertStringUnion<FeedTxKind>("FeedTxKind")
        assertStringUnion<FeedTxStatus>("FeedTxStatus")
        assertStringUnion<FeedDirection>("FeedDirection")
        assertStringUnion<FeedBatchKind>("FeedBatchKind")
        assertStringUnion<app.getvela.wallet.feature.wallet.core.FeedCounterpartyRole>("FeedCounterpartyRole")
    }

    /**
     * Spec 093: a dApp row's payload, its second line and its detail lines.
     * The view parts may be a subset; the variants must all be known — an
     * unknown one is dropped by the fail-soft lists, which is a part of the
     * row nobody sees.
     */
    @Test
    fun dappActivityWiresMatchTheGeneratedMirrors() {
        assertFieldsExist<app.getvela.wallet.feature.wallet.core.FeedDapp>("FeedDapp")
        assertFieldsExist<app.getvela.wallet.feature.wallet.core.FeedAllowance>("FeedAllowance")
        assertFieldsExist<app.getvela.wallet.feature.wallet.core.FeedDappChange>("FeedDappChange")
        assertVariantsExhaustive<app.getvela.wallet.feature.wallet.core.FeedLine>("FeedLine")
        assertVariantsExhaustive<app.getvela.wallet.feature.wallet.core.FeedFact>("FeedFact")
        assertVariantsExhaustive<app.getvela.wallet.feature.wallet.core.FeedDappOperation>("FeedDappOperation")
        assertStringUnion<app.getvela.wallet.feature.wallet.core.FeedDappContent>("FeedDappContent")
        assertStringUnion<app.getvela.wallet.feature.wallet.core.DappAction>("DappAction")
        // A contact's rows (spec 093): the view's list, the event that fills
        // it, and the day part of their second line — each declared here and
        // checked against the mirror by the gates above.
        assertTrue("contact_rows" in serializer<FeedView>().descriptor.elementNames)
        assertTrue("contact_filter_changed" in variantNames(FeedEvent.serializer()))
        assertTrue("day" in variantNames(app.getvela.wallet.feature.wallet.core.FeedLine.serializer()))
        assertFieldsExist<FeedView>("FeedView")
        assertVariantsExist<FeedEvent>("FeedEvent")
    }

    /**
     * Spec 093: what the shell stores VERBATIM and hands back — a dApp
     * record's summary and the sheet's judgments — must carry every field
     * the core writes. A field missing here would be dropped on the disk and
     * the feed would read the record as something it was not.
     */
    @Test
    fun whatTheRecordKeepsVerbatimLosesNoField() {
        assertFieldsExhaustive<app.getvela.wallet.feature.wallet.core.DappSummary>("DappSummary")
        assertVariantFieldsExhaustive(TrustSimJudgment.serializer(), "TrustSimJudgment")
        assertFieldsExhaustive<GuardTokenMetaView>("GuardTokenMetaView")
        // Spec 097: the coins a reading named, the reading the approve
        // carries, and the tracker's settlement — each kept verbatim.
        assertFieldsExhaustive<app.getvela.wallet.feature.wallet.core.DappToken>("DappToken")
        assertFieldsExhaustive<app.getvela.wallet.feature.wallet.core.DappReading>("DappReading")
        assertFieldsExhaustive<app.getvela.wallet.feature.send.core.TrackSettlement>("TrackSettlement")
        assertFieldsExhaustive<app.getvela.wallet.feature.send.core.TrackMove>("TrackMove")
        assertStringUnion<app.getvela.wallet.feature.send.core.TrackFailure>("TrackFailure")
    }

    @Test
    fun aFeedRecordKeepsSecondsAndMillisecondsApart() {
        // `timestamp` is epoch SECONDS and `day_start_ms` is epoch
        // MILLISECONDS, in the same struct, one field apart. Both are `f64` in
        // the Rust and both are `number` in the mirror, so nothing outside this
        // test can tell them apart — and swapping them puts every payment in
        // 1970 or in the year 57000.
        //
        // `day_start_ms` is also the one value in this wire the SHELL must
        // compute, because it is local midnight on this device in this
        // timezone. The core cannot know it.
        assertEquals("kotlin.Double", elementDescriptor<FeedTxRecord>("timestamp").serialName)
        assertEquals("kotlin.Double", elementDescriptor<FeedTxRecord>("day_start_ms").serialName)
        assertEquals("number", tsFieldType("FeedTxRecord", "timestamp"))
        assertEquals("number", tsFieldType("FeedTxRecord", "day_start_ms"))
    }

    @Test
    fun aFeedItemsAmountStaysAStringAndItsCountsStayIntegers() {
        // Same rule as the balance wire: an amount is a decimal STRING, never
        // a number. And `decimals` is a `u32` in the Rust — a Double here is
        // rejected outright by serde, which is 040's bug in a new place.
        assertEquals("kotlin.String", elementDescriptor<FeedItem>("id").serialName)
        assertEquals("kotlin.Int", elementDescriptor<FeedTxRecord>("decimals").serialName)
        assertEquals("kotlin.Int", elementDescriptor<FeedTxRecord>("chain_id").serialName)
        assertTrue(
            "a batch row has no single amount, so `value` must stay nullable",
            elementDescriptor<FeedItem>("value").isNullable,
        )
    }

    // -- token_trust (spec 041) -----------------------------------------------

    @Test
    fun trustViewsMatchTheGeneratedMirrors() {
        assertFieldsExist<TrustView>("TrustView")
        assertFieldsExist<TrustIncomingView>("TrustIncomingView")
        assertFieldsExist<TrustSimView>("TrustSimView")
        assertFieldsExist<TrustRawLog>("TrustRawLog")
        assertFieldsExist<TrustCustomToken>("TrustCustomToken")
        assertFieldsExist<TrustMetaEntry>("TrustMetaEntry")
        assertFieldsExist<TrustTokenMeta>("TrustTokenMeta")
        assertFieldsExist<TrustAssetDelta>("TrustAssetDelta")
        assertFieldsExist<TrustReceiptLog>("TrustReceiptLog")
    }

    @Test
    fun trustOperationsAndResultsAreExhaustive() {
        assertVariantsExhaustive<TrustOperation>("TrustOperation")
        assertVariantsExhaustive<TrustShellResult>("TrustShellResult")
    }

    @Test
    fun trustEventsExist() {
        assertVariantsExist<TrustEvent>("TrustEvent")
    }

    @Test
    fun trustOutcomesAndJudgmentsAreExhaustive() {
        // `range_capped` is not a failure and must not be encodable as one:
        // the core uses the cap to narrow its next span, and a shell that
        // reported it as `failed` would make a busy endpoint look broken and
        // stop the scan instead of retrying smaller.
        assertVariantsExhaustive<TrustLogsOutcome>("TrustLogsOutcome")
        assertVariantsExhaustive<TrustSimJudgment>("TrustSimJudgment")
        assertStringUnion<TrustDeltaKind>("TrustDeltaKind")
    }

    @Test
    fun anIncomingTransferKeepsItsRawAmountAndItsUnresolvedMetadata() {
        // `value` is the RAW on-chain amount as a string — undivided. The feed
        // divides by `decimals` when it has them; a shell that pre-divided
        // would hand the core a number it would divide again.
        //
        // `symbol` and `decimals` stay NULLABLE because "this token's metadata
        // could not be read" is a real state, and it is exactly the state a
        // scam token is in. Defaulting them here would put a confident name on
        // the one thing that has not earned one.
        assertEquals("kotlin.String", elementDescriptor<TrustIncomingView>("value").serialName)
        assertTrue(elementDescriptor<TrustIncomingView>("symbol").isNullable)
        assertTrue(elementDescriptor<TrustIncomingView>("decimals").isNullable)
        assertTrue(elementDescriptor<TrustMetaEntry>("meta").isNullable)
        // `tokens: null` means the READ failed, which fails admission closed —
        // a non-null empty list would mean "this person has no custom tokens",
        // which is a different fact entirely.
        assertTrue(
            "CustomTokens.tokens must stay nullable",
            elementDescriptor<TrustShellResult.CustomTokens>("tokens").isNullable,
        )
    }

    // -- receive_watch + payment_request (spec 041) ---------------------------

    @Test
    fun receiveViewsMatchTheGeneratedMirrors() {
        assertFieldsExist<ReceiveWatchView>("ReceiveWatchView")
        assertFieldsExist<TokenSnapshot>("TokenSnapshot")
        assertFieldsExist<DepositEntry>("DepositEntry")
        assertFieldsExist<DepositItem>("DepositItem")
        assertFieldsExist<PaymentRequestView>("PaymentRequestView")
        assertFieldsExist<ReceiveAsset>("Asset")
        assertFieldsExist<PayRequest>("PayRequest")
    }

    @Test
    fun receiveOperationsAndResultsAreExhaustive() {
        assertVariantsExhaustive<ReceiveWatchOperation>("ReceiveWatchOperation")
        assertVariantsExhaustive<ReceiveWatchShellResult>("ReceiveWatchShellResult")
        assertVariantsExhaustive<PaymentRequestOperation>("PaymentRequestOperation")
        assertVariantsExhaustive<PaymentRequestShellResult>("PaymentRequestShellResult")
    }

    @Test
    fun receiveEventsExist() {
        assertVariantsExist<ReceiveWatchEvent>("ReceiveWatchEvent")
        assertVariantsExist<PaymentRequestEvent>("PaymentRequestEvent")
        assertStringUnion<ReceiveMode>("ReceiveMode")
    }

    @Test
    fun aPayRequestCarriesBaseUnitsAsAString() {
        // Base units routinely exceed what a double holds exactly — 1 ETH is
        // 10^18 — and this is the number a payment is made FROM. A `number`
        // here would round somebody's request.
        val base = elementDescriptor<PayRequest>("amount_base")
        assertEquals("kotlin.String?", base.serialName)
        assertTrue("an open request has no amount at all", base.isNullable)
        assertEquals("string | null", tsFieldType("PayRequest", "amount_base"))
        // A watched BALANCE is a double on purpose, and that is not a
        // contradiction: it is compared against a threshold, never spent.
        assertEquals("kotlin.Double", elementDescriptor<TokenSnapshot>("balance").serialName)
    }

    @Test
    fun theReceiveGateStartsCoveredAndUnacknowledged() {
        // `gate_loading` defaulting to false would flash a QR code at somebody
        // before the warning they have not yet seen — the one thing this gate
        // exists to prevent.
        assertTrue(PaymentRequestView().gate_loading)
        assertEquals(false, PaymentRequestView().acknowledged)
    }

    // -- rpc_pool (spec 041) --------------------------------------------------

    @Test
    fun poolViewsMatchTheGeneratedMirrors() {
        assertFieldsExist<RpcPoolView>("RpcPoolView")
        assertFieldsExist<RpcEndpointSeed>("RpcEndpointSeed")
        assertFieldsExist<RpcBanEntry>("RpcBanEntry")
        assertFieldsExist<RpcErrorInfo>("RpcErrorInfo")
    }

    @Test
    fun poolOperationsAreExhaustive() {
        // Seven operations, and the pool is the base every other read-path
        // machine goes through — an operation nobody answers here is every
        // screen in the app hanging, not one.
        assertVariantsExhaustive<RpcOperation>("RpcOperation")
    }

    @Test
    fun poolResultsAreExhaustive() {
        assertVariantsExhaustive<RpcShellResult>("RpcShellResult")
    }

    @Test
    fun poolVerdictsAndOutcomesAreExhaustive() {
        assertVariantsExhaustive<RpcCallVerdict>("RpcCallVerdict")
        assertVariantsExhaustive<RpcTransportOutcome>("RpcTransportOutcome")
    }

    @Test
    fun poolEventsExist() {
        assertVariantsExist<RpcEvent>("RpcEvent")
    }

    @Test
    fun poolEnumsMatchTheGeneratedMirrors() {
        assertStringUnion<RpcSource>("RpcSource")
        assertStringUnion<RpcKind>("RpcKind")
    }

    // -- network_admin -------------------------------------------------------

    @Test
    fun networkViewsMatchTheGeneratedMirrors() {
        // Every struct the settings surface reads out of `NetView`. Forty-odd
        // fields transcribed by hand from Rust; this is what makes that safe.
        assertFieldsExist<NetView>("NetView")
        assertFieldsExist<NetNetworkRow>("NetNetworkRow")
        assertFieldsExist<NetChainMismatch>("NetChainMismatch")
        assertFieldsExist<NetEndpointView>("NetEndpointView")
        assertFieldsExist<NetProviderView>("NetProviderView")
        assertFieldsExist<NetProviderTestView>("NetProviderTestView")
        assertFieldsExist<NetProviderNetRow>("NetProviderNetRow")
        // Every field: `error_key` is the sentence a stop is said in, and
        // `compat` rides beside an error on the scan path — a field left
        // unread here is a stop with the wrong words.
        assertFieldsExhaustive<NetWizardView>("NetWizardView")
        assertFieldsExist<NetChainIndexEntry>("NetChainIndexEntry")
        assertFieldsExist<NetChainInfo>("NetChainInfo")
        assertFieldsExist<NetCompatibility>("NetCompatibility")
        assertFieldsExist<NetContractStatus>("NetContractStatus")
        assertFieldsExist<NetRawChainData>("NetRawChainData")
    }

    @Test
    fun networkStoredShapesMatchTheGeneratedMirrors() {
        // These cross the bridge in BOTH directions — an operation carries them
        // out and a result carries them back — so a missing field is a value
        // silently dropped on the way to storage.
        assertFieldsExist<NetCustomNetwork>("NetCustomNetwork")
        assertFieldsExist<NetNetworkConfig>("NetNetworkConfig")
        assertFieldsExist<NetServiceEndpoints>("NetServiceEndpoints")
        assertFieldsExist<NetStoredEndpoints>("NetStoredEndpoints")
        assertFieldsExist<NetProviderKeys>("NetProviderKeys")
    }

    @Test
    fun networkOperationsAreExhaustive() {
        assertVariantsExhaustive<NetOperation>("NetOperation")
    }

    @Test
    fun networkResultsAreExhaustive() {
        assertVariantsExhaustive<NetShellResult>("NetShellResult")
    }

    @Test
    fun networkHealthShapesAreExhaustive() {
        assertVariantsExhaustive<NetProbeHealth>("NetProbeHealth")
        assertVariantsExhaustive<NetServiceHealth>("NetServiceHealth")
        assertVariantsExhaustive<NetHealthBody>("NetHealthBody")
        assertVariantsExhaustive<NetWizardErrorKind>("NetWizardErrorKind")
    }

    @Test
    fun networkEventsExist() {
        assertVariantsExist<NetEvent>("NetEvent")
    }

    @Test
    fun networkEnumsMatchTheGeneratedMirrors() {
        // ts-rs writes a plain string union for a fieldless Rust enum, so these
        // are compared as values rather than as union members with payloads.
        assertStringUnion<NetEndpointField>("NetEndpointField")
        assertStringUnion<NetProviderId>("NetProviderId")
        assertStringUnion<NetOverrideField>("NetOverrideField")
        assertStringUnion<NetWizardPhase>("NetWizardPhase")
        // The core's one rule for the wizard's RPC field (F4 / F14 / F22).
        assertStringUnion<app.getvela.wallet.feature.settings.core.NetRpcField>("NetRpcField")
        assertStringUnion<NetRpcFailureKind>("NetRpcFailureKind")
    }


    // -- send / fee_policy / tx_tracker / manage_tokens (spec 043) ---------------
    //
    // Money crosses these four. The exhaustive families include the closed
    // error and verdict enums: a refusal the phone cannot decode is not a
    // wrong message, it is an exception on the confirm screen.

    @Test
    fun sendViewsMatchTheGeneratedMirrors() {
        assertFieldsExist<SendView>("SendView")
        assertFieldsExist<SendToken>("SendToken")
        assertFieldsExist<SendChainInfo>("SendChainInfo")
        assertFieldsExist<SendTokenMeta>("SendTokenMeta")
        assertFieldsExist<SendTreasuryStatus>("SendTreasuryStatus")
        // Issue #422: the stop's coin and figures are the core's, field for field.
        assertFieldsExhaustive<SendTreasuryCoin>("SendTreasuryCoin")
        // Issue #466: what "Report this" files is the core's, field for field.
        assertFieldsExhaustive<app.getvela.wallet.feature.send.core.SendRelayReport>("SendRelayReport")
        assertTrue("relay_report" in serializer<SendView>().descriptor.elementNames)
        // The fee row's coin is the core's, field for field (C10).
        assertFieldsExhaustive<app.getvela.wallet.feature.send.core.SendFeeCoin>("SendFeeCoin")
        assertTrue("fee_coin" in serializer<SendView>().descriptor.elementNames)
        assertFieldsExist<SendQuotedFee>("SendQuotedFee")
        assertFieldsExist<SendTxRecord>("SendTxRecord")
        assertFieldsExist<SendRecipientIdentity>("SendRecipientIdentity")
        assertFieldsExist<SendRecipientRisk>("SendRecipientRisk")
        assertFieldsExist<SendFeeIssueView>("SendFeeIssueView")
        assertFieldsExist<SendUnitIssue>("SendUnitIssue")
        assertFieldsExist<SendRecipientDraft>("SendRecipientDraft")
        assertFieldsExist<SendMultiSpecView>("SendMultiSpecView")
        assertFieldsExist<SendReceiptView>("SendReceiptView")
        // A refusal told by its reason, and the hold's one line.
        assertTrue("refusal_key" in serializer<SendReceiptView>().descriptor.elementNames)
        assertFieldsExhaustive<app.getvela.wallet.feature.send.core.SendPreviousPending>("SendPreviousPending")
        assertTrue("previous_pending" in serializer<app.getvela.wallet.feature.send.core.SendView>().descriptor.elementNames)
        assertFieldsExist<SendReceiptTransfer>("SendReceiptTransfer")
        assertFieldsExist<SendAccountRef>("SendAccountRef")
        assertFieldsExist<SendOpenParams>("SendOpenParams")
        assertFieldsExist<SendDisplayContext>("SendDisplayContext")
        assertFieldsExist<SendSplitRowIssue>("SendSplitRowIssue")
        assertFieldsExist<SendDuplicateRowView>("SendDuplicateRowView")
        assertStringUnion<SendRowFieldState>("SendRowFieldState")
        // Spec 097 F: who is paid, and every coin a receipt sent.
        assertFieldsExist<SendPayee>("SendPayee")
        assertFieldsExist<SendReceiptCoin>("SendReceiptCoin")
        assertVariantsExhaustive<SendNameSource>("SendNameSource")
        assertTrue("payees" in serializer<SendView>().descriptor.elementNames)
        assertTrue("coins" in serializer<SendReceiptView>().descriptor.elementNames)
    }

    /**
     * Spec 097 F: the payee's name source is a closed enum in Rust, but a
     * newer core may add a source; one this build cannot read must cost the
     * payee its tag (and so, drawn, its name) — never the whole send view.
     */
    @Test
    fun aPayeesNameSourceDecodesAndAnUnknownOneIsAbsent() {
        val view = roundTrip<SendView>(
            """{"recipient":"0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c","payees":[
              {"address":"0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c","name":"Wallet","name_source":{"type":"registry"}},
              {"address":"0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141","name":"bob.eth","name_source":{"type":"service","label":"ENS"}},
              {"address":"0x88cCA0EeDbF2C4426110bbFc998F048689266894","name":"Savings","name_source":{"type":"own"}},
              {"address":"0x88cCA0EeDbF2C4426110bbFc998F048689266894","name":null,"name_source":null},
              {"address":"0x88cCA0EeDbF2C4426110bbFc998F048689266894","name":"Later","name_source":{"type":"some_new_source","x":1}}
            ],"receipt":{"status":"confirmed","hold_reason":null,"kind":"multi_select","transfers":[],"coins":[
              {"amount":"0.000418","symbol":"ETH","logo_urls":[],"token_address":null,"usd_value":1.0},
              {"amount":"0.034929","symbol":"USDC","logo_urls":["https://x/usdc.png"],"token_address":"0x833589fcd6edb6e08f4c7c32d4f71b54bda02913","usd_value":0.03}
            ],"amount":"","usd_value":1.03,"submitted_at_ms":null,"typical_inclusion_s":null}}""",
        )
        assertEquals(
            listOf(SendNameSource.Registry, SendNameSource.Service("ENS"), SendNameSource.Own, null, null),
            view.payees.map { it.name_source },
        )
        assertEquals("Later", view.payees[4].name)
        assertEquals(SendPayee("0x88cCA0EeDbF2C4426110bbFc998F048689266894"), view.payees[3])
        assertEquals(listOf("ETH", "USDC"), view.receipt!!.coins.map { it.symbol })
        assertEquals(null, view.receipt!!.coins[0].token_address)
        assertEquals("", view.receipt!!.amount)
    }

    @Test
    fun sendOperationsResultsAndVerdictsAreExhaustive() {
        assertVariantsExhaustive<SendOperation>("SendOperation")
        assertVariantsExhaustive<SendShellResult>("SendShellResult")
        assertVariantsExhaustive<SendSubmitFailure>("SendSubmitFailure")
        assertVariantsExhaustive<SendAlertKind>("SendAlertKind")
        assertVariantsExhaustive<SendAmountWarning>("SendAmountWarning")
        assertVariantsExhaustive<SendTreasuryProbe>("SendTreasuryProbe")
        assertVariantsExhaustive<SendFeeOutcome>("SendFeeOutcome")
        assertVariantsExhaustive<SendAddNetworkOutcome>("SendAddNetworkOutcome")
        assertVariantsExhaustive<SendAddNetworkMsg>("SendAddNetworkMsg")
        assertVariantsExhaustive<SendLockError>("SendLockError")
        assertVariantsExhaustive<SendScan>("SendScan")
        assertVariantsExhaustive<SendReceiptOutcome>("SendReceiptOutcome")
    }

    @Test
    fun sendEventsExist() {
        assertVariantsExist<SendEvent>("SendEvent")
    }

    @Test
    fun sendStringUnionsMatch() {
        assertSendEstimateFailureMatchesTheMirror()
        assertStringUnion<SendTreasuryAsset>("SendTreasuryAsset")
        assertStringUnion<SendTxStatus>("SendTxStatus")
        assertStringUnion<SendTxErrorKey>("SendTxErrorKey")
        assertStringUnion<SendTimerTag>("SendTimerTag")
        assertStringUnion<SendHapticKind>("SendHapticKind")
        assertStringUnion<SendStage>("SendStage")
        assertStringUnion<SendReceiptStatus>("SendReceiptStatus")
        assertStringUnion<SendHoldReason>("SendHoldReason")
        assertStringUnion<SendReceiptKind>("SendReceiptKind")
    }

    @Test
    fun feeViewsMatchTheGeneratedMirrors() {
        assertFieldsExist<FeeView>("FeeView")
        assertFieldsExist<FeeOptionView>("FeeOptionView")
        assertFieldsExhaustive<app.getvela.wallet.feature.send.core.FeeShortfall>("FeeShortfall")
        assertFieldsExist<FeeEstimateView>("FeeEstimateView")
        assertFieldsExist<FeeBundlerQuote>("FeeBundlerQuote")
        assertFieldsExist<FeeAssetQuote>("FeeAssetQuote")
        assertFieldsExist<FeeCall>("FeeCall")
        // Issue #411: the sheet's simulation, told to the fee machine.
        assertFieldsExist<app.getvela.wallet.feature.send.core.FeeBalanceChange>("FeeBalanceChange")
        // The correctness batch: `provisional` (a switched coin re-measured)
        // and the relay's `minimum_amount` — every field, so the gate the
        // core reads back (`signConfirmState`) is never handed less.
        assertFieldsExhaustive<FeeView>("FeeView")
        assertTrue("minimum_amount" in serializer<FeeAssetQuote>().descriptor.elementNames)
        // PR 2 note 1: the failure said once for the row and the footer — every field.
        assertFieldsExhaustive<app.getvela.wallet.feature.send.core.FeeFailureView>("FeeFailureView")
        // PR 2 polish: what a tap on the failed row does — every word.
        assertStringUnion<app.getvela.wallet.feature.send.core.FeeFailureTap>("FeeFailureTap")
    }

    @Test
    fun feeOperationsResultsAndOutcomesAreExhaustive() {
        assertVariantsExhaustive<FeeOperation>("FeeOperation")
        assertVariantsExhaustive<FeeShellResult>("FeeShellResult")
        assertVariantsExhaustive<FeeGasOutcome>("FeeGasOutcome")
        assertVariantsExhaustive<FeeAssetView>("FeeAssetView")
        // Issue #483: the account read is the fee's own — every answer it can take.
        assertVariantsExhaustive<app.getvela.wallet.feature.send.core.DeploymentRead>("DeploymentRead")
        assertVariantsExist<FeeEvent>("FeeEvent")
        assertStringUnion<FeeTier>("FeeTier")
        assertFeeFailureMatchesTheMirror()
        assertStringUnion<FeeAssetKind>("FeeAssetKind")
    }

    /** Spec 102: Settings → Signing pages (it replaced spec 071's one-page preference). */
    @Test
    fun signingPagesMatchTheGeneratedMirrors() {
        assertFieldsExist<app.getvela.wallet.feature.settings.core.SigningPagesView>("SigningPagesView")
        assertFieldsExist<app.getvela.wallet.feature.settings.core.SigningPageRow>("SigningPageRow")
        assertFieldsExist<app.getvela.wallet.feature.settings.core.SigningPage>("SigningPage")
        assertVariantsExhaustive<app.getvela.wallet.feature.settings.core.SigningPagesOperation>("SigningPagesOperation")
        assertVariantsExhaustive<app.getvela.wallet.feature.settings.core.SigningPagesShellResult>("SigningPagesShellResult")
        assertVariantsExist<app.getvela.wallet.feature.settings.core.SigningPagesEvent>("SigningPagesEvent")
    }

    /** Spec 069: the default speed's machine and the speed control's. */
    @Test
    fun speedViewsEventsAndPreferenceMatchTheGeneratedMirrors() {
        assertFieldsExist<app.getvela.wallet.feature.settings.core.FeeTierPrefView>("FeeTierPrefView")
        assertVariantsExhaustive<app.getvela.wallet.feature.settings.core.FeeTierPrefOperation>("FeeTierPrefOperation")
        assertVariantsExhaustive<app.getvela.wallet.feature.settings.core.FeeTierPrefShellResult>("FeeTierPrefShellResult")
        assertVariantsExist<app.getvela.wallet.feature.settings.core.FeeTierPrefEvent>("FeeTierPrefEvent")
        assertFieldsExist<app.getvela.wallet.feature.send.core.FeeSpeedView>("FeeSpeedView")
        assertFieldsExist<app.getvela.wallet.feature.send.core.FeeSpeedOptionView>("FeeSpeedOptionView")
        assertFieldsExist<app.getvela.wallet.feature.send.core.TierQuote>("TierQuote")
        assertFieldsExist<app.getvela.wallet.feature.send.core.TierPreviewQuote>("TierPreviewQuote")
        assertVariantsExist<app.getvela.wallet.feature.send.core.FeeSpeedEvent>("FeeSpeedEvent")
    }

    @Test
    fun trackerViewsOperationsAndResultsMatch() {
        assertFieldsExist<TrackView>("TrackView")
        // Every field: an entry is re-encoded into `sendReceiptOutcomeOf`, and
        // a dropped `refusal` would tell every refusal as the plain one.
        assertFieldsExhaustive<TrackEntryView>("TrackEntryView")
        assertFieldsExhaustive<TrackPendingRecord>("TrackPendingRecord")
        // One in flight per account and network, and a refusal told by its reason.
        assertFieldsExhaustive<app.getvela.wallet.feature.send.core.InFlightOp>("InFlightOp")
        assertStringUnion<app.getvela.wallet.feature.send.core.RefusalReason>("RefusalReason")
        assertFieldsExist<TrackRecordPatch>("TrackRecordPatch")
        assertVariantsExhaustive<TrackOperation>("TrackOperation")
        assertVariantsExhaustive<TrackShellResult>("TrackShellResult")
        assertVariantsExist<TrackEvent>("TrackEvent")
        assertStringUnion<TrackLifecycle>("TrackLifecycle")
        assertStringUnion<TrackStatus>("TrackStatus")
        assertStringUnion<TrackOutcome>("TrackOutcome")
        assertStringUnion<TrackRecordStatus>("TrackRecordStatus")
        // Spec 099 R6: the landing's one countdown.
        assertStringUnion<app.getvela.wallet.feature.send.core.LandingLine>("LandingLine")
        assertFieldsExist<app.getvela.wallet.feature.send.core.LandingPace>("LandingPace")
    }

    @Test
    fun manageTokensViewsOperationsAndResultsMatch() {
        assertFieldsExist<MtokView>("MtokView")
        assertFieldsExist<MtokCustomToken>("MtokCustomToken")
        assertFieldsExist<MtokFound>("MtokFound")
        assertFieldsExist<MtokNetwork>("MtokNetwork")
        assertFieldsExist<MtokTokenMeta>("MtokTokenMeta")
        assertVariantsExhaustive<MtokOperation>("MtokOperation")
        assertVariantsExhaustive<MtokShellResult>("MtokShellResult")
        assertVariantsExist<MtokEvent>("MtokEvent")
    }

    // -- spec 082: every new variant decodes (T124) ------------------------------
    //
    // kotlinx refuses a whole view over ONE value it does not know: a tracker
    // entry in `not_sent` would have blanked every entry, a plain send the
    // whole signing sheet. These are the core's own shapes, decoded and
    // encoded back — the variant must survive the trip, not merely parse.

    private inline fun <reified T> roundTrip(json: String): T {
        val decoded = Wire.json.decodeFromString(serializer<T>(), json)
        val again = Wire.json.decodeFromString(serializer<T>(), Wire.json.encodeToString(serializer<T>(), decoded))
        assertEquals("round trip changed the value", decoded, again)
        return decoded
    }

    @Test
    fun theTrackerViewDecodesTheNewEndsAndOutcomes() {
        val view = roundTrip<TrackView>(
            """{"entries":[
              {"user_op_hash":"0xaa","chain_id":100,"record_ids":["0xaa"],"status":"not_sent","tx_hash":null,"polling":false,"submitted_at_ms":1.0,"outcome":"final","relay_tx_hash":null},
              {"user_op_hash":"0xbb","chain_id":100,"record_ids":["0xbb"],"status":"pending","tx_hash":null,"polling":true,"submitted_at_ms":2.0,"outcome":"maybe_sent","relay_tx_hash":"0xcc"}
            ]}""",
        )
        assertEquals(TrackStatus.NotSent, view.entries[0].status)
        assertEquals(TrackOutcome.MaybeSent, view.entries[1].outcome)
        assertEquals("0xcc", view.entries[1].relay_tx_hash)
    }

    /**
     * 098 follow-up: the relay topping up its gas is a status, a receipt
     * outcome, a hold reason and an ending flag. A Kotlin enum without the
     * name fails the WHOLE view's decode — the tracker would go silent for
     * every op the moment one waited on the relay's gas.
     */
    @Test
    fun theRelayToppingUpItsGasDecodesEverywhereItIsSaid() {
        val view = roundTrip<TrackView>(
            """{"entries":[
              {"user_op_hash":"0xaa","chain_id":42161,"record_ids":["0xaa"],"status":"relay_funding","tx_hash":null,"polling":true,"submitted_at_ms":1.0,"outcome":"landing","relay_tx_hash":null}
            ]}""",
        )
        assertEquals(TrackStatus.RelayFunding, view.entries[0].status)
        assertEquals(
            app.getvela.wallet.feature.send.core.SendReceiptOutcome.RelayFunding,
            roundTrip<app.getvela.wallet.feature.send.core.SendReceiptOutcome>("""{"type":"relay_funding"}"""),
        )
        assertEquals(
            app.getvela.wallet.feature.send.core.SendHoldReason.RelayFunding,
            roundTrip<app.getvela.wallet.feature.send.core.SendHoldReason>("\"relay_funding\""),
        )
        assertEquals(
            SignEndingState.Following("0xop", TrackOutcome.Landing, fee_held = false, relay_funding = true),
            roundTrip<SignEndingState>("""{"type":"following","user_op_hash":"0xop","outcome":"landing","fee_held":false,"relay_funding":true}"""),
        )
    }

    @Test
    fun theTrackerOperationsAndAnswersOf082Decode() {
        val find = roundTrip<TrackOperation>(
            """{"type":"find_op_event","chain_id":100,"entry_point":"0x0000000071727De22E5E9d8BAf0edAc6f37da032","topic0":"0x49628fd1","user_op_hash":"0xaa","from_block":48478700,"to_block":null}""",
        )
        assertEquals(48_478_700L, (find as TrackOperation.FindOpEvent).from_block)
        assertEquals(TrackOperation.HoldingsMoved(100), roundTrip<TrackOperation>("""{"type":"holdings_moved","chain_id":100}"""))
        val event = roundTrip<TrackShellResult>(
            """{"type":"op_event","user_op_hash":"0xaa","now_ms":5.0,"logs_json":"[]","error_json":null,"head_block":48478800}""",
        )
        assertEquals(48_478_800L, (event as TrackShellResult.OpEvent).head_block)
        val status = roundTrip<TrackShellResult>("""{"type":"status","user_op_hash":"0xaa","status":"queued","stage":null,"now_ms":1.0,"tx_hash":"0xdd"}""")
        assertEquals("0xdd", (status as TrackShellResult.Status).tx_hash)
        val record = roundTrip<TrackPendingRecord>("""{"record_id":"0xaa","user_op_hash":"0xaa","chain_id":100,"submitted_at_ms":1.0,"maybe_sent":true,"submit_block":42}""")
        assertTrue(record.maybe_sent)
        assertEquals(42L, record.submit_block)
        // A row stored before 082 reads back as "sent" with no start block.
        val old = roundTrip<TrackPendingRecord>("""{"record_id":"0xaa","user_op_hash":"0xaa","chain_id":100,"submitted_at_ms":1.0}""")
        assertEquals(false, old.maybe_sent)
        assertEquals(null, old.submit_block)
    }

    @Test
    fun thePoolsNewOutcomeAndViewFieldDecode() {
        assertEquals(RpcTransportOutcome.NotConnected, roundTrip<RpcTransportOutcome>("""{"type":"not_connected"}"""))
        val failed = roundTrip<RpcCallVerdict>("""{"type":"failed","rate_limited":false,"maybe_delivered":true}""")
        assertTrue((failed as RpcCallVerdict.Failed).maybe_delivered)
        val view = roundTrip<RpcPoolView>("""{"failed_chains":[],"rate_limited_chains":[],"banned":[],"unreached_chains":[100]}""")
        assertEquals(listOf(100), view.unreached_chains)
    }

    @Test
    fun aPlainSendSheetDecodes() {
        val view = roundTrip<ClearSigningView>(
            """{"resolving":false,"resolved":true,"result":null,"message":null,"surface":"plain_send","confirm":{"type":"confirm_intent","intent":"send","intent_term":"intentSend"},"blind_typed":null,"danger_haptic":false,
               "plain_send":{"to":"0x1111111111111111111111111111111111111111","value_wei":"1000000000000000","amount":"0.001","no_value":false}}""",
        )
        assertEquals(ClearSurface.PlainSend, view.surface)
        assertEquals("0.001", view.plain_send?.amount)
    }

    @Test
    fun theSignSheetsPhaseAndEndingsDecode() {
        val view = roundTrip<SignView>("""{"surface":"sheet","phase":"awaiting_signature","pending_op_maybe_sent":true}""")
        assertEquals(SignPhase.AwaitingSignature, view.phase)
        assertTrue(view.pending_op_maybe_sent)
        assertEquals(SignSubmitOutcome.AskerGone, roundTrip<SignSubmitOutcome>("""{"type":"asker_gone"}"""))
        assertEquals(
            SignEnding.Landed("0xtx", "0xop"),
            roundTrip<SignEnding>("""{"type":"landed","tx_hash":"0xtx","user_op_hash":"0xop"}"""),
        )
        assertEquals(
            SignEndingState.Following("0xop", TrackOutcome.MaybeSent, false),
            roundTrip<SignEndingState>("""{"type":"following","user_op_hash":"0xop","outcome":"maybe_sent","fee_held":false}"""),
        )
        val submitted = roundTrip<SignEvent>("""{"type":"op_submitted","id":"1","user_op_hash":"0xop","now_ms":1.0,"maybe_sent":true,"submit_block":7}""")
        assertEquals(7L, (submitted as SignEvent.OpSubmitted).submit_block)
        assertEquals(SignEvent.CeremonyStarted("1"), roundTrip<SignEvent>("""{"type":"ceremony_started","id":"1"}"""))
    }

    @Test
    fun theSendReceiptsNewWordsDecode() {
        val receipt = roundTrip<SendReceiptView>("""{"status":"maybe_sent","amount":"1","usd_value":0.0}""")
        assertEquals(SendReceiptStatus.MaybeSent, receipt.status)
        assertEquals(SendReceiptOutcome.Acknowledged, roundTrip<SendReceiptOutcome>("""{"type":"acknowledged"}"""))
        assertEquals(
            SendReceiptOutcome.Failed(rejected = false, not_sent = true),
            roundTrip<SendReceiptOutcome>("""{"type":"failed","rejected":false,"not_sent":true}"""),
        )
    }

    @Test
    fun aFeedRowSaysWhatItIs() {
        val view = roundTrip<FeedView>(
            """{"rows":[{"type":"item","item":{"id":"0xaa","direction":"out","counterparty":null,"alias":null,"value":null,"symbol":"","decimals":null,"usd_value":0.0,"chain_id":100,"timestamp":1.0,"day_start_ms":0.0,"tx_hash":null,"batch":null,"kind":"dapp_tx","status":"failed","site":"127.0.0.1:8137"}}],
               "transactions":[],"new_item_id":null,"toast":null,"history_empty_key":"history.emptyFilter","home_empty_key":"home.emptyNoActivityNetwork"}""",
        )
        val item = (view.rows.single() as FeedRow.Item).item
        assertEquals(FeedTxKind.DappTx, item.kind)
        assertEquals(FeedTxStatus.Failed, item.status)
        assertEquals("127.0.0.1:8137", item.site)
        assertEquals("history.emptyFilter", view.history_empty_key)
    }

    // -- spec 082 round 2: every new variant decodes (T244) --------------------
    //
    // The same rule as above: one unknown value fails the whole view, so each
    // new variant and field is decoded from the core's own shape and survives
    // the trip back.

    @Test
    fun theWriteAheadAndTheTrackersAnswerOfRound2Decode() {
        assertEquals(
            SignOperation.ClearToPost("1", "0xop"),
            roundTrip<SignOperation>("""{"type":"clear_to_post","id":"1","user_op_hash":"0xop"}"""),
        )
        assertEquals(SignOperation.DeleteRecord("0xop"), roundTrip<SignOperation>("""{"type":"delete_record","record_id":"0xop"}"""))
        assertEquals(
            SignOperation.UpdateRecord("0xop", SignRecordClose.Admitted),
            roundTrip<SignOperation>("""{"type":"update_record","record_id":"0xop","close":{"type":"admitted"}}"""),
        )
        assertEquals(
            SignSubmitOutcome.Failed("AA23 reverted", refused = true),
            roundTrip<SignSubmitOutcome>("""{"type":"failed","message":"AA23 reverted","refused":true}"""),
        )
        // A pre-082 shape still reads: not refused.
        assertEquals(SignSubmitOutcome.Failed("x"), roundTrip<SignSubmitOutcome>("""{"type":"failed","message":"x"}"""))
        val signed = roundTrip<SignEvent>("""{"type":"op_signed","id":"1","user_op_hash":"0xop","submit_block":42,"now_ms":1.0}""")
        assertEquals(42L, (signed as SignEvent.OpSigned).submit_block)
        val tracked = roundTrip<SignEvent>("""{"type":"op_tracked","user_op_hash":"0xop","status":"rejected","tx_hash":null,"now_ms":1.0}""")
        assertEquals(TrackStatus.Rejected, (tracked as SignEvent.OpTracked).status)
        assertNull("absent: the plain refusal", tracked.refusal)
        // PR 2 note 9: the tracker entry's reason rides the event, in the core's spelling.
        val why = roundTrip<SignEvent>("""{"type":"op_tracked","user_op_hash":"0xop","status":"rejected","now_ms":1.0,"refusal":"nonce_used"}""")
        assertEquals(app.getvela.wallet.feature.send.core.RefusalReason.NonceUsed, (why as SignEvent.OpTracked).refusal)
        assertTrue(
            "never sent as an explicit null",
            !Wire.json.encodeToString(SignEvent.serializer(), tracked).contains("refusal"),
        )
        assertEquals(
            "componentsUi.signing.wentFirst",
            roundTrip<SignView>("""{"surface":"sheet","failure_refused":true,"failure_refusal_key":"componentsUi.signing.wentFirst"}""").failure_refusal_key,
        )
        assertEquals(SignEndingState.Refused, roundTrip<SignEndingState>("""{"type":"refused"}"""))
        val view = roundTrip<SignView>(
            """{"surface":"sheet","error":{"kind":"submit_failed","detail":"x"},"failure_refused":true,
               "tracker_handoff":{"user_op_hash":"0xop","record_ids":["0xop"],"chain_id":100,"maybe_sent":false,"submit_block":null,"admitted":true},
               "tracker_withdraw":{"user_op_hash":"0xop","record_ids":["0xop"]}}""",
        )
        assertTrue(view.failure_refused)
        assertTrue("absent reads false", !view.failure_retryable)
        // Spec 096 F8: the core's flag and its event, in the core's spelling.
        assertTrue(roundTrip<SignView>("""{"surface":"sheet","failure_retryable":true}""").failure_retryable)
        assertEquals(SignEvent.RetryTapped, roundTrip<SignEvent>("""{"type":"retry_tapped"}"""))
        assertTrue(view.tracker_handoff!!.admitted)
        assertEquals(SignTrackerWithdraw("0xop", listOf("0xop")), view.tracker_withdraw)

        assertEquals(
            TrackOperation.TxReceipt(100, "0xtx", "0xop"),
            roundTrip<TrackOperation>("""{"type":"tx_receipt","chain_id":100,"tx_hash":"0xtx","user_op_hash":"0xop"}"""),
        )
        assertEquals(
            TrackShellResult.TxReceipt("0xop", 1.0, "null"),
            roundTrip<TrackShellResult>("""{"type":"tx_receipt","user_op_hash":"0xop","now_ms":1.0,"receipt_json":"null"}"""),
        )
        val submitted = roundTrip<TrackEvent>("""{"type":"submitted","user_op_hash":"0xop","record_ids":["0xop"],"chain_id":100,"admitted":true}""")
        assertTrue((submitted as TrackEvent.Submitted).admitted)
        assertEquals(
            TrackEvent.Withdrawn("0xop", listOf("0xop")),
            roundTrip<TrackEvent>("""{"type":"withdrawn","user_op_hash":"0xop","record_ids":["0xop"]}"""),
        )
    }

    @Test
    fun theSendWriteAheadOfRound2Decodes() {
        assertEquals(SendOperation.ClearToPost("0xop"), roundTrip<SendOperation>("""{"type":"clear_to_post","user_op_hash":"0xop"}"""))
        assertEquals(
            SendOperation.MarkAdmitted(listOf("0xop-0", "0xop-1")),
            roundTrip<SendOperation>("""{"type":"mark_admitted","record_ids":["0xop-0","0xop-1"]}"""),
        )
        assertEquals(SendOperation.DeleteTxRecords(listOf("0xop")), roundTrip<SendOperation>("""{"type":"delete_tx_records","ids":["0xop"]}"""))
        assertEquals(
            SendOperation.TrackWithdrawn("0xop", listOf("0xop")),
            roundTrip<SendOperation>("""{"type":"track_withdrawn","user_op_hash":"0xop","record_ids":["0xop"]}"""),
        )
        val track = roundTrip<SendOperation>(
            """{"type":"track_submitted","user_op_hash":"0xop","record_ids":["0xop"],"chain_id":100,"maybe_sent":false,"submit_block":null,"admitted":true}""",
        )
        assertTrue((track as SendOperation.TrackSubmitted).admitted)
        assertEquals(SendShellResult.PostCleared, roundTrip<SendShellResult>("""{"type":"post_cleared"}"""))
        val signed = roundTrip<SendEvent>("""{"type":"op_signed","user_op_hash":"0xop","submit_block":7,"now_ms":1.0}""")
        assertEquals(7L, (signed as SendEvent.OpSigned).submit_block)
    }

    /**
     * Issue #408: a coin's shortfall and the sheet's "no coin pays" decode as
     * the core writes them, and a view from before them still decodes — the
     * fields are additive, so neither can fail a whole fee view.
     */
    @Test
    fun aFeeCoinsShortfallDecodesAndItsAbsenceIsTolerated() {
        val option = """{"symbol":"USDT","contract":"0xdac1","decimals":6,"balance":"754189","recipient":"0x3e59","usd_balance":"0.75","usd_price":"1","amount":"3583610","insufficient":true,"selected":false,"spent_by_operation":false"""
        val view = roundTrip<FeeView>(
            """{"busy":false,"failed":null,"fee":null,"stale":false,"fee_token":null,"options":[$option,"short":{"need":"3.58361 USDT","have":"0.754189 USDT"}}],"confirm_fee_ready":false,"no_coin_pays":true}""",
        )
        assertTrue(view.no_coin_pays)
        assertEquals(
            app.getvela.wallet.feature.send.core.FeeShortfall(need = "3.58361 USDT", have = "0.754189 USDT"),
            view.options.single().short,
        )
        val before = roundTrip<FeeView>("""{"busy":false,"options":[$option}],"confirm_fee_ready":false}""")
        assertFalse(before.no_coin_pays)
        assertNull(before.options.single().short)
    }

    @Test
    fun aChainReadFeeFailureAndAFeedRowsRoleDecode() {
        val view = roundTrip<FeeView>("""{"busy":false,"failed":{"chain_read":{"rate_limited":true}}}""")
        assertEquals(FeeFailure.ChainRead(rate_limited = true), view.failed)
        assertEquals(FeeFailure.QuoteUnavailable, roundTrip<FeeView>("""{"failed":"quote_unavailable"}""").failed)
        assertEquals("""{"chain_read":{"rate_limited":false}}""", FeeFailure.ChainRead(false).wire)
        assertEquals("quote_unavailable", FeeFailure.QuoteUnavailable.wire)
        val refused = runCatching { Wire.json.decodeFromString(FeeView.serializer(), """{"failed":"no_such_word"}""") }
        assertTrue("an unknown fee failure fails the view, never reads as a default", refused.isFailure)

        val record = roundTrip<FeedTxRecord>(
            """{"id":"0xop","from":"0xa","to":"0xb","value":"0","symbol":"","decimals":18,"chain_id":100,"timestamp":1.0,"day_start_ms":0.0,"status":"pending","call_data":"0xa9059cbb"}""",
        )
        assertEquals("0xa9059cbb", record.call_data)
        val item = roundTrip<FeedItem>(
            """{"id":"0xaa","direction":"out","counterparty":"0xc","usd_value":0.0,"chain_id":100,"timestamp":1.0,"day_start_ms":0.0,"counterparty_role":"contract"}""",
        )
        assertEquals(app.getvela.wallet.feature.wallet.core.FeedCounterpartyRole.Contract, item.counterparty_role)
    }

    /**
     * `SendEstimateFailure` since PR 2 note 13: every `FeeFailure`, in its own
     * wire shape, plus the send side's `timeout` and `other` — the plain words
     * are FeeFailure's and those two, and its one object arm is chain_read.
     */
    private fun assertSendEstimateFailureMatchesTheMirror() {
        val arms = splitTopLevel(
            mirror("SendEstimateFailure")
                .replace(Regex("/\\*\\*.*?\\*/", RegexOption.DOT_MATCHES_ALL), " ")
                .substringAfter("export type SendEstimateFailure =")
                .substringBeforeLast(";"),
            '|',
        ).map { it.trim() }
        val plain = arms.filter { it.startsWith("\"") }.map { it.trim('"') }
        assertEquals(
            "SendEstimateFailure's plain words must be FeeFailure's and the send side's own",
            plain.sorted(),
            (FeeFailure.PLAIN.map { it.name } + SendEstimateFailure.SEND_ONLY).sorted(),
        )
        val objects = arms.filter { it.startsWith("{") }
        assertEquals("SendEstimateFailure has exactly one object arm, chain_read: $objects", 1, objects.size)
        assertTrue(objects.single(), objects.single().contains("\"chain_read\"") && objects.single().contains("rate_limited: boolean"))
        // Each passes through in the fee machine's own shape, both ways.
        assertEquals(
            SendEstimateFailure.Fee(FeeFailure.ChainRead(rate_limited = false)),
            Wire.json.decodeFromString(SendEstimateFailure.serializer(), """{"chain_read":{"rate_limited":false}}"""),
        )
        for (failure in listOf(SendEstimateFailure.Fee(FeeFailure.Internal), SendEstimateFailure.Fee(FeeFailure.WouldFail), SendEstimateFailure.Timeout, SendEstimateFailure.Other, SendEstimateFailure.Fee(FeeFailure.ChainRead(true)))) {
            val json = Wire.json.encodeToString(SendEstimateFailure.serializer(), failure)
            assertEquals(failure, Wire.json.decodeFromString(SendEstimateFailure.serializer(), json))
            assertEquals(json.trim('"'), failure.wire.trim('"'))
        }
        assertTrue(
            "an unknown word fails, never reads as a default",
            runCatching { Wire.json.decodeFromString(SendEstimateFailure.serializer(), "\"no_such_word\"") }.isFailure,
        )
    }

    /**
     * `FeeFailure` since spec 082 (RJ13): the plain words are a string union,
     * and `chain_read` is the one arm that carries a field. Kotlin's plain words
     * must be the mirror's, and its object arm must be the mirror's object arm.
     */
    private fun assertFeeFailureMatchesTheMirror() {
        val arms = splitTopLevel(
            mirror("FeeFailure")
                .replace(Regex("/\\*\\*.*?\\*/", RegexOption.DOT_MATCHES_ALL), " ")
                .substringAfter("export type FeeFailure =")
                .substringBeforeLast(";"),
            '|',
        ).map { it.trim() }
        val plain = arms.filter { it.startsWith("\"") }.map { it.trim('"') }
        assertEquals("FeeFailure's plain words must match the generated mirror", plain.sorted(), FeeFailure.PLAIN.map { it.name }.sorted())
        val objects = arms.filter { it.startsWith("{") }
        assertEquals("FeeFailure has exactly one object arm, chain_read: $objects", 1, objects.size)
        assertTrue(objects.single(), objects.single().contains("\"chain_read\"") && objects.single().contains("rate_limited: boolean"))
    }

    // -- assertions ----------------------------------------------------------


    // -- spec 044: the in-app browser and what it signs --------------------

    @Test
    fun dappBrowserWiresMatchTheMirrors() {
        assertFieldsExist<DbrView>("DbrView")
        assertFieldsExist<DbrConsentView>("DbrConsentView")
        assertFieldsExist<DbrTabView>("DbrTabView")
        assertFieldsExist<DbrSiteView>("DbrSiteView")
        assertFieldsExist<DbrSigningView>("DbrSigningView")
        assertFieldsExist<DbrStoredSite>("DbrStoredSite")
        assertFieldsExist<DpermGrant>("DpermGrant")
        assertVariantsExhaustive<DbrOperation>("DbrOperation")
        assertVariantsExhaustive<DbrShellResult>("DbrShellResult")
        assertVariantsExist<DbrEvent>("DbrEvent")
        assertVariantFields(DbrOperation.serializer(), "DbrOperation")
        assertVariantFields(DbrEvent.serializer(), "DbrEvent")
        assertVariantFields(DbrShellResult.serializer(), "DbrShellResult")
        // Spec 099: the request record and its vocabulary. Every value the
        // core can write must be named here — one it is not fails the whole
        // browser view, every tab with it.
        assertFieldsExist<DbrFailureNote>("DbrFailureNote")
        assertFieldsExist<DbrRequestRow>("DbrRequestRow")
        assertFieldsExist<DbrInspectorView>("DbrInspectorView")
        assertStringUnion<DbrPageState>("DbrPageState")
        assertStringUnion<DbrProviderState>("DbrProviderState")
        assertStringUnion<DbrLayer>("DbrLayer")
        assertStringUnion<DbrReason>("DbrReason")
        assertStringUnion<DbrOutcome>("DbrOutcome")
        assertStringUnion<DbrRequestClass>("DbrRequestClass")
        assertStringUnion<DbrReadFailure>("DbrReadFailure")
    }

    /**
     * Spec 099: the browser machine's new shapes as the core writes them — a
     * tab's status, the inspected record, a read that timed out, the events
     * that carry the clock and `consent_rejected`, now an object.
     */
    @Test
    fun theBrowserRecordOf099Decodes() {
        val tab = roundTrip<DbrTabView>(
            """{"tab":"t1","origin":"https://app.example","connected_address":null,"chain_id":1,"secure":true,"crashed":false,
               "busy":true,"page":"ready","provider":"no_hello","open_requests":1,"failed_recent":2,
               "last_failure":{"layer":"network","reason":"timed_out","method":"eth_call","key":"componentsUi.browserStatus.reason.timedOut"}}""",
        )
        assertTrue(tab.busy)
        assertEquals(DbrPageState.Ready, tab.page)
        assertEquals(DbrProviderState.NoHello, tab.provider)
        assertEquals(DbrFailureNote(DbrLayer.Network, DbrReason.TimedOut, "eth_call", DbrReason.TimedOut.key), tab.last_failure)
        // A tab view from before 099 still reads, with nothing to say.
        val old = roundTrip<DbrTabView>("""{"tab":"t1","origin":null,"connected_address":null,"chain_id":1,"secure":false,"crashed":false}""")
        assertEquals(DbrPageState.Blank, old.page)
        assertNull(old.last_failure)

        val view = roundTrip<DbrView>(
            """{"ready":true,"consent":null,"tabs":[],"sites":[],"signing":null,"queued_signing":0,
               "inspector":{"tab":"t1","origin":"https://app.example","page":"loading","provider":"pending","connected":true,"chain_id":8453,
                 "rows":[{"id":"7","method":"eth_call","class":"read","started_ms":1.0,"ended_ms":null,"outcome":"open","code":null,"layer":null,"reason":null},
                         {"id":"8","method":"personal_sign","class":"signing","started_ms":2.0,"ended_ms":9.0,"outcome":"failed","code":-32603,"layer":"signer","reason":"signer_not_discoverable"}],
                 "report":"Vela dApp browser — tab t1"}}""",
        )
        val inspector = view.inspector!!
        assertEquals(DbrRequestClass.Read, inspector.rows[0].kind)
        assertEquals(DbrOutcome.Failed, inspector.rows[1].outcome)
        assertEquals(-32603L, inspector.rows[1].code)
        assertEquals(DbrReason.SignerNotDiscoverable, inspector.rows[1].reason)

        assertEquals(
            DbrShellResult.ReadAnswered(null, now_ms = 5.0, failure = DbrReadFailure.TimedOut),
            roundTrip<DbrShellResult>("""{"type":"read_answered","body_json":null,"now_ms":5.0,"failure":"timed_out"}"""),
        )
        val read = roundTrip<DbrOperation>(
            """{"type":"read","tab":"t1","id":"1","chain_id":1,"method":"eth_call","params_json":"[]","bundler":false,"deadline_ms":30000.0}""",
        )
        assertEquals(30_000.0, (read as DbrOperation.Read).deadline_ms, 0.0)
        assertEquals(DbrEvent.ConsentRejected(3.0), roundTrip<DbrEvent>("""{"type":"consent_rejected","now_ms":3.0}"""))
        assertEquals(DbrEvent.InspectorClosed, roundTrip<DbrEvent>("""{"type":"inspector_closed"}"""))
        assertEquals(
            """{"type":"inspector_opened","tab":"t1"}""",
            Wire.json.encodeToString(DbrEvent.serializer(), DbrEvent.InspectorOpened("t1")),
        )
        // The explore view's recency, which the engine plan keeps by.
        assertEquals(listOf("b", "a"), roundTrip<ExploreView>("""{"tabs":[],"selected_tab":null,"ready":true,"recent_tabs":["b","a"]}""").recent_tabs)
    }

    /**
     * Spec 099: the signing machine's new words — the gate's block in its
     * view, the signer kinds of a failed passkey, and the outcome that
     * carries the classifier's kind.
     */
    @Test
    fun theSigningGateAndSignerKindsOf099Decode() {
        val view = roundTrip<SignView>(
            """{"surface":"sheet","confirm_gate_open":false,"confirm_block":"answered","failure_retryable":true,
               "error":{"kind":"signer_failed","detail":"the passkey ceremony failed"}}""",
        )
        assertEquals(ConfirmBlock.Answered, view.confirm_block)
        assertEquals(SignErrorKind.SignerFailed, view.error?.kind)
        for (kind in listOf("signer_unavailable", "signer_not_discoverable", "signer_failed")) {
            roundTrip<SignErrorNotice>("""{"kind":"$kind","detail":null}""")
        }
        assertEquals(
            SignSubmitOutcome.Failed("x", signer = app.getvela.wallet.feature.onboarding.core.FailureKind.NotDiscoverable),
            roundTrip<SignSubmitOutcome>("""{"type":"failed","message":"x","refused":false,"signer":"not_discoverable"}"""),
        )
        assertEquals(
            ConfirmState(enabled = false, block = ConfirmBlock.FeeShort, key = "componentsUi.signing.confirmBlock.feeShort"),
            roundTrip<ConfirmState>("""{"enabled":false,"block":"fee_short","key":"componentsUi.signing.confirmBlock.feeShort"}"""),
        )
        val entry = roundTrip<TrackEntryView>(
            """{"user_op_hash":"0xaa","chain_id":42161,"record_ids":[],"status":"pending","tx_hash":null,"polling":true,"submitted_at_ms":1.0,"outcome":"landing","relay_tx_hash":null,"relay_sent_at_ms":4000.0}""",
        )
        assertEquals(4000.0, entry.relay_sent_at_ms!!, 0.0)
    }

    @Test
    fun exploreAndHistoryWiresMatchTheMirrors() {
        assertFieldsExist<ExploreView>("ExploreView")
        // Stored THROUGH these classes (issue #425): a core field missing here
        // is dropped on every write, so they must match the mirror exactly.
        assertFieldsExhaustive<ExploreDoc>("ExploreDoc")
        assertFieldsExhaustive<ExploreSite>("ExploreSite")
        assertFieldsExist<ExploreTab>("ExploreTab")
        assertVariantsExhaustive<ExploreOperation>("ExploreOperation")
        assertVariantsExhaustive<ExploreShellResult>("ExploreShellResult")
        assertStringUnion<ExploreSystemGroup>("ExploreSystemGroup")
        assertVariantsExist<ExploreEvent>("ExploreEvent")
        assertVariantFields(ExploreEvent.serializer(), "ExploreEvent")
        // Spec 099 navigation: where 探索 lands and where an opened site goes.
        assertStringUnion<app.getvela.wallet.feature.browser.core.ExploreEntry>("ExploreEntry")
        assertStringUnion<app.getvela.wallet.feature.browser.core.ExploreOpenKind>("ExploreOpenKind")
        assertVariantsExhaustive<app.getvela.wallet.feature.browser.core.ExploreLanding>("ExploreLanding")
        assertVariantFieldsExhaustive(app.getvela.wallet.feature.browser.core.ExploreLanding.serializer(), "ExploreLanding")
        assertVariantsExhaustive<app.getvela.wallet.feature.browser.core.ExploreOpenTarget>("ExploreOpenTarget")
        assertVariantFieldsExhaustive(app.getvela.wallet.feature.browser.core.ExploreOpenTarget.serializer(), "ExploreOpenTarget")
        assertFieldsExist<BhistView>("BhistView")
        assertFieldsExist<BhistEntry>("BhistEntry")
        assertVariantsExhaustive<BhistOperation>("BhistOperation")
        assertVariantsExhaustive<BhistShellResult>("BhistShellResult")
        assertVariantsExist<BhistEvent>("BhistEvent")
        assertVariantFields(BhistEvent.serializer(), "BhistEvent")
    }

    /** The `batch_import` machine (spec 045): the wire the payroll batch crosses. */
    @Test
    fun `batch import wire matches the core`() {
        assertFieldsExist<BatchView>("BatchView")
        assertFieldsExist<BatchToken>("BatchToken")
        assertFieldsExist<BatchPreviewRow>("BatchPreviewRow")
        assertFieldsExist<BatchRecipient>("BatchRecipient")
        assertVariantsExhaustive<BatchOperation>("BatchOperation")
        assertVariantsExhaustive<BatchShellResult>("BatchShellResult")
        assertVariantsExhaustive<BatchFileContent>("BatchFileContent")
        assertVariantsExhaustive<BatchEvent>("BatchImportEvent")
        assertVariantFields(BatchEvent.serializer(), "BatchImportEvent")
        assertEquals(listOf("fiat", "token"), BatchUnit.serializer().descriptor.elementNames.toList())
        assertEquals(listOf("loading", "ok", "failed"), BatchRateStatus.serializer().descriptor.elementNames.toList())
        assertEquals(
            listOf("unreadable", "unsupported_encoding"),
            app.getvela.wallet.feature.send.core.BatchFileFailure.serializer().descriptor.elementNames.toList(),
        )
    }

    @Test
    fun signRequestWiresMatchTheMirrors() {
        assertFieldsExist<SignView>("SignView")
        assertFieldsExist<SignRequestView>("SignRequestView")
        assertFieldsExist<SignFundingView>("SignFundingView")
        assertFieldsExist<SignFundingNeeded>("SignFundingNeeded")
        assertFieldsExist<SignAccountRef>("SignAccountRef")
        assertFieldsExist<SignApproveOpts>("SignApproveOpts")
        assertFieldsExist<SignDappIdentity>("SignDappIdentity")
        assertFieldsExist<SignRecord>("SignRecord")
        assertFieldsExist<SignQuotedFee>("SignQuotedFee")
        assertFieldsExist<SignErrorNotice>("SignErrorNotice")
        assertFieldsExist<SignTrackerHandoff>("SignTrackerHandoff")
        assertFieldsExist<SignTrackerWithdraw>("SignTrackerWithdraw")
        assertVariantsExhaustive<SignOperation>("SignOperation")
        assertVariantsExhaustive<SignShellResult>("SignShellResult")
        assertVariantsExhaustive<SignResponsePayload>("SignResponsePayload")
        assertVariantsExhaustive<SignSubmitOutcome>("SignSubmitOutcome")
        assertVariantsExhaustive<SignSponsorship>("SignSponsorship")
        assertVariantsExhaustive<SignRecordClose>("SignRecordClose")
        assertVariantsExhaustive<SignNotice>("SignNotice")
        assertStringUnion<SignSurface>("SignSurface")
        assertStringUnion<SignSwipeAction>("SignSwipeAction")
        assertStringUnion<SignMethodKind>("SignMethodKind")
        assertStringUnion<SignErrorKind>("SignErrorKind")
        assertStringUnion<SignFundingPresentation>("SignFundingPresentation")
        assertStringUnion<SignRecordKind>("SignRecordKind")
        assertStringUnion<SignRecordStatus>("SignRecordStatus")
        assertStringUnion<SignSettledOutcome>("SignSettledOutcome")
        assertStringUnion<SignPhase>("SignPhase")
        // Spec 099 R7/R8: the gate and the signer's kind.
        assertStringUnion<ConfirmBlock>("ConfirmBlock")
        assertFieldsExist<ConfirmState>("ConfirmState")
        assertStringUnion<app.getvela.wallet.feature.onboarding.core.FailureKind>("FailureKind")
        assertVariantFields(SignSubmitOutcome.serializer(), "SignSubmitOutcome")
        assertVariantsExhaustive<SignEnding>("SignEnding")
        assertVariantsExhaustive<SignEndingState>("SignEndingState")
        assertVariantsExist<SignEvent>("SignEvent")
        assertVariantFields(SignOperation.serializer(), "SignOperation")
        assertVariantFields(SignEvent.serializer(), "SignEvent")
        // The wallet's own request is marked as such where it is raised, and
        // the view says so; a request that does not say is a page's.
        assertTrue("first_party" in serializer<SignRequestView>().descriptor.elementNames)
        val own = Wire.json.encodeToString(
            SignEvent.serializer(),
            SignEvent.RequestArrived(
                id = "1", method = "eth_sendTransaction", params_json = "[]", origin = "https://getvela.app",
                transport_id = "wallet", dedicated_transport = true, now_ms = 1.0, first_party = true,
            ),
        )
        assertTrue(own, "\"first_party\":true" in own)
        assertEquals(
            false,
            roundTrip<SignRequestView>("""{"id":"1","method":"eth_sign","kind":"eth_sign","params_json":"[]","origin":"https://x","chain_id":1}""").first_party,
        )
    }

    @Test
    fun clearSigningWiresMatchTheMirrors() {
        assertFieldsExist<ClearSigningView>("ClearSigningView")
        assertFieldsExist<ClearMessageView>("ClearMessageView")
        assertFieldsExist<ClearSignResult>("ClearSignResult")
        assertFieldsExist<ClearSignField>("ClearSignField")
        assertFieldsExist<ClearBlindTyped>("ClearBlindTyped")
        assertFieldsExist<ClearBlindField>("ClearBlindField")
        assertFieldsExist<ClearSiweFields>("ClearSiweFields")
        assertFieldsExist<ClearLocale>("ClearLocale")
        assertFieldsExist<ClearPlainSend>("ClearPlainSend")
        assertFieldsExist<ClearNativeValue>("ClearNativeValue")
        assertFieldsExist<ClearBatchCall>("ClearBatchCall")
        assertFieldsExist<ClearBatchView>("ClearBatchView")
        assertVariantsExhaustive<ClearOperation>("ClearOperation")
        assertVariantsExhaustive<ClearShellResult>("ClearShellResult")
        assertVariantsExhaustive<ClearConfirm>("ClearConfirm")
        assertStringUnion<ClearFieldRole>("ClearFieldRole")
        assertStringUnion<ClearSignType>("ClearSignType")
        assertStringUnion<ClearSignMethod>("ClearSignMethod")
        assertStringUnion<ClearProvenance>("ClearProvenance")
        assertStringUnion<ClearRisk>("ClearRisk")
        assertStringUnion<ClearDangerClass>("ClearDangerClass")
        assertStringUnion<ClearSurface>("ClearSurface")
        assertStringUnion<ClearProbe>("ClearProbe")
        assertStringUnion<ClearSiweBinding>("ClearSiweBinding")
        assertStringUnion<ClearDateFormat>("ClearDateFormat")
        assertStringUnion<ClearNumberFormat>("ClearNumberFormat")
        assertStringUnion<ClearTimeFormat>("ClearTimeFormat")
        assertVariantsExist<ClearSigningEvent>("ClearSigningEvent")
        assertVariantFields(ClearOperation.serializer(), "ClearOperation")
        assertVariantFields(ClearSigningEvent.serializer(), "ClearSigningEvent")
    }

    @Test
    fun approvalGuardWiresMatchTheMirrors() {
        assertFieldsExist<GuardView>("GuardView")
        assertFieldsExist<GuardEditorView>("GuardEditorView")
        assertFieldsExist<GuardLegView>("GuardLegView")
        assertFieldsExist<GuardBatchView>("GuardBatchView")
        assertFieldsExist<GuardIncreaseTotalView>("GuardIncreaseTotalView")
        assertFieldsExist<GuardTokenMetaView>("GuardTokenMetaView")
        assertFieldsExist<GuardTokenMetaEntry>("GuardTokenMetaEntry")
        assertFieldsExist<GuardDetectedApproval>("GuardDetectedApproval")
        assertVariantsExhaustive<GuardOperation>("GuardOperation")
        assertVariantsExhaustive<GuardShellResult>("GuardShellResult")
        assertVariantsExhaustive<GuardChoice>("GuardChoice")
        assertVariantsExhaustive<GuardLocus>("GuardLocus")
        assertStringUnion<GuardApprovalKind>("GuardApprovalKind")
        assertStringUnion<GuardAmountError>("GuardAmountError")
        assertStringUnion<GuardBlockReason>("GuardBlockReason")
        assertStringUnion<GuardEditorMode>("GuardEditorMode")
        assertStringUnion<GuardSurface>("GuardSurface")
        assertVariantsExist<GuardEvent>("GuardEvent")
        assertVariantFields(GuardOperation.serializer(), "GuardOperation")
        assertVariantFields(GuardEvent.serializer(), "GuardEvent")
    }

    /** Every field this Kotlin class names must exist in the mirror. */
    private inline fun <reified T> assertFieldsExist(tsName: String) {
        val descriptor = serializer<T>().descriptor
        val mirror = tsMembers(tsName).single().keys
        for (index in 0 until descriptor.elementsCount) {
            val field = descriptor.getElementName(index)
            assertTrue(
                "$tsName.$field is declared in Kotlin but absent from the generated mirror " +
                    "(fields there: ${mirror.sorted()}) — a Rust rename, or a typo here",
                field in mirror,
            )
        }
    }

    /** Every field of the mirror is declared in Kotlin, and vice versa (a type stored verbatim). */
    private inline fun <reified T> assertFieldsExhaustive(tsName: String) {
        val descriptor = serializer<T>().descriptor
        val kotlin = (0 until descriptor.elementsCount).map { descriptor.getElementName(it) }
        assertEquals("$tsName fields must match the generated mirror exactly", tsMembers(tsName).single().keys.sorted(), kotlin.sorted())
    }

    /** Each variant's payload fields match that variant of the mirror exactly. */
    private fun assertVariantFieldsExhaustive(serializer: KSerializer<*>, tsName: String) {
        val members = tsMembers(tsName).associateBy { it["type"] ?: "" }
        for ((variant, descriptor) in subclassDescriptors(serializer.descriptor)) {
            val kotlin = (0 until descriptor.elementsCount).map { descriptor.getElementName(it) }
            val mirror = members[variant]?.keys.orEmpty() - "type"
            assertEquals("$tsName.$variant fields must match the generated mirror exactly", mirror.sorted(), kotlin.sorted())
        }
    }

    /** Every variant the mirror names must exist in Kotlin, and vice versa. */
    private inline fun <reified T> assertVariantsExhaustive(tsName: String) {
        val kotlin = variantNames(serializer<T>())
        val mirror = tsVariants(tsName)
        assertEquals(
            "$tsName variants must match the generated mirror exactly — a missing one is " +
                "an operation nobody answers",
            mirror.sorted(),
            kotlin.sorted(),
        )
        assertVariantFields(serializer<T>(), tsName)
    }

    /** Kotlin may raise a subset of the events the core accepts. */
    private inline fun <reified T> assertVariantsExist(tsName: String) {
        val kotlin = variantNames(serializer<T>())
        val mirror = tsVariants(tsName)
        for (variant in kotlin) {
            assertTrue(
                "$tsName.$variant is declared in Kotlin but absent from the mirror (there: $mirror)",
                variant in mirror,
            )
        }
        assertVariantFields(serializer<T>(), tsName)
    }

    /**
     * A fieldless Rust enum is a plain TypeScript string union, and Kotlin
     * declares it as an `enum class` whose `@SerialName`s must match it exactly.
     * A missing value here is a state the shell cannot decode at all.
     */
    private inline fun <reified T : Enum<T>> assertStringUnion(tsName: String) {
        val descriptor = serializer<T>().descriptor
        val kotlin = (0 until descriptor.elementsCount).map { descriptor.getElementName(it) }
        val mirror = mirror(tsName)
            .substringAfter("export type $tsName =")
            .substringBefore(";")
            .split("|")
            .map { it.trim().trim('"') }
            .filter { it.isNotEmpty() }
        assertEquals("$tsName values must match the generated mirror", mirror.sorted(), kotlin.sorted())
    }

    /** Each variant's payload fields must exist in that variant of the mirror. */
    private fun assertVariantFields(serializer: KSerializer<*>, tsName: String) {
        val members = tsMembers(tsName).associateBy { it["type"] ?: "" }
        for ((variant, descriptor) in subclassDescriptors(serializer.descriptor)) {
            val mirror = members[variant]?.keys.orEmpty()
            for (index in 0 until descriptor.elementsCount) {
                val field = descriptor.getElementName(index)
                assertTrue(
                    "$tsName.$variant.$field is not in the generated mirror (there: ${mirror.sorted()})",
                    field in mirror,
                )
            }
        }
    }

    // -- kotlinx descriptors -------------------------------------------------

    private inline fun <reified T> elementDescriptor(name: String): SerialDescriptor {
        val descriptor = serializer<T>().descriptor
        val index = descriptor.getElementIndex(name)
        assertTrue("no such Kotlin field: $name", index >= 0)
        return descriptor.getElementDescriptor(index)
    }

    private fun variantNames(serializer: KSerializer<*>): List<String> =
        subclassDescriptors(serializer.descriptor).keys.toList()

    /**
     * The subclasses of a sealed hierarchy, by their `@SerialName`.
     *
     * kotlinx models a sealed class as two elements — the discriminator and a
     * holder whose element NAMES are the subclasses' serial names. That layout
     * is an implementation detail of the library, so it is asserted rather than
     * assumed: a kotlinx upgrade that changed it would otherwise turn this whole
     * file into a test that checks nothing and passes.
     */
    private fun subclassDescriptors(sealed: SerialDescriptor): Map<String, SerialDescriptor> {
        assertEquals(
            "expected a sealed hierarchy: ${sealed.serialName}",
            PolymorphicKind.SEALED,
            sealed.kind,
        )
        assertEquals(
            "kotlinx's sealed descriptor layout changed — this test needs rewriting, " +
                "not deleting",
            2,
            sealed.elementsCount,
        )
        val holder = sealed.getElementDescriptor(1)
        return (0 until holder.elementsCount).associate { index ->
            holder.getElementName(index) to holder.getElementDescriptor(index)
        }
    }

    // -- the generated mirrors ----------------------------------------------

    private fun mirror(tsName: String): String {
        val root = System.getProperty("vela.repo.root")
            ?: error("vela.repo.root system property not set (see app/build.gradle.kts testOptions)")
        val file = File(root, "app-web/vela-wallet/src/lib/core/generated/$tsName.ts")
        assertTrue("generated mirror missing: ${file.absolutePath}", file.isFile)
        return file.readText()
    }

    /**
     * The members of a ts-rs type: one map per union arm (a struct has one),
     * from field name to declared type.
     *
     * ts-rs output is mechanical — `export type X = { a: string, } | { … };` —
     * so this reads it directly rather than pulling in a TypeScript parser for
     * three shapes of declaration.
     */
    private fun tsMembers(tsName: String): List<Map<String, String>> {
        val body = mirror(tsName)
            .replace(Regex("/\\*\\*.*?\\*/", RegexOption.DOT_MATCHES_ALL), " ")
            .substringAfter("export type $tsName =")
            .substringBeforeLast(";")
        return splitTopLevel(body, '|')
            .map { it.trim().removePrefix("{").removeSuffix("}") }
            .map { member ->
                splitTopLevel(member, ',')
                    .mapNotNull { field ->
                        // ts-rs writes an optional field (`skip_serializing_if`) as `name?:`.
                        val name = field.substringBefore(':', "").trim().trim('"').removeSuffix("?")
                        val type = field.substringAfter(':', "").trim()
                        if (name.isEmpty() || type.isEmpty()) null else name to type.trim('"')
                    }
                    .toMap()
            }
    }

    /** The `"type"` discriminators of a ts-rs union. */
    private fun tsVariants(tsName: String): List<String> =
        tsMembers(tsName).mapNotNull { it["type"] }

    private fun tsFieldIsNullable(tsName: String, field: String): Boolean =
        tsMembers(tsName).single()[field]?.contains("null") == true

    private fun tsFieldType(tsName: String, field: String): String? =
        tsMembers(tsName).single()[field]

    /** Split on [separator], ignoring separators inside braces or angle brackets. */
    private fun splitTopLevel(text: String, separator: Char): List<String> {
        val parts = mutableListOf<String>()
        val current = StringBuilder()
        var depth = 0
        for (character in text) {
            when (character) {
                '{', '<', '(' -> depth++
                '}', '>', ')' -> depth--
            }
            if (character == separator && depth == 0) {
                parts += current.toString()
                current.clear()
            } else {
                current.append(character)
            }
        }
        parts += current.toString()
        return parts.filter { it.isNotBlank() }
    }
}
