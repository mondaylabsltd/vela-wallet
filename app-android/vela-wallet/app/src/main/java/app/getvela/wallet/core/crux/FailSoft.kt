package app.getvela.wallet.core.crux

import kotlinx.serialization.KSerializer
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.builtins.nullable
import kotlinx.serialization.descriptors.SerialDescriptor
import kotlinx.serialization.encoding.Decoder
import kotlinx.serialization.encoding.Encoder
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonDecoder
import kotlinx.serialization.json.JsonNull

/*
 * Fail-soft decoding for the few view parts a newer core may extend with a
 * variant this build does not know (spec 093: a feed row's second-line parts,
 * its detail facts). [Wire] is intolerant of a wrong value on purpose — a
 * whole screen decoding to a default is the lie it exists to prevent — but
 * one unknown fact must not cost a person their whole Activity. These drop
 * exactly the part that does not read and keep the rest; encoding is the
 * plain list / value.
 */

/** A list whose elements decode one by one: an element that does not read is left out. */
open class FailSoftListSerializer<T>(private val element: KSerializer<T>) : KSerializer<List<T>> {
    private val list = ListSerializer(element)
    override val descriptor: SerialDescriptor = list.descriptor
    override fun serialize(encoder: Encoder, value: List<T>) = list.serialize(encoder, value)
    override fun deserialize(decoder: Decoder): List<T> {
        val json = decoder as? JsonDecoder ?: return list.deserialize(decoder)
        val array = json.decodeJsonElement() as? JsonArray ?: return emptyList()
        return array.mapNotNull { runCatching { json.json.decodeFromJsonElement(element, it) }.getOrNull() }
    }
}

/** An optional value that reads as absent when it does not read at all. */
open class FailSoftSerializer<T : Any>(private val inner: KSerializer<T>) : KSerializer<T?> {
    private val optional = inner.nullable
    override val descriptor: SerialDescriptor = optional.descriptor
    override fun serialize(encoder: Encoder, value: T?) = optional.serialize(encoder, value)
    override fun deserialize(decoder: Decoder): T? {
        val json = decoder as? JsonDecoder ?: return optional.deserialize(decoder)
        val element = json.decodeJsonElement()
        if (element is JsonNull) return null
        return runCatching { json.json.decodeFromJsonElement(inner, element) }.getOrNull()
    }
}
