/**
 * Replays the conformance corpus through the uniffi-generated KOTLIN bindings.
 *
 * `cargo test` proves the Rust crate matches the corpus and verify-web.mjs
 * proves the shipped wasm does; this proves the third surface — the bindings
 * the planned native Android app will consume — agrees byte-for-byte
 * (spec SC-001). A green cargo test with a red run here would mean the FFI
 * layer, not the core, diverged.
 *
 * Vectors: the JSON files under rust/crates/vela-core/tests/vectors
 * Schema:  specs/001-rust-core-bindings/contracts/conformance-vectors.md
 */

import java.io.File
import org.json.JSONArray
import org.json.JSONObject
import uniffi.vela_core_uniffi.*

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fun hex(bytes: ByteArray): String {
    val sb = StringBuilder("0x")
    for (b in bytes) sb.append(String.format("%02x", b))
    return sb.toString()
}

/**
 * Strict hex decode, mirroring `in_bytes` in conformance.rs.
 *
 * An odd length must fail rather than silently drop the trailing nibble: a case
 * decoded from truncated input satisfies its expectation only by accident, and
 * the harness would report green over arguments the corpus never specified.
 */
fun bytes(s: String): ByteArray {
    val clean = if (s.startsWith("0x")) s.substring(2) else s
    require(clean.length % 2 == 0) { "odd-length hex `$s`" }
    val out = ByteArray(clean.length / 2)
    for (i in out.indices) {
        out[i] = clean.substring(i * 2, i * 2 + 2).toInt(16).toByte()
    }
    return out
}

/** Canonical JSON rendering of an AbiValue tree, matching the vector shape. */
fun abiValueToJson(v: AbiValue): JSONObject {
    val children = JSONArray()
    for (c in v.children) children.put(abiValueToJson(c))
    return JSONObject()
        .put("kind", v.kind)
        .put("name", v.name)
        .put("value", v.value)
        .put("children", children)
}

/** Structural comparison — key order must NOT matter. */
fun jsonEquals(a: Any?, b: Any?): Boolean {
    // A uniffi `Vec<String>` arrives as a Kotlin List, not a JSONArray. Without
    // this the two compare by `toString()` — `["_one","_other"]` against
    // `[_one, _other]` — and every list-returning function fails on formatting
    // rather than on value. Same class as the array bug in verify-web.mjs.
    if (a is JSONArray && b is List<*>) return jsonEquals(a, JSONArray(b))
    if (a is List<*> && b is JSONArray) return jsonEquals(JSONArray(a), b)
    if (a is List<*> && b is List<*>) return jsonEquals(JSONArray(a), JSONArray(b))
    if (a is JSONObject && b is Map<*, *>) return jsonEquals(a, JSONObject(b))
    if (a is Map<*, *> && b is JSONObject) return jsonEquals(JSONObject(a), b)
    if (a is JSONObject && b is JSONObject) {
        if (a.length() != b.length()) return false
        for (k in a.keys()) {
            if (!b.has(k)) return false
            if (!jsonEquals(a.get(k), b.get(k))) return false
        }
        return true
    }
    if (a is JSONArray && b is JSONArray) {
        if (a.length() != b.length()) return false
        for (i in 0 until a.length()) if (!jsonEquals(a.get(i), b.get(i))) return false
        return true
    }
    if (a is Boolean || b is Boolean) return a == b
    if (a == JSONObject.NULL || b == JSONObject.NULL) return a == b
    return a?.toString() == b?.toString()
}

/** The CoreError variant name, as the corpus spells it. */
fun errorCode(e: CoreException): String = e.javaClass.simpleName

// ---------------------------------------------------------------------------
// Dispatch — one arm per contracts/core-api.md function (mirrors conformance.rs)
// ---------------------------------------------------------------------------

