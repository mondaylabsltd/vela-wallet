// Replays the conformance corpus through the uniffi-generated SWIFT bindings.
//
// `cargo test` proves the Rust crate matches the corpus, verify-web.mjs proves
// the shipped wasm does, and smoke-kotlin.sh proves the Kotlin bindings do;
// this is the fourth surface — the bindings the planned native iOS app will
// consume (spec SC-001). A green cargo test with a red run here would mean the
// FFI layer, not the core, diverged.
//
// Vectors: the JSON files under rust/crates/vela-core/tests/vectors
// Schema:  specs/001-rust-core-bindings/contracts/conformance-vectors.md

import Foundation

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

func hex(_ data: Data) -> String {
    "0x" + data.map { String(format: "%02x", $0) }.joined()
}

/// Strict hex decode, mirroring `in_bytes` in conformance.rs.
///
/// It must THROW rather than substitute. Returning zeros for junk (`?? 0`) or
/// trapping on an odd length would let a case that never received its real
/// input still satisfy its expectation — the harness would report green over
/// arguments the corpus never specified. Kotlin and Rust are both strict here;
/// Swift silently substituting made 34 of 195 cases pass regardless of input.
func bytes(_ s: String) throws -> Data {
    let clean = s.hasPrefix("0x") ? String(s.dropFirst(2)) : s
    guard clean.count % 2 == 0 else { throw BadInput(detail: "odd-length hex `\(s)`") }
    var out = Data(capacity: clean.count / 2)
    var index = clean.startIndex
    while index < clean.endIndex {
        let next = clean.index(index, offsetBy: 2)
        guard let byte = UInt8(clean[index..<next], radix: 16) else {
            throw BadInput(detail: "bad hex pair `\(clean[index..<next])` in `\(s)`")
        }
        out.append(byte)
        index = next
    }
    return out
}

/// Canonical dictionary rendering of an AbiValue tree, matching the vector shape.
func abiValueToJson(_ v: AbiValue) -> [String: Any] {
    [
        "kind": v.kind,
        "name": v.name,
        "value": v.value,
        "children": v.children.map { abiValueToJson($0) },
    ]
}

/// Structural comparison — key order must NOT matter, and numbers/strings/bools
/// compare by their JSON meaning, not by Swift dynamic type.
func jsonEquals(_ a: Any?, _ b: Any?) -> Bool {
    if a == nil && b == nil { return true }
    // Exactly one side nil — an expectation naming a field the result does not
    // have (a typo'd or renamed key). This MUST report as a mismatch: the
    // force-unwrap below would trap and abort the whole run, so every case
    // after it would go unchecked while CI showed only a crash.
    if a == nil || b == nil { return false }
    if a is NSNull && b is NSNull { return true }
    if a is NSNull || b is NSNull { return false }
    if let ad = a as? [String: Any], let bd = b as? [String: Any] {
        if ad.count != bd.count { return false }
        for (k, av) in ad {
            guard let bv = bd[k], jsonEquals(av, bv) else { return false }
        }
        return true
    }
    if let aa = a as? [Any], let ba = b as? [Any] {
        if aa.count != ba.count { return false }
        for (i, av) in aa.enumerated() where !jsonEquals(av, ba[i]) { return false }
        return true
    }
    // Bools must not be compared as numbers: NSNumber bridges true to 1.
    let aIsBool = (a as? NSNumber).map { CFGetTypeID($0) == CFBooleanGetTypeID() } ?? false
    let bIsBool = (b as? NSNumber).map { CFGetTypeID($0) == CFBooleanGetTypeID() } ?? false
    if aIsBool != bIsBool { return false }
    if aIsBool { return (a as! NSNumber).boolValue == (b as! NSNumber).boolValue }
    return String(describing: a!) == String(describing: b!)
}

/// The CoreError variant name, as the corpus spells it.
func errorCode(_ error: CoreError) -> String {
    switch error {
    case .InvalidHex: return "InvalidHex"
    case .InvalidBase64Url: return "InvalidBase64Url"
    case .InvalidQuantity: return "InvalidQuantity"
    case .InvalidAddress: return "InvalidAddress"
    case .InvalidSignature: return "InvalidSignature"
    case .InvalidCbor: return "InvalidCbor"
    case .InvalidCoseKey: return "InvalidCoseKey"
    case .InvalidClientData: return "InvalidClientData"
    case .InvalidPublicKey: return "InvalidPublicKey"
    case .AbiParse: return "AbiParse"
    case .AbiDecode: return "AbiDecode"
    case .Eip712Parse: return "Eip712Parse"
    case .Eip712NonCanonicalDomain: return "Eip712NonCanonicalDomain"
    case .InvalidIdenticonSeed: return "InvalidIdenticonSeed"
    case .I18nEmptyKeyList: return "I18nEmptyKeyList"
    case .I18nInvalidCount: return "I18nInvalidCount"
    case .I18nUnsupportedOption: return "I18nUnsupportedOption"
    case .I18nCatalogUnavailable: return "I18nCatalogUnavailable"
    case .I18nCatalogParse: return "I18nCatalogParse"
    case .RegistryMetadata: return "RegistryMetadata"
    case .RegistryProof: return "RegistryProof"
    case .Internal: return "Internal"
    }
}

