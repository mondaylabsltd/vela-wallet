/* tslint:disable */
/* eslint-disable */
/**
 * A `count` as it arrives from JS.
 *
 * `serde_json::Value` cannot hold `Infinity` or `NaN` — JSON has no syntax for
 * them, so `serde_wasm_bindgen` turns both into `null`. That silently rendered
 * `{{count}}` as the empty string where i18next renders `\"Infinity\"`. The
 * committed corpus never caught it, because it encodes those values with a
 * `{\"__t\":\"infinity\"}` tag and so never exercises the raw-number path a real
 * caller takes. `scripts/verify-i18n-parity.mjs`\'s fuzz pass did.
 *
 * Untagged, with `f64` FIRST: a JS number deserialises straight into `f64`,
 * non-finite values included, before the `Value` arm can flatten it.
 */
export type CountValue = number | string | Value;

/**
 * An interpolation variable as it arrives from JS.
 *
 * Same defect as `CountValue`, same device — and it took a second sighting to
 * notice the fix had been applied to `count` alone. Every OTHER variable still
 * went through `serde_json::Value`, so `t(\'time.minutesShort\', { n: NaN })`
 * rendered `\"分前\"` where i18next renders `\"NaN分前\"`. That one is reachable in
 * production: `src/services/activity.ts:116` passes `{ n: Math.round(diff / 60) }`.
 *
 * The corpus cannot catch this class at all — it encodes non-finite values as
 * `{\"__t\":\"nan\"}` and decodes the tag back on the Rust side, so a vector never
 * crosses the raw-number boundary a live caller crosses (spec 005 FR-024).
 */
export type VarValue = number | Value;

/**
 * Flattened `IdenticonParams` — the same shape `getIdenticonsParams` returns in
 * the JS library, so migrating call sites stay recognisable.
 */
export interface IdenticonParams {
    main: string;
    background: string;
    accent: string;
    top: string;
    sides: string;
    face: string;
    bottom: string;
}

/**
 * Input for `encodeRegistryMetadata`; `version` is supplied by the core.
 */
export interface RegistryMetadataInput {
    address: string;
    walletVersion: string;
    keyNames: string[];
    createdAtIso: string;
}

/**
 * Mirror of `registry_proof::GroupProof`.
 */
export interface GroupProofJs {
    groupPublicKeyHex: string;
    proof: RegistryProofJs;
}

/**
 * Mirror of `registry_proof::RegistryProof` — the WebAuthn-shaped proof the
 * registry contract verifies.
 */
export interface RegistryProofJs {
    authenticatorData: string;
    clientDataJSON: string;
    challengeIndex: number;
    typeIndex: number;
    r: string;
    s: string;
}

/**
 * One stable\'s DEX quotes for 1 native coin, as the shell decodes them out of
 * the multicall. Each stable is its own group because its own `decimals()`
 * normalizes the amount — USDC (6) and DAI (18) must never be compared under
 * one shared scale.
 */
export interface NativeQuoteGroup {
    /**
     * Successful quote outputs in THIS stable\'s base units, as decimal
     * strings (failed calls are simply absent).
     */
    amountsOut: string[];
    /**
     * This stable\'s `decimals()` read; `null` = the read failed, and the core
     * applies its own `DEFAULT_QUOTE_DECIMALS`.
     */
    quoteDecimals: number | undefined;
}

/**
 * Per-call translation options, shaped so a TS caller writes the i18next object
 * literal verbatim — `{ count: 3, name: \'Alice\' }`. The reserved names are typed;
 * everything else falls into `vars` through `#[serde(flatten)]`.
 */
export interface TOptions extends Map<string, VarValue> {
    /**
     * Untyped: i18next accepts a number, a string (which silently DISABLES plural
     * handling), `null`, an object, or a BigInt (which makes it throw). Typing
     * this as `f64` would reject inputs the oracle accepts.
     *
     * Double `Option` because `Option<Value>` collapses an explicit JSON `null`
     * into `None`, which would make `count: null` indistinguishable from an absent
     * count — and upstream those differ: `null` still pluralises (`Number(null)`
     * is 0), while absent does not.
     */
    count?: CountValue | undefined | undefined;
    /**
     * Untyped for the same reason — a numeric context is coerced, not rejected.
     */
    context?: Value | undefined;
    /**
     * Untyped because i18next accepts a string, a number, a boolean, an object
     * or an array here, and the last two are non-strings this engine rejects
     * rather than approximates.
     */
    defaultValue?: Value | undefined;
    /**
     * Per-call language override. **Not** `changeLanguage`: `zh_TW` resolves to
     * `zh` there and falls through to English here.
     */
    lng?: string | undefined;
    ordinal?: boolean;
    /**
     * Per-call namespace override. Anything but `translation` misses.
     */
    ns?: string | undefined;
    /**
     * `keySeparator: false` — look the key up as ONE literal property.
     */
    keySeparator?: Value | undefined;
    /**
     * `nsSeparator: false` — a `:` in the key is not a namespace separator.
     */
    nsSeparator?: Value | undefined;
    /**
     * When present and an object, `replace` REPLACES the options as the
     * interpolation source (`i18next.js:1180`) — a top-level `v` is shadowed
     * rather than merged.
     */
    replace?: ReplaceArg | undefined;
    /**
     * Options i18next answers with a NON-string. A Rust `t()` is string-typed by
     * construction, so these are typed errors, not silent coercions.
     */
    returnObjects?: boolean | undefined;
    returnDetails?: boolean | undefined;
    joinArrays?: Value | undefined;
}

/**
 * The chosen price and the rung of the ladder it came from. `source` is the
 * `NativePriceSource` variant name; `\"none\"` when nothing could price.
 */
export interface NativePriceChoice {
    price: number | undefined;
    source: string;
}

/**
 * The resolve state after a language change.
 */
export interface LanguageState {
    language: string;
    resolvedLanguage: string | undefined;
    languages: string[];
}

/**
 * Wrapper so the group list crosses the boundary as one value.
 */
export interface NativeQuoteGroups {
    groups: NativeQuoteGroup[];
}

/**
 * `replace`, which when it is an object REPLACES the options as the
 * interpolation source (`i18next.js:1180`). Typed as a map of [`VarValue`] so a
 * non-finite value survives that route too — the 005 adapter deliberately routes
 * through `replace` when normalising an own-but-undefined `count`.
 */
export type ReplaceArg = Map<string, VarValue> | Value;

export interface AbiValue {
    kind: string;
    name: string;
    value: string;
    children: AbiValue[];
}