fun runCase(fn: String, input: JSONObject): Any? = when (fn) {
    // primitives
    "keccak256" -> hex(keccak256(bytes(input.getString("data"))))
    "sha256" -> hex(sha256(bytes(input.getString("data"))))
    "to_hex" -> toHex(bytes(input.getString("data")), input.getBoolean("prefixed"))
    "from_hex" -> hex(fromHex(input.getString("s")))
    "to_quantity" -> toQuantity(input.getString("value"))
    "checksum_address" -> checksumAddress(input.getString("address_hex"))
    "function_selector" -> hex(functionSelector(input.getString("signature")))
    "create2_address" -> create2Address(
        input.getString("deployer_hex"),
        bytes(input.getString("salt")),
        bytes(input.getString("init_code_hash")),
    )
    "to_base64url" -> toBase64url(bytes(input.getString("data")))
    "from_base64url" -> hex(fromBase64url(input.getString("s")))
    "abi_encode_address" -> hex(abiEncodeAddress(input.getString("address_hex")))
    "abi_encode_uint256" -> hex(abiEncodeUint256(input.getString("value_hex")))
    "abi_encode_bytes32" -> hex(abiEncodeBytes32(bytes(input.getString("data"))))

    // abi
    "canonicalize_signature" -> canonicalizeSignature(input.getString("sig"))
    "compute_selector" -> computeSelector(input.getString("sig"))
    "match_selector" -> matchSelector(input.getString("sig"), bytes(input.getString("calldata")))
    "decode_calldata" -> abiValueToJson(
        decodeCalldata(input.getString("sig"), bytes(input.getString("calldata")))
    )

    // eip712
    "hash_typed_data" -> hex(hashTypedData(input.getString("typed_data_json")))
    "encode_type" -> encodeType(input.getString("typed_data_json"))

    // safe
    "parse_public_key" -> parsePublicKey(input.getString("hex")).let {
        JSONObject().put("x", hex(it.x)).put("y", hex(it.y))
    }
    "compute_safe_address" -> computeSafeAddress(
        bytes(input.getString("x")),
        bytes(input.getString("y")),
    ).let {
        JSONObject()
            .put("address", it.address)
            .put("salt_nonce", hex(it.saltNonce))
            .put("setup_data", hex(it.setupData))
            .put("init_code_hash", hex(it.initCodeHash))
    }
    "compute_safe_address_multi" -> {
        val keysJson = input.getJSONArray("keys")
        val keys = (0 until keysJson.length()).map { i ->
            val k = keysJson.getJSONObject(i)
            P256PublicKey(bytes(k.getString("x")), bytes(k.getString("y")))
        }
        computeSafeAddressMulti(keys).let {
            JSONObject()
                .put("address", it.address)
                .put("salt_nonce", hex(it.saltNonce))
                .put("setup_data", hex(it.setupData))
                .put("init_code_hash", hex(it.initCodeHash))
        }
    }
    "compute_webauthn_signer_address" -> JSONObject().put(
        "address",
        computeWebauthnSignerAddress(bytes(input.getString("x")), bytes(input.getString("y"))),
    )
    "compute_splitter_address" -> computeSplitterAddress(input.getString("treasury_hex"))
    "encode_splitter_deploy_call" -> hex(encodeSplitterDeployCall(input.getString("treasury_hex")))
    "safe_proxy_runtime_code" -> safeProxyRuntimeCode()

    // webauthn
    "extract_attestation_public_key" ->
        extractAttestationPublicKey(bytes(input.getString("attestation_object"))).let {
            JSONObject().put("x", hex(it.x)).put("y", hex(it.y))
        }
    "der_signature_to_raw_low_s" -> hex(derSignatureToRawLowS(bytes(input.getString("der"))))
    "validate_client_data" -> {
        val kind = when (input.getString("kind")) {
            "Get" -> ClientDataKind.GET
            "Create" -> ClientDataKind.CREATE
            else -> throw IllegalArgumentException("unknown ClientDataKind")
        }
        validateClientData(
            kind,
            bytes(input.getString("client_data_json")),
            bytes(input.getString("authenticator_data")),
        )
        true
    }
    "webauthn_signing_hash" -> hex(
        webauthnSigningHash(
            bytes(input.getString("authenticator_data")),
            bytes(input.getString("client_data_json")),
        )
    )
    "recover_public_key_from_assertions" -> {
        val a = input.getJSONObject("a")
        val b = input.getJSONObject("b")
        val key = recoverPublicKeyFromAssertions(
            WebAuthnAssertion(
                bytes(a.getString("authenticator_data")),
                bytes(a.getString("client_data_json")),
                bytes(a.getString("signature_der")),
            ),
            WebAuthnAssertion(
                bytes(b.getString("authenticator_data")),
                bytes(b.getString("client_data_json")),
                bytes(b.getString("signature_der")),
            ),
        )
        if (key == null) JSONObject.NULL
        else "04" + hex(key.x).substring(2) + hex(key.y).substring(2)
    }

    // identicon (specs/003-rust-identicon). Params expectations carry section
    // INDICES; `sectionIndex` resolves the returned artwork back to one using the
    // table pinned by the corpus's own `section-table` group, so nothing here is
    // circular — both ends are anchored to the identicons-esm oracle.
    "make_hash" -> identiconMakeHash(input.getString("seed"))
    "identicon_svg" -> identiconSvg(input.getString("seed"))
    "identicon_svg_circular" -> identiconSvgCircular(input.getString("seed"))
    "identicon_data_uri" -> identiconDataUri(input.getString("seed"))
    "normalize_seed" -> identiconNormalizeSeed(input.getString("seed"))
    "identicon_params" -> identiconParams(input.getString("seed")).let { p ->
        JSONObject()
            .put("main", p.main)
            .put("background", p.background)
            .put("accent", p.accent)
            .put("face", sectionIndex("face", p.face))
            .put("top", sectionIndex("top", p.top))
            .put("sides", sectionIndex("sides", p.sides))
            .put("bottom", sectionIndex("bottom", p.bottom))
    }

    // --- i18n (spec 004-rust-i18n) ---
    //
    // Catalogs arrive as JSON bytes, the same on-demand route the web build uses,
    // so this surface exercises `Values::Owned` while the Rust suite exercises
    // `Values::Static`. A divergence between the two shows up here.
    "i18n_t" -> i18nEngine(input.optString("lng", "en"))
        .t(anyKey(input, "key"), i18nOpts(input.optJSONObject("opts")))
    "i18n_t_keys" -> {
        val arr = input.optJSONArray("keys")
        val keys = (0 until (arr?.length() ?: 0)).map { anyString(arr!!.opt(it)) }
        i18nEngine(input.optString("lng", "en")).tFirst(keys, i18nOpts(input.optJSONObject("opts")))
    }
    "i18n_t_lng_option" -> {
        // The per-call `lng` path: the ACTIVE language stays `en` while resolution
        // runs against the tag in the options. Two different upstream functions.
        val target = canonicalTag(input.optJSONObject("opts")?.optString("lng", "en") ?: "en")
        i18nEngine(target, active = "en").t(anyKey(input, "key"), i18nOpts(input.optJSONObject("opts")))
    }
    "i18n_t_legacy_plural" -> i18nEngine(input.optString("lng", "en"), legacyPlurals = true)
        .t(anyKey(input, "key"), i18nOpts(input.optJSONObject("opts")))
    "i18n_interpolate" -> i18nInterpolate(input.getString("template"), i18nOpts(input.optJSONObject("opts")))
    "i18n_plural_suffix" -> i18nPluralSuffix(input.getString("lng"), input.getDouble("count"))
    "i18n_plural_suffixes" -> i18nPluralSuffixes(input.getString("lng"))
    "i18n_plural_suffix_legacy" -> i18nPluralSuffixLegacy(input.getDouble("count"))
    "i18n_plural_suffixes_legacy" -> i18nPluralSuffixesLegacy()
    "i18n_resolve_language", "i18n_change_language" -> {
        val st = i18nEngine("en").changeLanguage(input.getString("requested"))
        // A JSONObject, not a Map: the field-wise comparison branch casts to
        // JSONObject and reports "expected an object result" for anything else.
        JSONObject()
            .put("language", st.language)
            .put("resolved_language", st.resolvedLanguage ?: JSONObject.NULL)
            .put("languages", JSONArray(st.languages))
    }
    else -> throw NoSuchElementException("no dispatch arm for fn `$fn` — add it to Harness.kt")
}