func errorMessage(_ error: CoreError) -> String {
    switch error {
    case .InvalidHex(let m), .InvalidBase64Url(let m), .InvalidQuantity(let m),
         .InvalidAddress(let m), .InvalidSignature(let m), .InvalidCbor(let m),
         .InvalidCoseKey(let m), .InvalidClientData(let m), .InvalidPublicKey(let m),
         .AbiParse(let m), .AbiDecode(let m), .Eip712Parse(let m),
         .Eip712NonCanonicalDomain(let m), .InvalidIdenticonSeed(let m),
         .I18nEmptyKeyList(let m), .I18nInvalidCount(let m),
         .I18nUnsupportedOption(let m), .I18nCatalogUnavailable(let m),
         .I18nCatalogParse(let m), .RegistryMetadata(let m), .RegistryProof(let m),
         .Internal(let m):
        return m
    }
}

struct NoDispatch: Error { let fn: String }
struct BadInput: Error { let detail: String }

// ---------------------------------------------------------------------------
// Dispatch — one arm per contracts/core-api.md function (mirrors conformance.rs)
// ---------------------------------------------------------------------------

/// A required string input. Missing means the corpus and this dispatch disagree
/// about the input key — a case that ran on "" would prove nothing.
func str(_ input: [String: Any], _ key: String) throws -> String {
    guard let v = input[key] as? String else { throw BadInput(detail: "missing string input `\(key)`") }
    return v
}

func data(_ input: [String: Any], _ key: String) throws -> Data { try bytes(try str(input, key)) }

func bool(_ input: [String: Any], _ key: String) throws -> Bool {
    guard let v = input[key] as? Bool else { throw BadInput(detail: "missing bool input `\(key)`") }
    return v
}

