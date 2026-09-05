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
import app.getvela.wallet.feature.settings.core.CurrencyEvent
import app.getvela.wallet.feature.wallet.core.BalanceCacheEntry
import app.getvela.wallet.feature.wallet.core.BalanceEvent
import app.getvela.wallet.feature.wallet.core.BalanceNotice
import app.getvela.wallet.feature.wallet.core.BalanceOperation
import app.getvela.wallet.feature.wallet.core.BalanceShellResult
import app.getvela.wallet.feature.wallet.core.BalanceSwitcherView
import app.getvela.wallet.feature.wallet.core.BalanceToken
import app.getvela.wallet.feature.wallet.core.BalanceView
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

    // -- assertions ----------------------------------------------------------

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