export interface CoreErrorJs {
    code: string;
    message: string;
}

export interface P256PublicKey {
    /**
     * 0x-hex, 32 bytes.
     */
    x: string;
    /**
     * 0x-hex, 32 bytes.
     */
    y: string;
}

export interface SafeAddressInfo {
    address: string;
    /**
     * 0x-hex, 32 bytes.
     */
    salt_nonce: string;
    /**
     * 0x-hex.
     */
    setup_data: string;
    /**
     * 0x-hex, 32 bytes.
     */
    init_code_hash: string;
}


/**
 * r" The activity feed: dedupe, batch folding, tombstones, celebrations.
 */
export class ActivityFeedCore {
    free(): void;
    [Symbol.dispose](): void;
    dispatch(event_json: string): string;
    constructor();
    resolve_effect(effect_id: bigint, result_json: string): string;
    view(): string;
}

/**
 * r" Never-unlimited approval guard and allowance editor.
 */
export class ApprovalGuardCore {
    free(): void;
    [Symbol.dispose](): void;
    dispatch(event_json: string): string;
    constructor();
    resolve_effect(effect_id: bigint, result_json: string): string;
    view(): string;
}

/**
 * r" Balance aggregation & display policy (per active account).
 */
export class BalanceDashboardCore {
    free(): void;
    [Symbol.dispose](): void;
    dispatch(event_json: string): string;
    constructor();
    resolve_effect(effect_id: bigint, result_json: string): string;
    view(): string;
}

/**
 * r" Payroll batch import: table interpretation, fiat conversion, caps.
 */
export class BatchImportCore {
    free(): void;
    [Symbol.dispose](): void;
    dispatch(event_json: string): string;
    constructor();
    resolve_effect(effect_id: bigint, result_json: string): string;
    view(): string;
}

/**
 * r" Browser history policy.
 */
export class BrowserHistoryCore {
    free(): void;
    [Symbol.dispose](): void;
    dispatch(event_json: string): string;
    constructor();
    resolve_effect(effect_id: bigint, result_json: string): string;
    view(): string;
}

/**
 * One loopback WebSocket connection, exactly as the phones run it.
 */
export class ClearSignerWs {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * The socket closed: `"declined"` when a page that had the request went
     * away, `""` when it never proved itself.
     */
    closed(): string;
    /**
     * `{write: number[], close, outcome?}` — `outcome` as [`clear_signer_verify`].
     */
    feed(bytes: Uint8Array): string;
    constructor(signer_url: string, token: string, id: string, request_json: string, digest: Uint8Array, keys_json: string);
}

/**
 * r" Clear-signing resolution pipeline and message risk verdicts.
 */
export class ClearSigningCore {
    free(): void;
    [Symbol.dispose](): void;
    dispatch(event_json: string): string;
    constructor();
    resolve_effect(effect_id: bigint, result_json: string): string;
    view(): string;
}

/**
 * r" The address book: manual + history-derived merge, tombstones, groups.
 */
export class ContactsCore {
    free(): void;
    [Symbol.dispose](): void;
    dispatch(event_json: string): string;
    constructor();
    resolve_effect(effect_id: bigint, result_json: string): string;
    view(): string;
}

/**
 * r" Creating a wallet: register → prove signing → derive → sync → save.
 */
export class CreateWalletCore {
    free(): void;
    [Symbol.dispose](): void;
    dispatch(event_json: string): string;
    constructor();
    resolve_effect(effect_id: bigint, result_json: string): string;
    view(): string;
}

/**
 * r" Per-origin grants + browser consent.
 */
export class DappPermissionsCore {
    free(): void;
    [Symbol.dispose](): void;
    dispatch(event_json: string): string;
    constructor();
    resolve_effect(effect_id: bigint, result_json: string): string;
    view(): string;
}

/**
 * r" dApp connection lifecycle: pairing, fingerprint confirmation,
 * r" reconnect policy and the timer discipline behind it.
 */
export class DappSessionCore {
    free(): void;
    [Symbol.dispose](): void;
    dispatch(event_json: string): string;
    constructor();
    resolve_effect(effect_id: bigint, result_json: string): string;
    view(): string;
}

/**
 * r" The display currency: atomic code+rate pair, first-launch region seed,
 * r" user-choice-wins.
 */
export class DisplayCurrencyCore {
    free(): void;
    [Symbol.dispose](): void;
    dispatch(event_json: string): string;
    constructor();
    resolve_effect(effect_id: bigint, result_json: string): string;
    view(): string;
}

/**
 * r" Safari extension account snapshot + Universal Link TTL.
 */
export class ExtCacheCore {
    free(): void;
    [Symbol.dispose](): void;
    dispatch(event_json: string): string;
    constructor();
    resolve_effect(effect_id: bigint, result_json: string): string;
    view(): string;
}

/**
 * r" Fee quoting + reserve math (spec 017 wave A): tier pricing, in-band
 * r" quotes, sign-what-was-displayed guards.
 */
export class FeePolicyCore {
    free(): void;
    [Symbol.dispose](): void;
    dispatch(event_json: string): string;
    constructor();
    resolve_effect(effect_id: bigint, result_json: string): string;
    view(): string;
}

/**
 * r" The speed control of one send surface (spec 069): the tier in force,
 * r" the free upgrade, the one-speed statement and each tier's gas bid.
 */
export class FeeSpeedCore {
    free(): void;
    [Symbol.dispose](): void;
    dispatch(event_json: string): string;
    constructor();
    resolve_effect(effect_id: bigint, result_json: string): string;
    view(): string;
}

/**
 * r" The default transaction speed (spec 068): the stored tier a send
 * r" starts at, the factory `fast` when nothing was chosen.
 */
export class FeeTierPrefCore {
    free(): void;
    [Symbol.dispose](): void;
    dispatch(event_json: string): string;
    constructor();
    resolve_effect(effect_id: bigint, result_json: string): string;
    view(): string;
}

/**
 * A translation engine.
 */