func runCase(_ fn: String, _ input: [String: Any]) throws -> Any? {
    switch fn {
    // primitives
    case "keccak256": return hex(keccak256(data: try data(input, "data")))
    case "sha256": return hex(sha256(data: try data(input, "data")))
    case "to_hex": return toHex(data: try data(input, "data"), prefixed: try bool(input, "prefixed"))
    case "from_hex": return hex(try fromHex(s: try str(input, "s")))
    case "to_quantity": return try toQuantity(value: try str(input, "value"))
    case "checksum_address": return try checksumAddress(addressHex: try str(input, "address_hex"))
    case "function_selector": return hex(try functionSelector(signature: try str(input, "signature")))
    case "create2_address":
        return try create2Address(
            deployerHex: try str(input, "deployer_hex"),
            salt: try data(input, "salt"),
            initCodeHash: try data(input, "init_code_hash"))
    case "to_base64url": return toBase64url(data: try data(input, "data"))
    case "from_base64url": return hex(try fromBase64url(s: try str(input, "s")))
    case "abi_encode_address": return hex(try abiEncodeAddress(addressHex: try str(input, "address_hex")))
    case "abi_encode_uint256": return hex(try abiEncodeUint256(valueHex: try str(input, "value_hex")))
    case "abi_encode_bytes32": return hex(try abiEncodeBytes32(data: try data(input, "data")))

    // abi
    case "canonicalize_signature": return try canonicalizeSignature(sig: try str(input, "sig"))
    case "compute_selector": return try computeSelector(sig: try str(input, "sig"))
    case "match_selector":
        return try matchSelector(sig: try str(input, "sig"), calldata: try data(input, "calldata"))
    case "decode_calldata":
        return abiValueToJson(try decodeCalldata(sig: try str(input, "sig"), calldata: try data(input, "calldata")))

    // eip712
    case "hash_typed_data": return hex(try hashTypedData(typedDataJson: try str(input, "typed_data_json")))
    case "encode_type": return try encodeType(typedDataJson: try str(input, "typed_data_json"))

    // safe
    case "parse_public_key":
        let key = try parsePublicKey(hex: try str(input, "hex"))
        return ["x": hex(key.x), "y": hex(key.y)]
    case "compute_safe_address":
        let info = try computeSafeAddress(x: try data(input, "x"), y: try data(input, "y"))
        return [
            "address": info.address,
            "salt_nonce": hex(info.saltNonce),
            "setup_data": hex(info.setupData),
            "init_code_hash": hex(info.initCodeHash),
        ]
    case "compute_safe_address_multi":
        guard let keysJson = input["keys"] as? [[String: Any]] else {
            throw BadInput(detail: "missing array input `keys`")
        }
        let keys = try keysJson.map {
            P256PublicKey(x: try data($0, "x"), y: try data($0, "y"))
        }
        let info = try computeSafeAddressMulti(keys: keys)
        return [
            "address": info.address,
            "salt_nonce": hex(info.saltNonce),
            "setup_data": hex(info.setupData),
            "init_code_hash": hex(info.initCodeHash),
        ]
    case "compute_webauthn_signer_address":
        return [
            "address": try computeWebauthnSignerAddress(
                x: try data(input, "x"),
                y: try data(input, "y")),
        ]
    case "compute_splitter_address":
        return try computeSplitterAddress(treasuryHex: try str(input, "treasury_hex"))
    case "encode_splitter_deploy_call":
        return hex(try encodeSplitterDeployCall(treasuryHex: try str(input, "treasury_hex")))
    case "safe_proxy_runtime_code": return try safeProxyRuntimeCode()

    // webauthn
    case "extract_attestation_public_key":
        let key = try extractAttestationPublicKey(attestationObject: try data(input, "attestation_object"))
        return ["x": hex(key.x), "y": hex(key.y)]
    case "der_signature_to_raw_low_s":
        return hex(try derSignatureToRawLowS(der: try data(input, "der")))
    case "validate_client_data":
        // An unrecognised kind must fail, not silently fall through to .get —
        // that would run every Create case against the Get rules.
        let kindName = try str(input, "kind")
        let kind: ClientDataKind
        switch kindName {
        case "Create": kind = .create
        case "Get": kind = .get
        default: throw BadInput(detail: "unknown ClientDataKind `\(kindName)`")
        }
        try validateClientData(
            kind: kind,
            clientDataJson: try data(input, "client_data_json"),
            authenticatorData: try data(input, "authenticator_data"))
        return true
    case "webauthn_signing_hash":
        return hex(webauthnSigningHash(
            authenticatorData: try data(input, "authenticator_data"),
            clientDataJson: try data(input, "client_data_json")))
    case "recover_public_key_from_assertions":
        guard let a = input["a"] as? [String: Any] else { throw BadInput(detail: "missing input `a`") }
        guard let b = input["b"] as? [String: Any] else { throw BadInput(detail: "missing input `b`") }
        let key = try recoverPublicKeyFromAssertions(
            a: WebAuthnAssertion(
                authenticatorData: try data(a, "authenticator_data"),
                clientDataJson: try data(a, "client_data_json"),
                signatureDer: try data(a, "signature_der")),
            b: WebAuthnAssertion(
                authenticatorData: try data(b, "authenticator_data"),
                clientDataJson: try data(b, "client_data_json"),
                signatureDer: try data(b, "signature_der")))
        guard let key else { return NSNull() }
        return "04" + String(hex(key.x).dropFirst(2)) + String(hex(key.y).dropFirst(2))

    // identicon (specs/003-rust-identicon). Params expectations carry section
    // INDICES; `sectionIndex` resolves the returned artwork back to one using the
    // table pinned by the corpus's own `section-table` group, so nothing here is
    // circular — both ends are anchored to the identicons-esm oracle.
    case "make_hash":
        return identiconMakeHash(seed: try str(input, "seed"))
    case "identicon_svg":
        return try identiconSvg(seed: try str(input, "seed"))
    case "identicon_svg_circular":
        return try identiconSvgCircular(seed: try str(input, "seed"))
    case "identicon_data_uri":
        return try identiconDataUri(seed: try str(input, "seed"))
    case "normalize_seed":
        return identiconNormalizeSeed(seed: try str(input, "seed"))
    case "identicon_params":
        let p = try identiconParams(seed: try str(input, "seed"))
        return [
            "main": p.main,
            "background": p.background,
            "accent": p.accent,
            "face": sectionIndex("face", p.face),
            "top": sectionIndex("top", p.top),
            "sides": sectionIndex("sides", p.sides),
            "bottom": sectionIndex("bottom", p.bottom),
        ] as [String: Any]

    // --- i18n (spec 004-rust-i18n) ---
    //
    // Catalogs arrive as JSON bytes, the same on-demand route the web build uses.
    // That means this surface exercises `Values::Owned` while the Rust suite
    // exercises `Values::Static`, so a divergence between the two representations
    // shows up here rather than in production.
    case "i18n_t":
        return try i18nEngine(lng: input["lng"] as? String ?? "en")
            .t(key: anyKey(input, "key"), opts: i18nOpts(input["opts"]))
    case "i18n_t_keys":
        let keys = (input["keys"] as? [Any] ?? []).map { anyString($0) }
        return try i18nEngine(lng: input["lng"] as? String ?? "en")
            .tFirst(keys: keys, opts: i18nOpts(input["opts"]))
    case "i18n_t_lng_option":
        // The per-call `lng` path: the ACTIVE language stays `en` while resolution
        // runs against the tag in the options. Two different upstream functions.
        let target = canonicalTag((input["opts"] as? [String: Any])?["lng"] as? String ?? "en")
        return try i18nEngine(lng: target, active: "en")
            .t(key: anyKey(input, "key"), opts: i18nOpts(input["opts"]))
    case "i18n_t_legacy_plural":
        return try i18nEngine(lng: input["lng"] as? String ?? "en", legacyPlurals: true)
            .t(key: anyKey(input, "key"), opts: i18nOpts(input["opts"]))
    case "i18n_interpolate":
        return try i18nInterpolate(template: try str(input, "template"), opts: i18nOpts(input["opts"]))
    case "i18n_plural_suffix":
        return i18nPluralSuffix(locale: try str(input, "lng"), count: i18nCount(input))
    case "i18n_plural_suffixes":
        return i18nPluralSuffixes(locale: try str(input, "lng"))
    case "i18n_plural_suffix_legacy":
        return i18nPluralSuffixLegacy(count: i18nCount(input))
    case "i18n_plural_suffixes_legacy":
        return i18nPluralSuffixesLegacy()
    case "i18n_resolve_language", "i18n_change_language":
        let st = try i18nEngine(lng: "en").changeLanguage(lng: try str(input, "requested"))
        return [
            "language": st.language,
            // Unwrap: boxing an Optional into `Any` compares as `Optional("zh")`.
            "resolved_language": st.resolvedLanguage.map { $0 as Any } ?? NSNull(),
            "languages": st.languages,
        ] as [String: Any]
    default: throw NoDispatch(fn: fn)
    }
}