// ---------------------------------------------------------------------------

/**
 * The corpus is five suites, discovered by scanning the directory. Asserting the
 * exact set is what stops a vector file lost to a bad merge or a partial checkout
 * from making this harness report "green" over a corpus that silently shrank —
 * the precise false confidence this feature exists to prevent.
 */
val REQUIRED_SUITES = listOf(
    "abi", "eip712",
    // `i18n-*` sorts before `identicon`: '1' is 0x31, 'd' is 0x64.
    "i18n-behaviour", "i18n-exhaustive", "i18n-plural", "i18n-plural-legacy",
    "identicon", "identicon-bulk", "primitives", "safe", "safe-multi", "webauthn",
)

/**
 * Functions that exist in vela-core but are deliberately NOT on any binding surface
 * (specs/003-rust-identicon contracts/identicon-api.md): a test-only parity device,
 * plus helpers no Vela platform calls. Skipping is counted and reported — an
 * unreported skip is how a corpus quietly stops covering things.
 */
// ---------------------------------------------------------------------------
// i18n helpers
// ---------------------------------------------------------------------------

/** i18next only canonicalises tags containing `-`; a bare `ZH` is left alone. */
fun canonicalTag(t: String): String {
    if (!t.contains("-")) return t
    return t.split("-").mapIndexed { i, p ->
        when {
            i == 0 -> p.lowercase()
            p.length == 4 -> p.replaceFirstChar { it.uppercase() }.let { it[0] + it.substring(1).lowercase() }
            p.length == 2 -> p.uppercase()
            else -> p.lowercase()
        }
    }.joinToString("-")
}