export class I18n {
    free(): void;
    [Symbol.dispose](): void;
    changeLanguage(lng: string): LanguageState;
    dir(): string;
    exists(key: string, opts?: any | null): boolean;
    language(): string;
    /**
     * Make `lang`'s catalog active — the on-demand load.
     */
    loadCatalog(lang: string, json: Uint8Array): void;
    /**
     * Build from the `en` fallback catalog, supplied as the bytes of
     * `/i18n/en.json`.
     */
    constructor(fallback_json: Uint8Array);
    /**
     * Build an engine pinned to the LEGACY plural rule — i18next's `dummyRule`,
     * which is what a host without `Intl.PluralRules` silently falls back to.
     * Exposed so the conformance corpus can replay MODE B here too; production
     * code should never call it.
     */
    static newWithLegacyPlurals(fallback_json: Uint8Array): I18n;
    /**
     * Release `lang` if it is the active catalog. `en` is never releasable.
     */
    releaseCatalog(lang: string): boolean;
    residentBytes(): number;
    residentLocales(): string[];
    /**
     * Resolve `key`. Returns the key itself when nothing matches.
     */
    t(key: string, opts?: any | null): string;
    /**
     * First key that resolves wins; all-missing returns the **last** key.
     */
    tFirst(keys: string[], opts?: any | null): string;
}

/**
 * r" Signing in with an existing passkey, including on-device recovery.
 */
export class LoginCore {
    free(): void;
    [Symbol.dispose](): void;
    dispatch(event_json: string): string;
    constructor();
    resolve_effect(effect_id: bigint, result_json: string): string;
    view(): string;
}

/**
 * r" Manual custom-token management.
 */
export class ManageTokensCore {
    free(): void;
    [Symbol.dispose](): void;
    dispatch(event_json: string): string;
    constructor();
    resolve_effect(effect_id: bigint, result_json: string): string;
    view(): string;
}

/**
 * r" Network & endpoint configuration: add-network wizard, overrides,
 * r" service endpoints, provider keys.
 */
export class NetworkAdminCore {
    free(): void;
    [Symbol.dispose](): void;
    dispatch(event_json: string): string;
    constructor();
    resolve_effect(effect_id: bigint, result_json: string): string;
    view(): string;
}

/**
 * r" Payment requests: the acknowledge gate, the EIP-681/pay-link builder,
 * r" and the strict `/pay` validator.
 */
export class PaymentRequestCore {
    free(): void;
    [Symbol.dispose](): void;
    dispatch(event_json: string): string;
    constructor();
    resolve_effect(effect_id: bigint, result_json: string): string;
    view(): string;
}

/**
 * r" Deposit detection on the Receive screen: phased polling, baseline
 * r" diff, false-positive guards.
 */
export class ReceiveWatchCore {
    free(): void;
    [Symbol.dispose](): void;
    dispatch(event_json: string): string;
    constructor();
    resolve_effect(effect_id: bigint, result_json: string): string;
    view(): string;
}

/**
 * r" RPC/bundler endpoint pool decisions: scoring, cooldowns, bans.
 */
export class RpcPoolCore {
    free(): void;
    [Symbol.dispose](): void;
    dispatch(event_json: string): string;
    constructor();
    resolve_effect(effect_id: bigint, result_json: string): string;
    view(): string;
}

/**
 * r" The whole Send flow: three modes, the step machine, EIP-681 locked
 * r" requests, Max/fiat math, the treasury pre-check and the sign→submit
 * r" lifecycle behind a single-flight re-entry lock.
 */
export class SendCore {
    free(): void;
    [Symbol.dispose](): void;
    dispatch(event_json: string): string;
    constructor();
    resolve_effect(effect_id: bigint, result_json: string): string;
    view(): string;
}

/**
 * r" The wallet session truth source: accounts, active index, boot restore.
 */
export class SessionCore {
    free(): void;
    [Symbol.dispose](): void;
    dispatch(event_json: string): string;
    constructor();
    resolve_effect(effect_id: bigint, result_json: string): string;
    view(): string;
}

/**
 * r#" How this device signs by default (spec 071): the "Sign with" every"#
 * r" signing sheet starts at, and which Clear Signer page it opens.
 */
export class SignPrefCore {
    free(): void;
    [Symbol.dispose](): void;
    dispatch(event_json: string): string;
    constructor();
    resolve_effect(effect_id: bigint, result_json: string): string;
    view(): string;
}

/**
 * r" The dApp signing approval lifecycle.
 */
export class SignRequestCore {
    free(): void;
    [Symbol.dispose](): void;
    dispatch(event_json: string): string;
    constructor();
    resolve_effect(effect_id: bigint, result_json: string): string;
    view(): string;
}

/**
 * r" The token trust model: transfer allowlists, auto-add admission,
 * r" asymmetric simulation trust.
 */
export class TokenTrustCore {
    free(): void;
    [Symbol.dispose](): void;
    dispatch(event_json: string): string;
    constructor();
    resolve_effect(effect_id: bigint, result_json: string): string;
    view(): string;
}

/**
 * r" Post-submit transaction lifecycle / reconciliation.
 */
export class TxTrackerCore {
    free(): void;
    [Symbol.dispose](): void;
    dispatch(event_json: string): string;
    constructor();
    resolve_effect(effect_id: bigint, result_json: string): string;
    view(): string;
}

export function abiEncodeAddress(address_hex: string): Uint8Array;

export function abiEncodeBytes32(data: Uint8Array): Uint8Array;

export function abiEncodeUint256(value_hex: string): Uint8Array;

/**
 * The Safe message hash a passkey signs for EIP-1271 (`SafeMessage(bytes)`
 * under the Safe's own domain) — the core's reading, for comparison.
 */
export function attestSafeMessageHash(original_hash: Uint8Array, chain_id: bigint, safe_address: string): Uint8Array;

/**
 * The SafeOp hash of `op_json`, on `chain_id` — after checking that its
 * calldata is exactly what `calls_json` describes (pass `""` to skip the
 * calldata check and attest the hash alone).
 */
export function attestSafeOpHash(op_json: string, calls_json: string, chain_id: bigint): Uint8Array;

/**
 * The deepest pool across ALL stable quotes — `best_native_dex_price`, which
 * folds `best_group_price` over each group.
 */
export function bestNativeDexPrice(groups: NativeQuoteGroups): number | undefined;

/**
 * Derive the one-time group key from a 32-byte seed and build its closing
 * proof over the group's content-hash challenge.
 */
export function buildGroupProof(seed_hex: string, rp_id: string, challenge_hex: string): GroupProofJs;

/**
 * Assemble a member passkey's proof from its real WebAuthn assertion.
 */
export function buildMemberProof(authenticator_data_hex: string, client_data_json_hex: string, signature_der_hex: string): RegistryProofJs;

/**
 * Issue 212: may the relay's gas quote for one tier be held?
 */
export function bundlerQuoteCacheable(max_fee_per_gas: string): boolean;

export function canonicalizeSignature(sig: string): string;

export function checksumAddress(address_hex: string): string;

/**
 * The source ladder and its sanity band — `choose_native_price`.
 */