// ---------------------------------------------------------------------------
// i18n helpers
// ---------------------------------------------------------------------------

/// i18next only canonicalises tags containing `-`, so `zh-tw` becomes `zh-TW`
/// while a bare `ZH` does not. The asset map is keyed by the canonical form.
func canonicalTag(_ t: String) -> String {
    guard t.contains("-") else { return t }
    return t.split(separator: "-").enumerated().map { i, p -> String in
        if i == 0 { return p.lowercased() }
        if p.count == 4 { return p.prefix(1).uppercased() + p.dropFirst().lowercased() }
        if p.count == 2 { return p.uppercased() }
        return p.lowercased()
    }.joined(separator: "-")
}

var i18nSkippedCases: [String] = []
let i18nLocales = ["en", "zh", "zh-TW", "zh-HK", "ja", "ko", "vi", "id", "tr",
                   "es-MX", "pt-BR", "fr", "de", "ru", "it"]
// COMPUTED, not a stored global: Swift initialises top-level bindings in source
// order, and `vectorsPath` is declared further down. A stored `let` here read it
// before its initialiser had run — which is undefined behaviour, and crashed the
// harness with SIGSEGV rather than anything diagnosable.
var i18nAssetDir: String { "\(vectorsPath)/../../../../../assets/i18n" }
var i18nAssets: [String: Data] = [:]

func i18nAsset(_ lng: String) -> Data? {
    if let cached = i18nAssets[lng] { return cached }
    guard let d = try? Data(contentsOf: URL(fileURLWithPath: "\(i18nAssetDir)/\(lng).json")) else {
        return nil
    }
    i18nAssets[lng] = d
    return d
}

func i18nEngine(lng: String, active: String? = nil, legacyPlurals: Bool = false) throws -> I18n {
    guard let en = i18nAsset("en") else { throw NoDispatch(fn: "i18n:no en asset") }
    let engine = legacyPlurals ? try I18n.newWithLegacyPlurals(fallbackJson: en)
                               : try I18n(fallbackJson: en)
    let tag = canonicalTag(lng)
    if tag != "en", let bytes = i18nAsset(tag) {
        try engine.loadCatalog(lang: tag, json: bytes)
    }
    _ = try engine.changeLanguage(lng: active ?? tag)
    return engine
}

/// i18next `String()`-coerces a non-string key, so a numeric or null key is a
/// legitimate lookup rather than a malformed vector.
func anyString(_ v: Any?) -> String {
    switch v {
    case let s as String: return s
    case nil, is NSNull: return "null"
    case let n as NSNumber:
        // `JSONSerialization` boxes booleans as `NSNumber`, so `stringValue`
        // renders them "0"/"1". JS renders "false"/"true", and the corpus expects
        // JS. `CFBooleanGetTypeID` is the only reliable way to tell them apart.
        if CFGetTypeID(n) == CFBooleanGetTypeID() {
            return n.boolValue ? "true" : "false"
        }
        return n.stringValue
    default: return String(describing: v!)
    }
}

func i18nCount(_ input: [String: Any]) -> Double {
    (input["count"] as? NSNumber)?.doubleValue ?? Double.nan
}

func anyKey(_ input: [String: Any], _ key: String) -> String {
    anyString(input[key])
}

/// Option keys the uniffi record deliberately does NOT model.
///
/// The FFI surface carries what a native app actually passes — `count`, `context`,
/// `defaultValue`, `lng`, `ordinal` and interpolation variables. The rest are
/// corpus probes of i18next's edge behaviour (`returnObjects`, `joinArrays`,
/// separator overrides, a string or BigInt `count`, host-only values that
/// stringify through JS semantics). Modelling them would widen a wallet's FFI
/// contract to serve a test.
///
/// Cases carrying one are SKIPPED — counted and printed, never silent, because an
/// unreported skip is how a corpus quietly stops covering things.
let i18nUnmodelledOptions: Set<String> = [
    "returnObjects", "returnDetails", "joinArrays", "keySeparator", "nsSeparator",
    "ns", "replace",
]