var i18nAssetDirPath: String = ""
val i18nAssets = mutableMapOf<String, ByteArray>()

fun i18nAsset(lng: String): ByteArray? = i18nAssets.getOrPut(lng) {
    val f = File("$i18nAssetDirPath/$lng.json")
    if (f.exists()) f.readBytes() else return null
}

fun i18nEngine(lng: String, active: String? = null, legacyPlurals: Boolean = false): I18n {
    val en = i18nAsset("en") ?: throw IllegalStateException("no en i18n asset at $i18nAssetDirPath")
    val engine = if (legacyPlurals) I18n.newWithLegacyPlurals(en) else I18n(en)
    val tag = canonicalTag(lng)
    if (tag != "en") i18nAsset(tag)?.let { engine.loadCatalog(tag, it) }
    engine.changeLanguage(active ?: tag)
    return engine
}

/**
 * ECMAScript `Number::toString`, which Kotlin's `Double.toString` is not.
 *
 * Kotlin renders 1e21 as `"1.0E21"`; JS renders `"1e+21"`. The FFI record takes
 * pre-stringified variables, so the CALLER owns this conversion — in a real
 * Android app exactly as much as here. Rust uses `ryu-js` for the same reason.
 */
fun jsNumberString(d: Double): String {
    if (d.isNaN()) return "NaN"
    if (d.isInfinite()) return if (d > 0) "Infinity" else "-Infinity"
    if (d == 0.0) return "0"
    // Integers below 1e21 print without an exponent and without a fraction.
    if (d == Math.floor(d) && Math.abs(d) < 1e21) {
        return java.math.BigDecimal(d).toBigInteger().toString()
    }
    return d.toString()
        .replace("E", "e")
        .let { if (it.contains("e") && !it.contains("e-")) it.replace("e", "e+") else it }
        .replace(".0e", "e")
}

/** i18next `String()`-coerces a non-string key. */
fun anyString(v: Any?): String = when (v) {
    null, JSONObject.NULL -> "null"
    is String -> v
    is Boolean -> if (v) "true" else "false"
    is Double -> jsNumberString(v)
    is Int -> v.toString()
    is Long -> v.toString()
    is Number -> jsNumberString(v.toDouble())
    else -> v.toString()
}

