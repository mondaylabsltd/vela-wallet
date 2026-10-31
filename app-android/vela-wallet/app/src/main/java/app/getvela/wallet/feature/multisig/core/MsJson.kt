package app.getvela.wallet.feature.multisig.core

import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.doubleOrNull
import kotlinx.serialization.json.longOrNull

/*
 * Lenient reads over the machines' JSON (the views carry records the core
 * reads back whole, keyed by address or by hash): a field that is not there,
 * or not what it should be, reads as absent — a newer core never blanks a
 * screen. And the one builder events are written with.
 */

operator fun JsonElement?.get(key: String): JsonElement? = (this as? JsonObject)?.get(key)?.takeUnless { it is JsonNull }

val JsonElement?.str: String?
    get() = (this as? JsonPrimitive)?.let { p ->
        if (p is JsonNull) null else if (p.isString) p.content else p.longOrNull?.toString() ?: p.contentOrNull
    }

val JsonElement?.long: Long?
    get() = (this as? JsonPrimitive)?.let { it.longOrNull ?: it.doubleOrNull?.toLong() ?: it.contentOrNull?.toLongOrNull() }

val JsonElement?.int: Int? get() = long?.toInt()

val JsonElement?.bool: Boolean? get() = (this as? JsonPrimitive)?.booleanOrNull

val JsonElement?.arr: List<JsonElement> get() = (this as? JsonArray) ?: emptyList()

val JsonElement?.obj: JsonObject get() = (this as? JsonObject) ?: JsonObject(emptyMap())

/** A tagged value's tag: `kind`, else `type`. */
val JsonElement?.kind: String? get() = this["kind"].str ?: this["type"].str

val JsonElement?.isNullish: Boolean get() = this == null || this is JsonNull

/** Any Kotlin value as JSON: maps, lists, strings, numbers, booleans, JSON already. */
fun jsonOf(value: Any?): JsonElement = when (value) {
    null -> JsonNull
    is JsonElement -> value
    is String -> JsonPrimitive(value)
    is Number -> JsonPrimitive(value)
    is Boolean -> JsonPrimitive(value)
    is Map<*, *> -> JsonObject(value.entries.associate { (k, v) -> k.toString() to jsonOf(v) })
    is Iterable<*> -> JsonArray(value.map(::jsonOf))
    is Array<*> -> JsonArray(value.map(::jsonOf))
    else -> JsonPrimitive(value.toString())
}

/** `{ "type": type, fields… }` — an event as the core reads it. */
fun ev(type: String, vararg fields: Pair<String, Any?>): JsonObject =
    JsonObject(linkedMapOf<String, JsonElement>("type" to JsonPrimitive(type)).apply {
        for ((k, v) in fields) put(k, jsonOf(v))
    })

/** A JSON object from pairs. */
fun jo(vararg fields: Pair<String, Any?>): JsonObject = JsonObject(fields.associate { (k, v) -> k to jsonOf(v) })