func i18nExpressible(_ raw: Any?) -> Bool {
    guard let o = raw as? [String: Any] else { return true }
    for (k, v) in o {
        if i18nUnmodelledOptions.contains(k) { return false }
        if k.hasPrefix("defaultValue_") { return false }
        // A tagged encoding for a value JSON cannot carry.
        if let tagged = v as? [String: Any], tagged["__t"] != nil { return false }
        // A string or object `count` is a shape the typed record cannot express.
        if k == "count", !(v is NSNumber) { return false }
        if k == "defaultValue", !(v is String) { return false }
        // A nested object variable feeds `{{a.b}}`, which needs dotted flattening.
        if v is [String: Any] { return false }
        if v is [Any] { return false }
    }
    return true
}

func i18nOpts(_ raw: Any?) -> TOptions {
    guard let o = raw as? [String: Any] else {
        return TOptions(count: nil, context: nil, defaultValue: nil, lng: nil, ordinal: false, vars: [])
    }
    var count: Double?
    var context: String?
    var defaultValue: String?
    var lng: String?
    var ordinal = false
    var vars: [TVar] = []
    for (k, v) in o {
        switch k {
        case "count": if let n = v as? NSNumber { count = n.doubleValue }
        case "context": context = anyString(v)
        case "defaultValue": if let s = v as? String { defaultValue = s }
        case "lng": lng = v as? String
        case "ordinal": ordinal = (v as? Bool) ?? false
        default: vars.append(TVar(name: k, value: v is NSNull ? nil : anyString(v)))
        }
    }
    return TOptions(count: count, context: context, defaultValue: defaultValue,
                    lng: lng, ordinal: ordinal, vars: vars)
}

/// Artwork -> 1-based index, built from the corpus's own `section-table` cases so
/// the compact index form used by every params expectation can be checked here too.
var fragmentIndex: [String: Int] = [:]

func sectionIndex(_ section: String, _ svg: String) -> Any {
    fragmentIndex["\(section):\(svg)"] ?? NSNull()
}

// ---------------------------------------------------------------------------

let vectorsPath = CommandLine.arguments.count > 1
    ? CommandLine.arguments[1]
    : "crates/vela-core/tests/vectors"

let fileManager = FileManager.default
guard let names = try? fileManager.contentsOfDirectory(atPath: vectorsPath) else {
    FileHandle.standardError.write("smoke-swift: cannot read \(vectorsPath)\n".data(using: .utf8)!)
    exit(1)
}
let vectorFiles = names.filter { $0.hasSuffix(".json") }.sorted()
if vectorFiles.isEmpty {
    FileHandle.standardError.write("smoke-swift: no vector files in \(vectorsPath)\n".data(using: .utf8)!)
    exit(1)
}

/// The corpus is five suites, discovered by scanning the directory. Asserting the
/// exact set is what stops a vector file lost to a bad merge or a partial checkout
/// from making this harness report "green" over a corpus that silently shrank —
/// the precise false confidence this feature exists to prevent.
let REQUIRED_SUITES = [
    "abi", "eip712",
    // `i18n-*` sorts before `identicon`: '1' is 0x31, 'd' is 0x64.
    "i18n-behaviour", "i18n-exhaustive", "i18n-plural", "i18n-plural-legacy",
    "identicon", "identicon-bulk", "primitives", "safe", "safe-multi", "webauthn",
]

/// Functions that exist in vela-core but are deliberately NOT on any binding surface
/// (specs/003-rust-identicon contracts/identicon-api.md): a test-only parity device,
/// plus helpers no Vela platform calls. Skipping is counted and reported — an
/// unreported skip is how a corpus quietly stops covering things.
let CORE_ONLY_FNS: Set<String> = [
    "identicon_params_js_compat", "section_svg", "create_identicon",
    "nimiq_is_valid_address", "constants",
]

var total = 0
var skipped = 0
var failures: [String] = []
var seenSuites: [String] = []

// Build the artwork index before dispatching anything that needs it.
if vectorFiles.contains("identicon.json"),
   let raw = try? Data(contentsOf: URL(fileURLWithPath: "\(vectorsPath)/identicon.json")),
   let doc = try? JSONSerialization.jsonObject(with: raw) as? [String: Any],
   let cases = doc["cases"] as? [[String: Any]] {
    for c in cases {
        guard let name = c["name"] as? String,
              name.hasPrefix("section-table/"),
              let expect = c["expect"] as? [String: Any],
              let value = expect["value"] as? String else { continue }
        let tail = name.dropFirst("section-table/".count)
        guard let sep = tail.lastIndex(of: "_"), let idx = Int(tail[tail.index(after: sep)...]) else { continue }
        fragmentIndex["\(tail[..<sep]):\(value)"] = idx
    }
    if fragmentIndex.count != 84 {
        FileHandle.standardError.write(
            "smoke-swift: expected 84 section-table cases, found \(fragmentIndex.count)\n".data(using: .utf8)!
        )
        exit(1)
    }
}