fun anyKey(input: JSONObject, key: String): String = anyString(input.opt(key))

/**
 * Option keys the uniffi record deliberately does NOT model.
 *
 * The FFI surface carries what a native app actually passes. The rest are corpus
 * probes of i18next's edge behaviour; modelling them would widen a wallet's FFI
 * contract to serve a test. Cases carrying one are SKIPPED — counted and printed.
 */
val I18N_UNMODELLED_OPTIONS = setOf(
    "returnObjects", "returnDetails", "joinArrays", "keySeparator", "nsSeparator",
    "ns", "replace",
)

fun i18nExpressible(opts: JSONObject?): Boolean {
    if (opts == null) return true
    for (k in opts.keys()) {
        if (k in I18N_UNMODELLED_OPTIONS || k.startsWith("defaultValue_")) return false
        val v = opts.opt(k)
        if (v is JSONObject) {
            if (v.has("__t")) return false
            return false
        }
        if (v is JSONArray) return false
        if (k == "count" && v !is Number) return false
        if (k == "defaultValue" && v !is String) return false
    }
    return true
}

fun i18nOpts(opts: JSONObject?): TOptions {
    if (opts == null) return TOptions(null, null, null, null, false, emptyList())
    var count: Double? = null
    var context: String? = null
    var defaultValue: String? = null
    var lng: String? = null
    var ordinal = false
    val vars = mutableListOf<TVar>()
    for (k in opts.keys()) {
        val v = opts.opt(k)
        when (k) {
            "count" -> if (v is Number) count = v.toDouble()
            "context" -> context = anyString(v)
            "defaultValue" -> if (v is String) defaultValue = v
            "lng" -> lng = v as? String
            "ordinal" -> ordinal = v as? Boolean ?: false
            else -> vars.add(TVar(k, if (v == JSONObject.NULL) null else anyString(v)))
        }
    }
    return TOptions(count, context, defaultValue, lng, ordinal, vars)
}

var i18nSkipped = 0

val CORE_ONLY_FNS = setOf(
    "identicon_params_js_compat", "section_svg", "create_identicon",
    "nimiq_is_valid_address", "constants",
)

/**
 * Artwork -> 1-based index, built from the corpus's own `section-table` cases so the
 * compact index form used by every params expectation can be checked here too.
 */
val fragmentIndex = mutableMapOf<String, Int>()

fun sectionIndex(section: String, svg: String): Any =
    fragmentIndex["$section:$svg"] ?: JSONObject.NULL


/**
 * Drive one create-wallet dispatch through the Crux bridge (spec 019, T100).
 *
 * The conformance corpus above proves the PURE functions agree across surfaces.
 * It says nothing about the state machines, which arrived on this crate in spec
 * 019 and are what the Android app actually runs. This is the narrow claim the
 * smoke can make without a passkey provider: the bridge accepts an event as
 * JSON, advances the machine, hands back the effect the core is waiting on, and
 * honours the correlation rules.
 *
 * It deliberately stops before `register_passkey` — performing that needs
 * Credential Manager and an Activity, neither of which exists in a JVM harness.
 */
