package app.getvela.wallet

import app.getvela.wallet.feature.settings.core.CurrencyEvent
import app.getvela.wallet.feature.settings.core.CurrencyOperation
import app.getvela.wallet.feature.settings.core.CurrencyShellResult
import app.getvela.wallet.feature.settings.core.CurrencyView
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