export function chooseNativePrice(dex?: number | null, chainlink_local?: number | null, chainlink_eth?: number | null): NativePriceChoice;

export function clearSignerDefaultUrl(): string;

/**
 * `{method, params, origin, chainId, chainName?, nativeSymbol?, account,
 * accountName?, credentialIdsHex, userOp?, calls?}` → the page's `{intent,
 * context}`. As the phones' `clear_signer_request`: `calls` are the
 * operation's legs before its fee leg (so the fee leg is `calls.length`),
 * and an empty `method` — the wallet's own send — makes them the intent.
 */
export function clearSignerRequest(input_json: string): string;

/**
 * The address normalised, or throws `"invalid"` / `"insecure"`.
 */
export function clearSignerUrl(input: string): string;

export function clearSignerUsesWalletPasskeys(url: string): boolean;

/**
 * The page's answer judged against `digest` and the account's keys
 * (`[{credentialId, publicKeyHex}]`): `{accepted: {credentialIdHex,
 * signatureDer, authenticatorData, clientDataJSON}}` or `{refused: {code,
 * detail}}`.
 */
export function clearSignerVerify(result_json: string, digest: Uint8Array, keys_json: string): string;

export function computeSafeAddress(x: Uint8Array, y: Uint8Array): SafeAddressInfo;

/**
 * Multi-device Safe: `keys_xy` is a concatenation of 64-byte x‖y blocks,
 * one per key — raw coordinates only, same byte convention as the
 * single-key `computeSafeAddress`. NOT hex strings: a bare-hex form would be
 * ambiguous for keys whose x starts with byte 0x04 (the SEC1-tag strip in
 * `parsePublicKey`). Key 0 drives the shared signer; later keys become
 * factory signer owners, their proxies deployed inside the setup MultiSend.
 */
export function computeSafeAddressMulti(keys_xy: Uint8Array): SafeAddressInfo;

export function computeSelector(sig: string): string;

export function computeSplitterAddress(treasury_hex: string): string;

export function computeWebauthnSignerAddress(x: Uint8Array, y: Uint8Array): string;

export function create2Address(deployer_hex: string, salt: Uint8Array, init_code_hash: Uint8Array): string;

/**
 * The document-start script an in-app browser injects, for `host`
 * (`"android"` / `"ios"` / `"desktop"`) — exported so the web suite can run
 * the real bridge in a real browser.
 */
export function dappProviderScript(host: string): string;

/**
 * The core's route for `method`, as JSON (`{"type":"read","bundler":true}`).
 */
export function dappRpcClassify(method: string): string;

export function decodeCalldata(sig: string, calldata: Uint8Array): AbiValue;

export function derSignatureToRawLowS(der: Uint8Array): Uint8Array;

/**
 * Encode the wallet's registry metadata blob to `0x`-hex, bounded to the
 * contract's 2048-byte cap.
 */
export function encodeRegistryMetadata(input: RegistryMetadataInput): string;

export function encodeSplitterDeployCall(treasury_hex: string): Uint8Array;

export function encodeType(typed_data_json: string): string;

export function extractAttestationPublicKey(attestation_object: Uint8Array): P256PublicKey;

/**
 * Issue 212: how long a chain's fee signals may be held, in ms — the one
 * number every shell's cache used to carry its own copy of.
 */
export function feeSignalsCacheTtlMs(): number;

export function fromBase64Url(s: string): Uint8Array;

export function fromHex(s: string): Uint8Array;

export function functionSelector(signature: string): Uint8Array;

/**
 * Issue 212: may this gas-signal read be held? Decimal wei in; see
 * `fee_policy::gas_signals_cacheable`.
 */
export function gasSignalsCacheable(eth_gas_price: string | null | undefined, block_answered: boolean, want_tip: boolean, priority_fee?: string | null): boolean;

/**
 * The uncompressed public key of the one-time group key a 32-byte seed
 * derives — needed before requesting the group's challenge.
 */
export function groupPublicKeyFromSeed(seed_hex: string): string;

export function hashTypedData(typed_data_json: string): Uint8Array;

/**
 * Interpolate a template in isolation, without a key lookup.
 */
export function i18nInterpolate(template: string, opts?: TOptions | null): string;

export function i18nPluralSuffix(locale: string, count: number): string;

export function i18nPluralSuffixLegacy(count: number): string;

export function i18nPluralSuffixes(locale: string): string[];

export function i18nPluralSuffixesLegacy(): string[];

export function i18nTextDirection(lng: string): string;

/**
 * Stock output as a `data:image/svg+xml;base64,…` URI.
 */
export function identiconDataUri(seed: string): string;

export function identiconMakeHash(seed: string): string;

/**
 * Case- and length-normalises a seed. Every platform must call this rather than
 * lowercasing locally — that is how the platforms drift apart.
 */
export function identiconNormalizeSeed(seed: string): string;

export function identiconParams(seed: string): IdenticonParams;

/**
 * The library's stock hexagonal output.
 */
export function identiconSvg(seed: string): string;

/**
 * **The wallet's identicon.** Circular variant, no SVG ids — several instances can
 * share one DOM without their clip paths colliding.
 */
export function identiconSvgCircular(seed: string): string;

export function keccak256(data: Uint8Array): Uint8Array;

export function matchSelector(sig: string, calldata: Uint8Array): boolean;

/**
 * The lowest gas price a chain accepts, as a decimal string (money crosses
 * this boundary as decimal strings, never as JS numbers).
 *
 * `"0"` on every chain but Arc, which discards an underpriced transaction
 * SILENTLY — no error, no trace, a payment that simply never happens. The web
 * shell keeps a TypeScript twin of the gas-price derivation
 * (`safe-transaction.ts`), and it reads the floor from here rather than
 * carrying its own copy of the number.
 */
export function minGasPriceWei(chain_id: number): string;

export function parsePublicKey(hex: string): P256PublicKey;

/**
 * **Read a directory answer.** `undefined` unless the body is about the AAGUID
 * that was asked about and carries a usable name; `iconUrl` is present only
 * when the path is the service's own shape.
 */
export function passkeyDirectoryEntry(aaguid: string, json: string, dark: boolean, origin?: string | null): any;

/**
 * **Where to ask about a model the compiled catalog cannot name**, or
 * `undefined` when there is nothing to ask: a malformed or all-zero AAGUID, or
 * one the catalog already answers offline.
 *
 * `origin` is the directory node the person's service-endpoint settings
 * name (spec 038 #E4); absent, the crate's default.
 */
export function passkeyDirectoryUrl(aaguid: string, origin?: string | null): string | undefined;