fun checkOnboardingBridge() {
    val core = CreateWalletCore()

    fun dispatch(event: String): JSONObject = JSONObject(core.dispatch(event))
    fun fail(why: String): Nothing {
        System.err.println("smoke-kotlin: onboarding bridge — $why")
        System.exit(1)
        throw IllegalStateException()
    }

    // The form is pure model: it must advance with no effect at all.
    dispatch("""{"type":"start"}""")
    dispatch("""{"type":"name_changed","name":"Ada"}""")
    dispatch("""{"type":"ack_toggled","index":0}""")
    dispatch("""{"type":"ack_toggled","index":1}""")
    // ACK_COUNT is 3 since the privacy/terms ack joined the form.
    val acked = dispatch("""{"type":"ack_toggled","index":2}""")
    if (acked.getJSONArray("effects").length() != 0) {
        fail("filling the form asked the shell to do something")
    }
    if (!acked.getJSONObject("view").getBoolean("can_submit")) {
        fail("a named, fully-acked form is not submittable")
    }

    // Submitting is where the shell is first asked for anything.
    val submitted = dispatch("""{"type":"submit"}""")
    val effects = submitted.getJSONArray("effects")
    if (effects.length() != 1) fail("submit produced ${effects.length()} effects, want 1")
    val first = effects.getJSONObject(0)
    val id = first.getLong("id")
    if (id != 1L) fail("effect ids are not monotonic from 1 — got $id")
    val operation = first.getJSONObject("operation").getString("type")
    if (operation != "check_passkey_support") fail("submit asked for `$operation`")

    // Answering it advances the machine and asks for the next thing.
    val supported =
        JSONObject(core.resolveEffect(id.toULong(), """{"type":"passkey_support","supported":true}"""))
    val next = supported.getJSONArray("effects")
    if (next.length() != 1 || next.getJSONObject(0).getJSONObject("operation").getString("type") != "generate_group_key") {
        fail("answering passkey support did not lead to `generate_group_key`")
    }

    // Rule 2: an answer that outlived its question changes nothing and is not
    // an error. A bridge that threw here would turn every raced ceremony on a
    // phone into a crash.
    val stale =
        JSONObject(core.resolveEffect(id.toULong(), """{"type":"passkey_support","supported":true}"""))
    if (stale.getJSONArray("effects").length() != 0) {
        fail("re-answering a resolved effect id produced work")
    }
    if (stale.getJSONObject("view").getString("status") != "setting_up_identity") {
        fail("a stale answer moved the view")
    }

    // A malformed event is a shell bug and must be reported as one, not swallowed.
    try {
        core.dispatch("not json")
        fail("a malformed event was accepted")
    } catch (e: CoreException) {
        if (!(e.message ?: "").contains("invalid event from shell")) {
            fail("a malformed event threw the wrong thing: ${e.message}")
        }
    }

    // The other two machines must at least stand up and render.
    JSONObject(LoginCore().dispatch("""{"type":"start"}""")).getJSONArray("effects").let {
        if (it.length() != 1 || it.getJSONObject(0).getJSONObject("operation").getString("type") != "probe_index_health") {
            fail("login start did not probe the index")
        }
    }
    if (JSONObject(SessionCore().view()).getString("allowed_route") != "loading") {
        fail("a fresh session is not in `loading`")
    }

    println("smoke-kotlin: onboarding bridge green (create → login → session, correlation rules held)")
}


/**
 * The golden multi-key Safe, through this surface (spec 019 T145 / SC-003).
 *
 * SC-003 asks that a multi-key wallet created on one client resolve to a
 * CHARACTER-IDENTICAL address on the other three. The property behind that
 * requirement is not really about four devices: all four clients derive the
 * address by calling `compute_safe_address_multi` in this crate, so what has to
 * agree is the four SURFACES the one function is reached through — Rust, wasm,
 * Kotlin and Swift. Four people with four phones would be testing the same
 * function four times and calling it evidence.
 *
 * The keys are the three parallel-space fixture keys, and the address is the
 * frozen golden Safe that `src/__tests__/services/passkey-fixture.test.ts`
 * already locks on the web side. A derivation change must be a conscious edit
 * in every one of these places, never a silent drift in one.
 */