for name in vectorFiles {
    let raw = try Data(contentsOf: URL(fileURLWithPath: "\(vectorsPath)/\(name)"))
    guard let suite = try JSONSerialization.jsonObject(with: raw) as? [String: Any],
          let suiteName = suite["suite"] as? String else {
        failures.append("\(name) — bad schema")
        continue
    }
    seenSuites.append(suiteName)

    // The bulk identicon suite uses a compact `pairs` schema and its own runner.
    if let pairs = suite["pairs"] as? [[String]] {
        for pair in pairs where pair.count == 2 {
            total += 1
            let got = identiconMakeHash(seed: pair[0])
            if got != pair[1], failures.count < 10 {
                failures.append("\(suiteName)::makeHash — expected \(pair[1]), got \(got)")
            }
        }
        continue
    }

    // The exhaustive i18n suite is COLUMNAR: {locales, keys, values}. Without this
    // branch it hits the `cases` guard below and reports a schema failure — which
    // is exactly what happened the moment the vectors landed, and is the intended
    // order of events: a new schema must break the harness rather than pass through
    // it silently.
    if let locales = suite["locales"] as? [String],
       let keys = suite["keys"] as? [String],
       let values = suite["values"] as? [String: [String]] {
        if locales.count != 15 {
            failures.append("\(suiteName): locale set shrank to \(locales.count)")
        }
        for lng in locales {
            guard let column = values[lng], column.count == keys.count else {
                failures.append("\(suiteName): column \(lng) is not key-aligned")
                continue
            }
            guard let engine = try? i18nEngine(lng: lng) else {
                failures.append("\(suiteName): no engine for \(lng)")
                continue
            }
            for (k, key) in keys.enumerated() {
                total += 1
                let got = (try? engine.t(key: key, opts: i18nOpts(nil))) ?? "<err>"
                if got != column[k], failures.count < 20 {
                    failures.append("\(suiteName)::\(lng)::\(key) — expected \(column[k]), got \(got)")
                }
            }
        }
        continue
    }

    guard let cases = suite["cases"] as? [[String: Any]] else {
        failures.append("\(name) — bad schema (no cases and no pairs)")
        continue
    }
    for c in cases {
        let caseName = c["name"] as? String ?? "?"
        let fn = c["fn"] as? String ?? "?"
        if CORE_ONLY_FNS.contains(fn) {
            skipped += 1
            continue
        }
        if fn.hasPrefix("i18n_"), !i18nExpressible(c["input"].flatMap { ($0 as? [String: Any])?["opts"] }) {
            skipped += 1
            i18nSkippedCases.append(caseName)
            continue
        }
        total += 1
        let input = c["input"] as? [String: Any] ?? [:]
        let expect = c["expect"] as? [String: Any] ?? [:]
        let label = "\(suiteName)::\(caseName) [\(fn)]"

        var actual: Any?
        var thrown: Error?
        do {
            actual = try runCase(fn, input)
        } catch {
            thrown = error
        }

        if let expectedError = expect["error"] as? String {
            switch thrown {
            case nil:
                failures.append("\(label) — expected error \(expectedError), got \(actual ?? "nil")")
            case let e as CoreError where errorCode(e) != expectedError:
                failures.append("\(label) — expected error \(expectedError), got \(errorCode(e))")
            case let e as CoreError:
                _ = e // matched
            case let e?:
                failures.append("\(label) — expected error \(expectedError), got \(e)")
            }
            continue
        }
        if let e = thrown {
            failures.append("\(label) — expected success, threw \(e)")
            continue
        }

        if let want = expect["value"] {
            if !jsonEquals(want, actual) {
                failures.append("\(label) — expected \(want), got \(actual ?? "nil")")
            }
        } else {
            guard let obj = actual as? [String: Any] else {
                failures.append("\(label) — expected an object result, got \(actual ?? "nil")")
                continue
            }
            // An expectation with no fields would pass over ANY result.
            let fields = expect.filter { $0.key != "error" }
            if fields.isEmpty {
                failures.append("\(label) — expectation has no fields to check")
            }
            for (k, want) in fields {
                if !jsonEquals(want, obj[k]) {
                    failures.append("\(label) — field `\(k)`: expected \(want), got \(obj[k] ?? "nil")")
                }
            }
        }
    }
}

