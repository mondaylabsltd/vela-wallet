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
import app.getvela.wallet.feature.send.core.TrackEntryView
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
        assertFieldsExist<BalanceSwitcherView>("BalanceSwitcherView")
    }

    @Test
    fun balanceOperationsAndResultsAreExhaustive() {
        assertVariantsExhaustive<BalanceOperation>("BalanceOperation")
        assertVariantsExhaustive<BalanceShellResult>("BalanceShellResult")
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
        assertFieldsExist<NetWizardView>("NetWizardView")
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
        assertFieldsExist<SendQuotedFee>("SendQuotedFee")
        assertFieldsExist<SendTxRecord>("SendTxRecord")
        assertFieldsExist<SendRecipientIdentity>("SendRecipientIdentity")
        assertFieldsExist<SendRecipientRisk>("SendRecipientRisk")
        assertFieldsExist<SendFeeIssueView>("SendFeeIssueView")
        assertFieldsExist<SendUnitIssue>("SendUnitIssue")
        assertFieldsExist<SendRecipientDraft>("SendRecipientDraft")
        assertFieldsExist<SendMultiSpecView>("SendMultiSpecView")
        assertFieldsExist<SendReceiptView>("SendReceiptView")
        assertFieldsExist<SendReceiptTransfer>("SendReceiptTransfer")
        assertFieldsExist<SendAccountRef>("SendAccountRef")
        assertFieldsExist<SendOpenParams>("SendOpenParams")
        assertFieldsExist<SendDisplayContext>("SendDisplayContext")
        assertFieldsExist<SendSplitRowIssue>("SendSplitRowIssue")
        assertFieldsExist<SendDuplicateRowView>("SendDuplicateRowView")
        assertStringUnion<SendRowFieldState>("SendRowFieldState")
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
        assertStringUnion<SendEstimateFailure>("SendEstimateFailure")
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
        assertFieldsExist<FeeEstimateView>("FeeEstimateView")
        assertFieldsExist<FeeBundlerQuote>("FeeBundlerQuote")
        assertFieldsExist<FeeAssetQuote>("FeeAssetQuote")
        assertFieldsExist<FeeCall>("FeeCall")
    }

    @Test
    fun feeOperationsResultsAndOutcomesAreExhaustive() {
        assertVariantsExhaustive<FeeOperation>("FeeOperation")
        assertVariantsExhaustive<FeeShellResult>("FeeShellResult")
        assertVariantsExhaustive<FeeGasOutcome>("FeeGasOutcome")
        assertVariantsExhaustive<FeeAssetView>("FeeAssetView")
        assertVariantsExist<FeeEvent>("FeeEvent")
        assertStringUnion<FeeTier>("FeeTier")
        assertStringUnion<FeeFailure>("FeeFailure")
        assertStringUnion<FeeAssetKind>("FeeAssetKind")
    }

    /** Spec 071: the default "Sign with" and the Trusted Signer page. */
    @Test
    fun signPreferenceMatchesTheGeneratedMirrors() {
        assertFieldsExist<app.getvela.wallet.feature.settings.core.SignPrefView>("SignPrefView")
        assertVariantsExhaustive<app.getvela.wallet.feature.settings.core.SignPrefOperation>("SignPrefOperation")
        assertVariantsExhaustive<app.getvela.wallet.feature.settings.core.SignPrefShellResult>("SignPrefShellResult")
        assertVariantsExist<app.getvela.wallet.feature.settings.core.SignPrefEvent>("SignPrefEvent")
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
        assertFieldsExist<TrackEntryView>("TrackEntryView")
        assertFieldsExist<TrackPendingRecord>("TrackPendingRecord")
        assertFieldsExist<TrackRecordPatch>("TrackRecordPatch")
        assertVariantsExhaustive<TrackOperation>("TrackOperation")
        assertVariantsExhaustive<TrackShellResult>("TrackShellResult")
        assertVariantsExist<TrackEvent>("TrackEvent")
        assertStringUnion<TrackLifecycle>("TrackLifecycle")
        assertStringUnion<TrackStatus>("TrackStatus")
        assertStringUnion<TrackRecordStatus>("TrackRecordStatus")
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
    }

    @Test
    fun exploreAndHistoryWiresMatchTheMirrors() {
        assertFieldsExist<ExploreView>("ExploreView")
        assertFieldsExist<ExploreGroupView>("ExploreGroupView")
        assertFieldsExist<ExploreDoc>("ExploreDoc")
        assertFieldsExist<ExploreSite>("ExploreSite")
        assertFieldsExist<ExploreGroup>("ExploreGroup")
        assertFieldsExist<ExploreTab>("ExploreTab")
        assertVariantsExhaustive<ExploreOperation>("ExploreOperation")
        assertVariantsExhaustive<ExploreShellResult>("ExploreShellResult")
        assertStringUnion<ExploreSystemGroup>("ExploreSystemGroup")
        assertVariantsExist<ExploreEvent>("ExploreEvent")
        assertVariantFields(ExploreEvent.serializer(), "ExploreEvent")
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
        assertVariantsExist<SignEvent>("SignEvent")
        assertVariantFields(SignOperation.serializer(), "SignOperation")
        assertVariantFields(SignEvent.serializer(), "SignEvent")
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
                        val name = field.substringBefore(':', "").trim().trim('"')
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