fun checkGoldenMultiKeySafe() {
    val keys = listOf(
        "04197db9030a1e166bec2cee05e0ddb94b26ee0b6d6f429f1748cda4eedac36f04fe546861a9c9dfaf75719b53c75e0b933d4aad6d325f18c75776a260d507647b",
        "047802f2cc39cc6ed85c41268a580c5f0df36df3f065facfb40f84265927b7ed678438b2084cb84f0d00708e40d44ed8c9901d979bbcb390117089847672c12eec",
        "043eb2ae2f4e8090837820048baca2db04a7e7ca7dc6742f342a30c6855e7d96947f36ac5a4538dc3a8a7cbf0deea1c0ecb80bfb96a3f8ca57c98f8f400548cd56",
    ).map { parsePublicKey(it) }
    val derived = computeSafeAddressMulti(keys).address
    if (derived != "0x88cCA0EeDbF2C4426110bbFc998F048689266894") {
        System.err.println("smoke-kotlin: golden multi-key Safe drifted - got $derived")
        System.exit(1)
    }
    println("smoke-kotlin: golden multi-key Safe agrees ($derived)")
}

fun main(args: Array<String>) {
    val vectorsDir = File(args.getOrElse(0) { "crates/vela-core/tests/vectors" })
    val failures = mutableListOf<String>()
    var total = 0

    i18nAssetDirPath = File(vectorsDir, "../../../../../assets/i18n").canonicalPath
    val files = vectorsDir.listFiles { f -> f.name.endsWith(".json") }?.sortedBy { it.name }
        ?: emptyList()
    if (files.isEmpty()) {
        System.err.println("smoke-kotlin: no vector files found in ${vectorsDir.absolutePath}")
        System.exit(1)
    }

    // Build the artwork index before dispatching anything that needs it.
    files.firstOrNull { it.name == "identicon.json" }?.let { f ->
        val cases = JSONObject(f.readText()).getJSONArray("cases")
        val re = Regex("""^section-table/(\w+)_(\d+)$""")
        for (i in 0 until cases.length()) {
            val c = cases.getJSONObject(i)
            re.find(c.getString("name"))?.let { m ->
                fragmentIndex["${m.groupValues[1]}:${c.getJSONObject("expect").getString("value")}"] =
                    m.groupValues[2].toInt()
            }
        }
        if (fragmentIndex.size != 84) {
            System.err.println("smoke-kotlin: expected 84 section-table cases, found ${fragmentIndex.size}")
            System.exit(1)
        }
    }

    val seenSuites = mutableListOf<String>()
    var skipped = 0
    for (file in files) {
        val suite = JSONObject(file.readText())
        val suiteName = suite.getString("suite")
        seenSuites.add(suiteName)

        // The bulk identicon suite uses a compact `pairs` schema and its own runner.
        if (suite.has("pairs")) {
            val pairs = suite.getJSONArray("pairs")
            for (i in 0 until pairs.length()) {
                total++
                val pair = pairs.getJSONArray(i)
                val seed = pair.getString(0)
                val want = pair.getString(1)
                val got = identiconMakeHash(seed)
                if (got != want && failures.size < 10) {
                    failures.add("$suiteName::makeHash — expected $want, got $got")
                }
            }
            continue
        }

        // The exhaustive i18n suite is COLUMNAR: {locales, keys, values}. Without
        // this branch `getJSONArray("cases")` throws on a file that has none.
        if (suite.has("values") && suite.has("keys")) {
            val locales = suite.getJSONArray("locales")
            val keys = suite.getJSONArray("keys")
            val values = suite.getJSONObject("values")
            if (locales.length() != 15) {
                failures.add("$suiteName: locale set shrank to ${locales.length()}")
            }
            for (li in 0 until locales.length()) {
                val lng = locales.getString(li)
                val column = values.optJSONArray(lng)
                if (column == null || column.length() != keys.length()) {
                    failures.add("$suiteName: column $lng is not key-aligned")
                    continue
                }
                val engine = i18nEngine(lng)
                val noOpts = i18nOpts(null)
                for (k in 0 until keys.length()) {
                    total++
                    val got = engine.t(keys.getString(k), noOpts)
                    if (got != column.getString(k) && failures.size < 20) {
                        failures.add("$suiteName::$lng::${keys.getString(k)} — expected ${column.getString(k)}, got $got")
                    }
                }
            }
            continue
        }

        val cases = suite.getJSONArray("cases")
        for (i in 0 until cases.length()) {
            total++
            val c = cases.getJSONObject(i)
            val name = c.getString("name")
            val fn = c.getString("fn")
            if (fn in CORE_ONLY_FNS) {
                total--
                skipped++
                continue
            }
            if (fn.startsWith("i18n_") && !i18nExpressible(c.optJSONObject("input")?.optJSONObject("opts"))) {
                total--
                skipped++
                i18nSkipped++
                continue
            }
            val input = c.getJSONObject("input")
            val expect = c.getJSONObject("expect")
            val label = "$suiteName::$name [$fn]"

            var actual: Any? = null
            var thrown: Throwable? = null
            try {
                actual = runCase(fn, input)
            } catch (e: Throwable) {
                thrown = e
            }

            val expectedError = if (expect.has("error")) expect.getString("error") else null
            if (expectedError != null) {
                when {
                    thrown == null -> failures.add("$label — expected error $expectedError, got $actual")
                    thrown !is CoreException ->
                        failures.add("$label — expected error $expectedError, got ${thrown.javaClass.name}: ${thrown.message}")
                    errorCode(thrown) != expectedError ->
                        failures.add("$label — expected error $expectedError, got ${errorCode(thrown)}")
                }
                continue
            }
            if (thrown != null) {
                failures.add("$label — expected success, threw ${thrown.javaClass.simpleName}: ${thrown.message}")
                continue
            }

            if (expect.has("value")) {
                if (!jsonEquals(expect.get("value"), actual)) {
                    failures.add("$label — expected ${expect.get("value")}, got $actual")
                }
            } else {
                // Field-wise object expectation (parse_public_key, compute_safe_address).
                val obj = actual as? JSONObject
                if (obj == null) {
                    failures.add("$label — expected an object result, got $actual")
                } else {
            // An expectation with no fields would pass over ANY result — the
            // corpus must never contain one, and if it does the harness has to
            // say so rather than count a case it never checked.
            val fields = expect.keys().asSequence().filter { it != "error" }.toList()
                    if (fields.isEmpty()) {
                        failures.add("$label — expectation has no fields to check")
                    }
                    for (k in fields) {
                        if (!jsonEquals(expect.get(k), obj.opt(k))) {
                            failures.add("$label — field `$k`: expected ${expect.get(k)}, got ${obj.opt(k)}")
                        }
                    }
                }
            }
        }
    }

    if (seenSuites.sorted() != REQUIRED_SUITES) {
        System.err.println("smoke-kotlin: corpus is not the expected suite set — got $seenSuites, want $REQUIRED_SUITES")
        System.exit(1)
    }
    if (failures.isNotEmpty()) {
        System.err.println("smoke-kotlin: ${failures.size} of $total cases FAILED:")
        failures.forEach { System.err.println("  $it") }
        System.exit(1)
    }
    println(
        "smoke-kotlin: $total conformance cases green through the Kotlin bindings " +
            "($skipped skipped: ${skipped - i18nSkipped} core-only functions with no " +
            "binding surface, $i18nSkipped i18n cases carrying an option the FFI " +
            "record does not model)",
    )
    // An unreported skip is how a corpus quietly stops covering things. These are
    // corpus probes of i18next's edge behaviour (returnObjects, joinArrays,
    // separator overrides, string/BigInt counts, host-only values), not surface a
    // native app calls.

    // The uniffi flat-error Display message must survive into Kotlin
    // (uniffi issue #2699 reported it being dropped) — a wrong-but-present
    // message would make every native error report useless.
    try {
        fromHex("zz")
        System.err.println("smoke-kotlin: expected fromHex(\"zz\") to throw")
        System.exit(1)
    } catch (e: CoreException.InvalidHex) {
        val msg = e.message ?: ""
        if (!msg.contains("invalid hex pair")) {
            System.err.println("smoke-kotlin: flat-error message lost — got \"$msg\"")
            System.exit(1)
        }
        println("smoke-kotlin: flat-error Display message preserved (\"$msg\")")
    }

    checkOnboardingBridge()
    checkGoldenMultiKeySafe()
}

