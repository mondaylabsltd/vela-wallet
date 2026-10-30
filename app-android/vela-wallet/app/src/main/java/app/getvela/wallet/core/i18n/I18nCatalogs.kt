package app.getvela.wallet.core.i18n

import org.json.JSONObject

/**
 * The on-demand namespaces (`scripts/gen-i18n.mjs` `ON_DEMAND`, round 5).
 *
 * A subtree only some screens draw is cut out of the main catalog
 * (`assets/i18n/<lng>.json`) and written as its own per-locale catalog,
 * `assets/i18n/<name>/<lng>.json`, rooted at its path
 * (`{"home":{"multisig":…}}`). A shell loads it for the screens that draw it
 * and deep-merges it into the locale's main catalog — never resident for the
 * readers who never open those screens. The web merges at build time
 * (`engine.server.ts` `catalogBytes`); this app merges when a screen asks
 * ([I18nRuntime.include]).
 *
 * The merge is the web's (`deepMerge` there and in `verify-i18n-parity.mjs`):
 * an object meets an object → merged key by key; anything else → the
 * on-demand side wins.
 */
object I18nCatalogs {

    /** `multisig` = `home.multisig`: the multisig screens, the connect page, the send flow's token note. */
    const val MULTISIG: String = "multisig"

    /** Every on-demand namespace gen-i18n writes (a test holds this to `assets/i18n/<dir>/`). */
    val ON_DEMAND: List<String> = listOf(MULTISIG)

    /** Where the app packages a namespace's catalog (`syncVelaI18nAssets` copies each `assets/i18n/<name>/<lng>.json`). */
    fun assetPath(tag: String, namespace: String? = null): String =
        if (namespace == null) "i18n/$tag.json" else "i18n/$namespace/$tag.json"

    /** [main] with each of [parts] deep-merged in, in order; [main] alone when there are none. */
    fun merged(main: ByteArray, parts: List<ByteArray>): ByteArray {
        if (parts.isEmpty()) return main
        var doc = JSONObject(String(main, Charsets.UTF_8))
        for (part in parts) doc = deepMerge(doc, JSONObject(String(part, Charsets.UTF_8)))
        return doc.toString().toByteArray(Charsets.UTF_8)
    }

    /** A new object: [a]'s keys, with [b]'s merged in (objects recursively, the rest replaced). */
    fun deepMerge(a: JSONObject, b: JSONObject): JSONObject {
        val out = JSONObject()
        for (key in a.keys()) out.put(key, a.get(key))
        for (key in b.keys()) {
            val value = b.get(key)
            val here = out.opt(key)
            out.put(key, if (value is JSONObject && here is JSONObject) deepMerge(here, value) else value)
        }
        return out
    }
}