if seenSuites.sorted() != REQUIRED_SUITES {
    FileHandle.standardError.write(
        "smoke-swift: corpus is not the expected suite set — got \(seenSuites.sorted()), want \(REQUIRED_SUITES)\n"
            .data(using: .utf8)!)
    exit(1)
}
if !failures.isEmpty {
    var out = "smoke-swift: \(failures.count) of \(total) cases FAILED:\n"
    for f in failures { out += "  \(f)\n" }
    FileHandle.standardError.write(out.data(using: .utf8)!)
    exit(1)
}
print(
    "smoke-swift: \(total) conformance cases green through the Swift bindings "
        + "(\(skipped) skipped: \(skipped - i18nSkippedCases.count) core-only functions "
        + "with no binding surface, \(i18nSkippedCases.count) i18n cases carrying an "
        + "option the FFI record does not model)"
)
// Naming them is the point: an unreported skip is how a corpus quietly stops
// covering things. These are corpus probes of i18next's edge behaviour
// (returnObjects, joinArrays, separator overrides, string/BigInt counts,
// host-only values), not surface a native app calls.
if !i18nSkippedCases.isEmpty {
    print("smoke-swift:   i18n skips — " + i18nSkippedCases.sorted().joined(separator: ", "))
}

// The recursive AbiValue must survive the boundary with its nesting intact —
// uniffi 0.32's cycle detection is what makes this type expressible at all.
let nested = try decodeCalldata(
    sig: "exactInput((bytes path,address recipient,uint256 amountIn,uint256 amountOutMinimum) params)",
    calldata: bytes(
        "0xb858183f"
        + "0000000000000000000000000000000000000000000000000000000000000020"
        + "0000000000000000000000000000000000000000000000000000000000000080"
        + "000000000000000000000000d8da6bf26964af9d7eed9e03e53415d37aa96045"
        + "00000000000000000000000000000000000000000000000000000000000f4240"
        + "00000000000000000000000000000000000000000000000000000000000003e7"
        + "0000000000000000000000000000000000000000000000000000000000000006"
        + "aabbccddeeff0000000000000000000000000000000000000000000000000000"))
guard nested.children.count == 1, nested.children[0].children.count == 4 else {
    FileHandle.standardError.write("smoke-swift: recursive AbiValue lost its nesting\n".data(using: .utf8)!)
    exit(1)
}
print("smoke-swift: recursive AbiValue survives the boundary (tuple with 4 fields)")

// The uniffi flat-error message must reach Swift intact — a wrong-but-present
// message would make every native error report useless.
do {
    _ = try fromHex(s: "zz")
    FileHandle.standardError.write("smoke-swift: expected fromHex(\"zz\") to throw\n".data(using: .utf8)!)
    exit(1)
} catch let e as CoreError {
    let msg = errorMessage(e)
    guard msg.contains("invalid hex pair") else {
        FileHandle.standardError.write("smoke-swift: flat-error message lost — got \"\(msg)\"\n".data(using: .utf8)!)
        exit(1)
    }
    print("smoke-swift: flat-error message preserved (\"\(msg)\")")
}

// ---------------------------------------------------------------------------
// The Crux bridge (spec 019, T100)
// ---------------------------------------------------------------------------
//
// The corpus above proves the PURE functions agree across surfaces. It says
// nothing about the state machines, which arrived on this crate in spec 019 and
// are what the iOS app actually runs. This is the narrow claim the smoke can
// make without a passkey provider: the bridge accepts an event as JSON,
// advances the machine, hands back the effect the core is waiting on, and
// honours the correlation rules.
//
// It deliberately stops before `register_passkey` — performing that needs
// AuthenticationServices and a presentation anchor, neither of which exists in
// a command-line harness.

func bridgeFail(_ why: String) -> Never {
    FileHandle.standardError.write("smoke-swift: onboarding bridge — \(why)\n".data(using: .utf8)!)
    exit(1)
}

func bridgeObject(_ json: String) -> [String: Any] {
    guard let data = json.data(using: .utf8),
          let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
    else { bridgeFail("the bridge returned something that is not a JSON object: \(json)") }
    return object
}