/**
 * **The security-key fallback mark**, as an `image/svg+xml` data URI, for a
 * key whose AAGUID the catalog cannot name. `undefined` when the row deserves
 * no mark of this kind — a platform authenticator, which the client already
 * draws its own way.
 *
 * The three colours are the caller's tokens: the artwork ships in one theme,
 * and one vendor's greys are not this app's greys in either.
 */
export function passkeyFallbackIconDataUri(authenticator_attachment: string, transports: string, chose_security_key: boolean, strong: string, soft: string, hole: string): string | undefined;

/**
 * **A passkey provider's mark**, as an `image/svg+xml` data URI, from the
 * vendored AAGUID catalog. `undefined` when the catalog does not know the
 * model — the caller then shows what it showed before this existed.
 *
 * A data URI rather than markup to inline: these marks carry `<style>` blocks
 * and `clipPath` ids, and several of them inlined into one document would
 * fight over both. The lookup is offline by construction — asking a directory
 * service would tell it which vault holds a Vela wallet's key.
 */
export function passkeyProviderIconDataUri(aaguid: string, dark: boolean): string | undefined;

/**
 * The provider's brand name, or an empty string when the catalog has no entry.
 */
export function passkeyProviderName(aaguid: string): string;

/**
 * The $1 peg for a native gas coin that IS a dollar stablecoin — Tempo's
 * `USD`, Arc's `USDC`. `None` means "not pegged": the caller falls through to
 * the Chainlink/DEX ladder unchanged.
 *
 * This exists so the rule is written once. It used to be a `symbol == "USD"`
 * literal in each of the four shells, which is four chances to disagree about
 * what a coin is worth.
 */
export function peggedNativeUsd(symbol: string): number | undefined;

/**
 * `{key: rawValue}` → `[{key, value | null}]`, the writes that bring an
 * older spelling to the shared record.
 */
export function prefsMigrations(entries_json: string): string;

/**
 * `{key: rawValue}` → `{theme, language, avatarStyle, textScale,
 * textScaleFactor, numberFormat, dateFormat, timeFormat}`.
 */
export function prefsRead(entries_json: string): string;

/**
 * Returns `null` when the two assertions do not pin down exactly one key
 * (different credentials, or the same signature twice) — that is a legitimate
 * outcome, not an error.
 */
export function recoverPublicKeyFromAssertions(a_authenticator_data: Uint8Array, a_client_data_json: Uint8Array, a_signature_der: Uint8Array, b_authenticator_data: Uint8Array, b_client_data_json: Uint8Array, b_signature_der: Uint8Array): P256PublicKey | undefined;

/**
 * **Backing the founding record up to Ethereum — the next step of the walk**
 * (spec 062). Server-free: every request is an `eth_call` against the
 * registry contract, on Gnosis (where the record lives) or Ethereum (where
 * the backup goes). `answers_json` is the transcript (`LookupAnswer[]`, the
 * same shape `registryNameStep` uses); the return is a `BackupStep` as JSON —
 * requests to perform, or the verdict and, when the wallet is not backed up,
 * the one call that would do it. No passkey is involved at any point.
 * `target_chain` is Ethereum when absent; Base (8453) is the operator's
 * rehearsal deployment; anything else is refused.
 */
export function registryBackupStep(address: string, founding_public_key_hex: string, answers_json: string, target_chain?: number | null): string;

/**
 * **Signing in when the index is gone** (spec 062): the registry contract's
 * side of the index's two read questions, answered in the index's own JSON
 * shapes so a shell's existing parsing runs unchanged. `…Plan` = the chains to
 * try (in order) and the two `eth_call`s to make on one of them; the matching
 * function turns the two raw results into the body, or nothing when that
 * chain did not really answer (the shell then tries the next). Unit ids are
 * per deployment: ask about a key's units on the chain that listed them.
 */
export function registryChainKeyPlan(public_key_hex: string): string | undefined;

/**
 * See [`registry_chain_key_plan`].
 */
export function registryChainKeyStatus(has_entry_hex: string, groups_hex: string): string | undefined;

/**
 * See [`registry_chain_key_plan`].
 */
export function registryChainUnit(unit_id: number, unit_hex: string, members_hex: string): string | undefined;

/**
 * See [`registry_chain_key_plan`].
 */
export function registryChainUnitPlan(unit_id: number): string | undefined;

/**
 * **The registered name behind an address — the next step of the lookup.**
 *
 * The v2 passkey index cannot be asked about an address, so the name is
 * reached through the chain (`vela_core::registry_lookup`). `answers_json` is
 * the transcript so far (`LookupAnswer[]`); the return is a `LookupStep` as
 * JSON: requests to perform (an `eth_call`, a GET against the configured
 * index), or the verdict and how long it may be remembered. The shell owns
 * the transport; every rule is the core's.
 */
export function registryNameStep(address: string, answers_json: string): string;

/**
 * Sign-in's first question — which groups does this key belong to? — through
 * the three layers: the index, then the registry contract on Gnosis, then on
 * Ethereum. See `vela_core::registry_resolve`.
 */
export function registryResolveKeyStep(public_key_hex: string, answers_json: string): string;

/**
 * Sign-in's second question — who are this group's members? — with an index
 * answer PROVED against the chain (`contentHash`), not merely believed.
 * `source` is the token the key step returned.
 */
export function registryResolveUnitStep(unit_id: number, source: string, answers_json: string): string;

export function safeProxyRuntimeCode(): string;

export function sha256(data: Uint8Array): Uint8Array;

/**
 * What a site's message request asks the account to sign, before the
 * Safe's `SafeMessage` wrap — the phones' `sign_message_hash`.
 */
export function signMessageHash(method: string, params_json: string): Uint8Array | undefined;

/**
 * `{value, unit}` — a byte count in 1024s, for the shell to format in the
 * person's numbers.
 */
export function storageBytesDisplay(bytes: number): string;

export function storageIsCacheKey(key: string): boolean;

export function storageIsErasableKey(key: string): boolean;

/**
 * Is this key the wallet's at all (counted in the storage total)?
 */
export function storageIsOurs(key: string): boolean;

/**
 * The row a key belongs to, or `undefined`.
 */
export function storageItemOfKey(key: string): string | undefined;

/**
 * `[{id, group}]`, in the order the page draws them.
 */
export function storageItems(): string;

/**
 * Records in a stored list value; `undefined` when it is not a list.
 */
export function storageRecordsIn(value: string): number | undefined;

export function toBase64Url(data: Uint8Array): string;

export function toHex(data: Uint8Array, prefixed: boolean): string;