do {
    let core = CreateWalletCore()

    // The form is pure model: it must advance with no effect at all.
    _ = try core.dispatch(eventJson: #"{"type":"start"}"#)
    _ = try core.dispatch(eventJson: #"{"type":"name_changed","name":"Ada"}"#)
    _ = try core.dispatch(eventJson: #"{"type":"ack_toggled","index":0}"#)
    _ = try core.dispatch(eventJson: #"{"type":"ack_toggled","index":1}"#)
    // ACK_COUNT is 3 since the privacy/terms ack joined the form.
    let acked = bridgeObject(try core.dispatch(eventJson: #"{"type":"ack_toggled","index":2}"#))
    guard let ackedEffects = acked["effects"] as? [Any], ackedEffects.isEmpty else {
        bridgeFail("filling the form asked the shell to do something")
    }
    guard let ackedView = acked["view"] as? [String: Any],
          ackedView["can_submit"] as? Bool == true
    else { bridgeFail("a named, fully-acked form is not submittable") }

    // Submitting is where the shell is first asked for anything.
    let submitted = bridgeObject(try core.dispatch(eventJson: #"{"type":"submit"}"#))
    guard let effects = submitted["effects"] as? [[String: Any]], effects.count == 1 else {
        bridgeFail("submit did not produce exactly one effect")
    }
    guard let rawId = effects[0]["id"] as? NSNumber, rawId.uint64Value == 1 else {
        bridgeFail("effect ids are not monotonic from 1")
    }
    let effectId = rawId.uint64Value
    guard let operation = effects[0]["operation"] as? [String: Any],
          operation["type"] as? String == "check_passkey_support"
    else { bridgeFail("submit asked for the wrong operation") }

    // Answering it advances the machine and asks for the next thing.
    let supported = bridgeObject(
        try core.resolveEffect(
            effectId: effectId, resultJson: #"{"type":"passkey_support","supported":true}"#))
    guard let next = supported["effects"] as? [[String: Any]], next.count == 1,
          let nextOp = next[0]["operation"] as? [String: Any],
          nextOp["type"] as? String == "generate_group_key"
    else { bridgeFail("answering passkey support did not lead to `generate_group_key`") }

    // Rule 2: an answer that outlived its question changes nothing and is not
    // an error. A bridge that threw here would turn every raced ceremony on a
    // phone into a crash.
    let stale = bridgeObject(
        try core.resolveEffect(
            effectId: effectId, resultJson: #"{"type":"passkey_support","supported":true}"#))
    guard let staleEffects = stale["effects"] as? [Any], staleEffects.isEmpty else {
        bridgeFail("re-answering a resolved effect id produced work")
    }
    guard let staleView = stale["view"] as? [String: Any],
          staleView["status"] as? String == "setting_up_identity"
    else { bridgeFail("a stale answer moved the view") }

    // A malformed event is a shell bug and must be reported as one, not swallowed.
    do {
        _ = try core.dispatch(eventJson: "not json")
        bridgeFail("a malformed event was accepted")
    } catch let error as CoreError {
        guard errorMessage(error).contains("invalid event from shell") else {
            bridgeFail("a malformed event threw the wrong thing: \(errorMessage(error))")
        }
    }

    // The other two machines must at least stand up and render.
    let login = bridgeObject(try LoginCore().dispatch(eventJson: #"{"type":"start"}"#))
    guard let loginEffects = login["effects"] as? [[String: Any]], loginEffects.count == 1,
          let loginOp = loginEffects[0]["operation"] as? [String: Any],
          loginOp["type"] as? String == "probe_index_health"
    else { bridgeFail("login start did not probe the index") }

    let session = bridgeObject(try SessionCore().view())
    guard session["allowed_route"] as? String == "loading" else {
        bridgeFail("a fresh session is not in `loading`")
    }

    print("smoke-swift: onboarding bridge green (create → login → session, correlation rules held)")
} catch {
    bridgeFail("threw: \(error)")
}

// ---------------------------------------------------------------------------
// The golden multi-key Safe (spec 019 T145 / SC-003)
// ---------------------------------------------------------------------------
//
// SC-003 asks that a multi-key wallet created on one client resolve to a
// CHARACTER-IDENTICAL address on the other three. The property behind that
// requirement is not really about four devices: all four clients derive the
// address by calling `compute_safe_address_multi` in this crate, so what has to
// agree is the four SURFACES the one function is reached through — Rust, wasm,
// Kotlin and Swift. Four people with four phones would be testing the same
// function four times and calling it evidence.
//
// The keys are the three parallel-space fixture keys, and the address is the
// frozen golden Safe that `src/__tests__/services/passkey-fixture.test.ts`
// already locks on the web side.

do {
    let keys = try [
        "04197db9030a1e166bec2cee05e0ddb94b26ee0b6d6f429f1748cda4eedac36f04fe546861a9c9dfaf75719b53c75e0b933d4aad6d325f18c75776a260d507647b",
        "047802f2cc39cc6ed85c41268a580c5f0df36df3f065facfb40f84265927b7ed678438b2084cb84f0d00708e40d44ed8c9901d979bbcb390117089847672c12eec",
        "043eb2ae2f4e8090837820048baca2db04a7e7ca7dc6742f342a30c6855e7d96947f36ac5a4538dc3a8a7cbf0deea1c0ecb80bfb96a3f8ca57c98f8f400548cd56",
    ].map { try parsePublicKey(hex: $0) }
    let derived = try computeSafeAddressMulti(keys: keys).address
    guard derived == "0x88cCA0EeDbF2C4426110bbFc998F048689266894" else {
        FileHandle.standardError.write(
            "smoke-swift: golden multi-key Safe drifted — got \(derived)\n".data(using: .utf8)!)
        exit(1)
    }
    print("smoke-swift: golden multi-key Safe agrees (\(derived))")
} catch {
    FileHandle.standardError.write(
        "smoke-swift: golden multi-key Safe threw — \(error)\n".data(using: .utf8)!)
    exit(1)
}