export function toQuantity(value: string): string;

/**
 * `kind` is `"create"` or `"get"` (anything else errors — the caller is
 * choosing which contract-mirrored rule set applies).
 */
export function validateClientData(kind: string, client_data_json: Uint8Array, authenticator_data: Uint8Array): void;

/**
 * Which passkeys control the wallet at `address` — the Settings keys view
 * (spec 062). `device_keys_json` is the account record's `keys` array (or a
 * one-element array built from the legacy scalars); the answer is `ask` with
 * `eth_call`s to perform, or `done` with the rows and where they came from.
 */
export function walletKeysStep(address: string, device_keys_json: string, answers_json: string): string;

export function webauthnSigningHash(authenticator_data: Uint8Array, client_data_json: Uint8Array): Uint8Array;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_activityfeedcore_free: (a: number, b: number) => void;
    readonly __wbg_approvalguardcore_free: (a: number, b: number) => void;
    readonly __wbg_balancedashboardcore_free: (a: number, b: number) => void;
    readonly __wbg_batchimportcore_free: (a: number, b: number) => void;
    readonly __wbg_browserhistorycore_free: (a: number, b: number) => void;
    readonly __wbg_clearsignerws_free: (a: number, b: number) => void;
    readonly __wbg_clearsigningcore_free: (a: number, b: number) => void;
    readonly __wbg_contactscore_free: (a: number, b: number) => void;
    readonly __wbg_createwalletcore_free: (a: number, b: number) => void;
    readonly __wbg_dapppermissionscore_free: (a: number, b: number) => void;
    readonly __wbg_dappsessioncore_free: (a: number, b: number) => void;
    readonly __wbg_displaycurrencycore_free: (a: number, b: number) => void;
    readonly __wbg_extcachecore_free: (a: number, b: number) => void;
    readonly __wbg_feepolicycore_free: (a: number, b: number) => void;
    readonly __wbg_feespeedcore_free: (a: number, b: number) => void;
    readonly __wbg_feetierprefcore_free: (a: number, b: number) => void;
    readonly __wbg_i18n_free: (a: number, b: number) => void;
    readonly __wbg_logincore_free: (a: number, b: number) => void;
    readonly __wbg_managetokenscore_free: (a: number, b: number) => void;
    readonly __wbg_networkadmincore_free: (a: number, b: number) => void;
    readonly __wbg_paymentrequestcore_free: (a: number, b: number) => void;
    readonly __wbg_receivewatchcore_free: (a: number, b: number) => void;
    readonly __wbg_rpcpoolcore_free: (a: number, b: number) => void;
    readonly __wbg_sendcore_free: (a: number, b: number) => void;
    readonly __wbg_sessioncore_free: (a: number, b: number) => void;
    readonly __wbg_signprefcore_free: (a: number, b: number) => void;
    readonly __wbg_signrequestcore_free: (a: number, b: number) => void;
    readonly __wbg_tokentrustcore_free: (a: number, b: number) => void;
    readonly __wbg_txtrackercore_free: (a: number, b: number) => void;
    readonly abiEncodeAddress: (a: number, b: number) => [number, number, number, number];
    readonly abiEncodeBytes32: (a: number, b: number) => [number, number, number, number];
    readonly abiEncodeUint256: (a: number, b: number) => [number, number, number, number];
    readonly activityfeedcore_dispatch: (a: number, b: number, c: number) => [number, number, number, number];
    readonly activityfeedcore_new: () => number;
    readonly activityfeedcore_resolve_effect: (a: number, b: bigint, c: number, d: number) => [number, number, number, number];
    readonly activityfeedcore_view: (a: number) => [number, number, number, number];
    readonly approvalguardcore_dispatch: (a: number, b: number, c: number) => [number, number, number, number];
    readonly approvalguardcore_new: () => number;
    readonly approvalguardcore_resolve_effect: (a: number, b: bigint, c: number, d: number) => [number, number, number, number];
    readonly approvalguardcore_view: (a: number) => [number, number, number, number];
    readonly attestSafeMessageHash: (a: number, b: number, c: bigint, d: number, e: number) => [number, number, number, number];
    readonly attestSafeOpHash: (a: number, b: number, c: number, d: number, e: bigint) => [number, number, number, number];
    readonly balancedashboardcore_dispatch: (a: number, b: number, c: number) => [number, number, number, number];
    readonly balancedashboardcore_new: () => number;
    readonly balancedashboardcore_resolve_effect: (a: number, b: bigint, c: number, d: number) => [number, number, number, number];
    readonly balancedashboardcore_view: (a: number) => [number, number, number, number];
    readonly batchimportcore_dispatch: (a: number, b: number, c: number) => [number, number, number, number];
    readonly batchimportcore_new: () => number;
    readonly batchimportcore_resolve_effect: (a: number, b: bigint, c: number, d: number) => [number, number, number, number];
    readonly batchimportcore_view: (a: number) => [number, number, number, number];
    readonly bestNativeDexPrice: (a: any) => [number, number];
    readonly browserhistorycore_dispatch: (a: number, b: number, c: number) => [number, number, number, number];
    readonly browserhistorycore_new: () => number;
    readonly browserhistorycore_resolve_effect: (a: number, b: bigint, c: number, d: number) => [number, number, number, number];
    readonly browserhistorycore_view: (a: number) => [number, number, number, number];
    readonly buildGroupProof: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number, number];
    readonly buildMemberProof: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number, number];
    readonly bundlerQuoteCacheable: (a: number, b: number) => number;
    readonly canonicalizeSignature: (a: number, b: number) => [number, number, number, number];
    readonly checksumAddress: (a: number, b: number) => [number, number, number, number];
    readonly chooseNativePrice: (a: number, b: number, c: number, d: number, e: number, f: number) => any;
    readonly clearSignerDefaultUrl: () => [number, number];
    readonly clearSignerRequest: (a: number, b: number) => [number, number, number, number];
    readonly clearSignerUrl: (a: number, b: number) => [number, number, number, number];
    readonly clearSignerUsesWalletPasskeys: (a: number, b: number) => number;
    readonly clearSignerVerify: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number, number, number];
    readonly clearsignerws_closed: (a: number) => [number, number];
    readonly clearsignerws_feed: (a: number, b: number, c: number) => [number, number];
    readonly clearsignerws_new: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number) => [number, number, number];
    readonly clearsigningcore_dispatch: (a: number, b: number, c: number) => [number, number, number, number];
    readonly clearsigningcore_new: () => number;
    readonly clearsigningcore_resolve_effect: (a: number, b: bigint, c: number, d: number) => [number, number, number, number];
    readonly clearsigningcore_view: (a: number) => [number, number, number, number];
    readonly computeSafeAddress: (a: number, b: number, c: number, d: number) => [number, number, number];
    readonly computeSafeAddressMulti: (a: number, b: number) => [number, number, number];
    readonly computeSelector: (a: number, b: number) => [number, number, number, number];
    readonly computeSplitterAddress: (a: number, b: number) => [number, number, number, number];
    readonly computeWebauthnSignerAddress: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly contactscore_dispatch: (a: number, b: number, c: number) => [number, number, number, number];
    readonly contactscore_new: () => number;
    readonly contactscore_resolve_effect: (a: number, b: bigint, c: number, d: number) => [number, number, number, number];
    readonly contactscore_view: (a: number) => [number, number, number, number];
    readonly create2Address: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number, number, number];
    readonly createwalletcore_dispatch: (a: number, b: number, c: number) => [number, number, number, number];
    readonly createwalletcore_new: () => number;
    readonly createwalletcore_resolve_effect: (a: number, b: bigint, c: number, d: number) => [number, number, number, number];
    readonly createwalletcore_view: (a: number) => [number, number, number, number];
    readonly dappProviderScript: (a: number, b: number) => [number, number];
    readonly dappRpcClassify: (a: number, b: number) => [number, number];
    readonly dapppermissionscore_dispatch: (a: number, b: number, c: number) => [number, number, number, number];
    readonly dapppermissionscore_new: () => number;
    readonly dapppermissionscore_resolve_effect: (a: number, b: bigint, c: number, d: number) => [number, number, number, number];
    readonly dapppermissionscore_view: (a: number) => [number, number, number, number];
    readonly dappsessioncore_dispatch: (a: number, b: number, c: number) => [number, number, number, number];
    readonly dappsessioncore_new: () => number;
    readonly dappsessioncore_resolve_effect: (a: number, b: bigint, c: number, d: number) => [number, number, number, number];
    readonly dappsessioncore_view: (a: number) => [number, number, number, number];
    readonly decodeCalldata: (a: number, b: number, c: number, d: number) => [number, number, number];
    readonly derSignatureToRawLowS: (a: number, b: number) => [number, number, number, number];
    readonly displaycurrencycore_dispatch: (a: number, b: number, c: number) => [number, number, number, number];
    readonly displaycurrencycore_new: () => number;
    readonly displaycurrencycore_resolve_effect: (a: number, b: bigint, c: number, d: number) => [number, number, number, number];
    readonly displaycurrencycore_view: (a: number) => [number, number, number, number];
    readonly encodeRegistryMetadata: (a: any) => [number, number, number, number];
    readonly encodeSplitterDeployCall: (a: number, b: number) => [number, number, number, number];
    readonly encodeType: (a: number, b: number) => [number, number, number, number];
    readonly extcachecore_dispatch: (a: number, b: number, c: number) => [number, number, number, number];
    readonly extcachecore_new: () => number;
    readonly extcachecore_resolve_effect: (a: number, b: bigint, c: number, d: number) => [number, number, number, number];
    readonly extcachecore_view: (a: number) => [number, number, number, number];
    readonly extractAttestationPublicKey: (a: number, b: number) => [number, number, number];
    readonly feeSignalsCacheTtlMs: () => number;
    readonly feepolicycore_dispatch: (a: number, b: number, c: number) => [number, number, number, number];
    readonly feepolicycore_new: () => number;
    readonly feepolicycore_resolve_effect: (a: number, b: bigint, c: number, d: number) => [number, number, number, number];
    readonly feepolicycore_view: (a: number) => [number, number, number, number];
    readonly feespeedcore_dispatch: (a: number, b: number, c: number) => [number, number, number, number];
    readonly feespeedcore_new: () => number;
    readonly feespeedcore_resolve_effect: (a: number, b: bigint, c: number, d: number) => [number, number, number, number];
    readonly feespeedcore_view: (a: number) => [number, number, number, number];
    readonly feetierprefcore_dispatch: (a: number, b: number, c: number) => [number, number, number, number];
    readonly feetierprefcore_new: () => number;
    readonly feetierprefcore_resolve_effect: (a: number, b: bigint, c: number, d: number) => [number, number, number, number];
    readonly feetierprefcore_view: (a: number) => [number, number, number, number];
    readonly fromBase64Url: (a: number, b: number) => [number, number, number, number];
    readonly fromHex: (a: number, b: number) => [number, number, number, number];
    readonly functionSelector: (a: number, b: number) => [number, number, number, number];
    readonly gasSignalsCacheable: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly groupPublicKeyFromSeed: (a: number, b: number) => [number, number, number, number];
    readonly hashTypedData: (a: number, b: number) => [number, number, number, number];
    readonly i18nInterpolate: (a: number, b: number, c: number) => [number, number, number, number];
    readonly i18nPluralSuffix: (a: number, b: number, c: number) => [number, number];
    readonly i18nPluralSuffixLegacy: (a: number) => [number, number];
    readonly i18nPluralSuffixes: (a: number, b: number) => [number, number];
    readonly i18nPluralSuffixesLegacy: () => [number, number];
    readonly i18nTextDirection: (a: number, b: number) => [number, number];
    readonly i18n_changeLanguage: (a: number, b: number, c: number) => any;
    readonly i18n_dir: (a: number) => [number, number];
    readonly i18n_exists: (a: number, b: number, c: number, d: number) => [number, number, number];
    readonly i18n_language: (a: number) => [number, number];
    readonly i18n_loadCatalog: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly i18n_new: (a: number, b: number) => [number, number, number];
    readonly i18n_newWithLegacyPlurals: (a: number, b: number) => [number, number, number];
    readonly i18n_releaseCatalog: (a: number, b: number, c: number) => number;
    readonly i18n_residentBytes: (a: number) => number;
    readonly i18n_residentLocales: (a: number) => [number, number];
    readonly i18n_t: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly i18n_tFirst: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly identiconDataUri: (a: number, b: number) => [number, number, number, number];
    readonly identiconMakeHash: (a: number, b: number) => [number, number];
    readonly identiconNormalizeSeed: (a: number, b: number) => [number, number];
    readonly identiconParams: (a: number, b: number) => [number, number, number];
    readonly identiconSvg: (a: number, b: number) => [number, number, number, number];
    readonly identiconSvgCircular: (a: number, b: number) => [number, number, number, number];
    readonly keccak256: (a: number, b: number) => [number, number];
    readonly logincore_dispatch: (a: number, b: number, c: number) => [number, number, number, number];
    readonly logincore_new: () => number;
    readonly logincore_resolve_effect: (a: number, b: bigint, c: number, d: number) => [number, number, number, number];
    readonly logincore_view: (a: number) => [number, number, number, number];
    readonly managetokenscore_dispatch: (a: number, b: number, c: number) => [number, number, number, number];
    readonly managetokenscore_new: () => number;
    readonly managetokenscore_resolve_effect: (a: number, b: bigint, c: number, d: number) => [number, number, number, number];
    readonly managetokenscore_view: (a: number) => [number, number, number, number];
    readonly matchSelector: (a: number, b: number, c: number, d: number) => [number, number, number];
    readonly minGasPriceWei: (a: number) => [number, number];
    readonly networkadmincore_dispatch: (a: number, b: number, c: number) => [number, number, number, number];
    readonly networkadmincore_new: () => number;
    readonly networkadmincore_resolve_effect: (a: number, b: bigint, c: number, d: number) => [number, number, number, number];
    readonly networkadmincore_view: (a: number) => [number, number, number, number];
    readonly parsePublicKey: (a: number, b: number) => [number, number, number];
    readonly passkeyDirectoryEntry: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => [number, number, number];
    readonly passkeyDirectoryUrl: (a: number, b: number, c: number, d: number) => [number, number];
    readonly passkeyFallbackIconDataUri: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number) => [number, number];
    readonly passkeyProviderIconDataUri: (a: number, b: number, c: number) => [number, number];
    readonly passkeyProviderName: (a: number, b: number) => [number, number];
    readonly paymentrequestcore_dispatch: (a: number, b: number, c: number) => [number, number, number, number];
    readonly paymentrequestcore_new: () => number;
    readonly paymentrequestcore_resolve_effect: (a: number, b: bigint, c: number, d: number) => [number, number, number, number];
    readonly paymentrequestcore_view: (a: number) => [number, number, number, number];
    readonly peggedNativeUsd: (a: number, b: number) => [number, number];
    readonly prefsMigrations: (a: number, b: number) => [number, number];
    readonly prefsRead: (a: number, b: number) => [number, number];
    readonly receivewatchcore_dispatch: (a: number, b: number, c: number) => [number, number, number, number];
    readonly receivewatchcore_new: () => number;
    readonly receivewatchcore_resolve_effect: (a: number, b: bigint, c: number, d: number) => [number, number, number, number];
    readonly receivewatchcore_view: (a: number) => [number, number, number, number];
    readonly recoverPublicKeyFromAssertions: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number) => [number, number, number];
    readonly registryBackupStep: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => [number, number];
    readonly registryChainKeyPlan: (a: number, b: number) => [number, number];
    readonly registryChainKeyStatus: (a: number, b: number, c: number, d: number) => [number, number];
    readonly registryChainUnit: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly registryChainUnitPlan: (a: number) => [number, number];
    readonly registryNameStep: (a: number, b: number, c: number, d: number) => [number, number];
    readonly registryResolveKeyStep: (a: number, b: number, c: number, d: number) => [number, number];
    readonly registryResolveUnitStep: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly rpcpoolcore_dispatch: (a: number, b: number, c: number) => [number, number, number, number];
    readonly rpcpoolcore_new: () => number;
    readonly rpcpoolcore_resolve_effect: (a: number, b: bigint, c: number, d: number) => [number, number, number, number];
    readonly rpcpoolcore_view: (a: number) => [number, number, number, number];
    readonly safeProxyRuntimeCode: () => [number, number, number, number];
    readonly sendcore_dispatch: (a: number, b: number, c: number) => [number, number, number, number];
    readonly sendcore_new: () => number;
    readonly sendcore_resolve_effect: (a: number, b: bigint, c: number, d: number) => [number, number, number, number];
    readonly sendcore_view: (a: number) => [number, number, number, number];
    readonly sessioncore_dispatch: (a: number, b: number, c: number) => [number, number, number, number];
    readonly sessioncore_new: () => number;
    readonly sessioncore_resolve_effect: (a: number, b: bigint, c: number, d: number) => [number, number, number, number];
    readonly sessioncore_view: (a: number) => [number, number, number, number];
    readonly sha256: (a: number, b: number) => [number, number];
    readonly signMessageHash: (a: number, b: number, c: number, d: number) => [number, number];
    readonly signprefcore_dispatch: (a: number, b: number, c: number) => [number, number, number, number];
    readonly signprefcore_new: () => number;
    readonly signprefcore_resolve_effect: (a: number, b: bigint, c: number, d: number) => [number, number, number, number];
    readonly signprefcore_view: (a: number) => [number, number, number, number];
    readonly signrequestcore_dispatch: (a: number, b: number, c: number) => [number, number, number, number];
    readonly signrequestcore_new: () => number;
    readonly signrequestcore_resolve_effect: (a: number, b: bigint, c: number, d: number) => [number, number, number, number];
    readonly signrequestcore_view: (a: number) => [number, number, number, number];
    readonly storageBytesDisplay: (a: number) => [number, number];
    readonly storageIsCacheKey: (a: number, b: number) => number;
    readonly storageIsErasableKey: (a: number, b: number) => number;
    readonly storageIsOurs: (a: number, b: number) => number;
    readonly storageItemOfKey: (a: number, b: number) => [number, number];
    readonly storageItems: () => [number, number];
    readonly storageRecordsIn: (a: number, b: number) => number;
    readonly toBase64Url: (a: number, b: number) => [number, number];
    readonly toHex: (a: number, b: number, c: number) => [number, number];
    readonly toQuantity: (a: number, b: number) => [number, number, number, number];
    readonly tokentrustcore_dispatch: (a: number, b: number, c: number) => [number, number, number, number];
    readonly tokentrustcore_new: () => number;
    readonly tokentrustcore_resolve_effect: (a: number, b: bigint, c: number, d: number) => [number, number, number, number];
    readonly tokentrustcore_view: (a: number) => [number, number, number, number];
    readonly txtrackercore_dispatch: (a: number, b: number, c: number) => [number, number, number, number];
    readonly txtrackercore_new: () => number;
    readonly txtrackercore_resolve_effect: (a: number, b: bigint, c: number, d: number) => [number, number, number, number];
    readonly txtrackercore_view: (a: number) => [number, number, number, number];
    readonly validateClientData: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number];
    readonly walletKeysStep: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number];
    readonly webauthnSigningHash: (a: number, b: number, c: number, d: number) => [number, number];
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __externref_drop_slice: (a: number, b: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
