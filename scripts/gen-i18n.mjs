#!/usr/bin/env node
/**
 * Generates the compiled-in i18n tables in `rust/crates/vela-core/src/` from the
 * translation corpus (spec 004-rust-i18n, FR-010 / research D2).
 *
 * Source   <-  rust/crates/vela-core/i18n/locales/   240 files, THE source of truth
 * Stage 1  ->  rust/.../src/i18n/paths.rs             the SHARED path table, paid once
 * Stage 2  ->  rust/.../src/i18n_catalogs/<lng>.rs    one value blob per locale
 * Stage 4  ->  assets/i18n/<lng>.json                  the on-demand catalog every shell reads
 * Stage 5  ->  rust/.../src/l10n/datetime_data.rs      day periods + weekday names
 *
 * (Stage 3 was `src/i18n/resources.ts`, the React Native app's import; it went
 * with that app in spec 039. The numbering is kept so older notes still line up.)
 *
 * Why the split: the 1,141 dotted paths repeated per locale cost 460,471 bytes;
 * interned once they cost 31,198 — a 14.8x collapse, and the only reason `ja`+`en`
 * fits SC-005's 135,345-byte residency budget at a measured 126,352.
 *
 * Usage:  node scripts/gen-i18n.mjs
 * Then:   git diff --stat rust/crates/vela-core/src/i18n/paths.rs \
 *                        rust/crates/vela-core/src/i18n_catalogs
 *         (expected: no change)
 *
 * A silently partial table is the worst possible outcome — it would compile, pass
 * most tests, and render the wrong string for a subset of keys. So every structural
 * assumption is asserted before a single byte is written.
 */

import { mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO_ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');
// THE single source of truth for translation content (FR-010). This inverts the
// repository's usual codegen direction — the crate has always been the destination
// — and the inversion is the point: edit a string here and every platform artefact
// below regenerates from it.
const LOCALES_DIR = join(REPO_ROOT, 'rust/crates/vela-core/i18n/locales');
const PATHS_FILE = join(REPO_ROOT, 'rust/crates/vela-core/src/i18n/paths.rs');
const CATALOG_DIR = join(REPO_ROOT, 'rust/crates/vela-core/src/i18n_catalogs');
// `assets/i18n/` is read AT THIS PATH by every shell and two gates: app-ios
// (Xcode file lists + bundle-catalogs.sh), app-android (build.gradle.kts and the
// fixture tests), app-web/vela-wallet (engine.server.ts, at build time), the
// Kotlin/Swift harnesses, scripts/verify-i18n-parity.mjs and
// rust/scripts/verify-web.mjs. Moving it is a five-toolchain edit — spec 039
// did exactly one, from the Expo-era `public/i18n`, and the list above is
// where to look if it ever has to happen again.
const ASSET_DIR = join(REPO_ROOT, 'assets/i18n');
const DATETIME_FILE = join(REPO_ROOT, 'rust/crates/vela-core/src/l10n/datetime_data.rs');

// Locale order and namespace spread order are the ones the original
// hand-maintained resource file had, so a later namespace overwriting an
// earlier key behaves identically here (and in every artefact below).
const LOCALES = ['en', 'zh', 'zh-TW', 'zh-HK', 'ja', 'ko', 'vi', 'id', 'tr', 'es-MX', 'pt-BR', 'fr', 'de', 'ru', 'it'];
const NAMESPACE_FILES = [
  'home', 'send', 'receive', 'assets', 'addToken', 'tokenDetail', 'history',
  'onboarding', 'connect', 'about', 'clearSigning', 'componentsTx',
  'componentsUi', 'settingsModals', 'contacts', 'explore',
];

/** `es-MX` -> `es-mx`, matching the cargo feature name. */
const featureOf = (lng) => `i18n-${lng.toLowerCase()}`;
/** `es-MX` -> `es_mx`, matching the Rust module name. */
const modOf = (lng) => lng.toLowerCase().replace(/-/g, '_');

function fail(message) {
  console.error(`gen-i18n: ${message}`);
  process.exit(1);
}

// ---------------------------------------------------------------------------
// Load — never trim, never normalise
// ---------------------------------------------------------------------------
//
// 39 values are sentence fragments whose leading/trailing whitespace is
// concatenated at render time, and zh/zh-TW/zh-HK deliberately OMIT that
// whitespace, so no uniform rule applies. All 240 files are already NFC, so
// re-normalising is a no-op that can only ever become a spurious diff.

const readJson = (p) => JSON.parse(readFileSync(p, 'utf8'));

function buildLocale(lng) {
  let out = { ...readJson(join(LOCALES_DIR, `${lng}.json`)) };
  for (const ns of NAMESPACE_FILES) out = { ...out, ...readJson(join(LOCALES_DIR, lng, `${ns}.json`)) };
  return out;
}

const bundles = {};
for (const lng of LOCALES) bundles[lng] = buildLocale(lng);

// ---------------------------------------------------------------------------
// Stage 1 — the shared path table
// ---------------------------------------------------------------------------

/** Collect every leaf path and every branch (object) path. */
function walk(obj, prefix, leaves, branches) {
  for (const k of Object.keys(obj)) {
    const v = obj[k];
    const p = prefix ? `${prefix}.${k}` : k;
    if (v && typeof v === 'object' && !Array.isArray(v)) {
      branches.add(p);
      walk(v, p, leaves, branches);
    } else {
      if (typeof v !== 'string') fail(`non-string leaf at ${p} (${typeof v}) — the corpus is meant to be all strings`);
      leaves.add(p);
    }
  }
}

const leafSet = new Set();
const branchSet = new Set();
for (const lng of LOCALES) walk(bundles[lng], '', leafSet, branchSet);

// A path that is a leaf in one locale and a branch in another would make the
// branch bitmap ambiguous, and `t()` would return a translation in one language
// and the object diagnostic in another.
const both = [...leafSet].filter((p) => branchSet.has(p));
if (both.length) fail(`${both.length} paths are BOTH leaf and branch: ${both.slice(0, 5).join(', ')}`);

// The branch structure must be identical across locales, or the shared table
// cannot be shared. Verified rather than assumed.
for (const lng of LOCALES) {
  const l = new Set(), b = new Set();
  walk(bundles[lng], '', l, b);
  const missing = [...branchSet].filter((p) => !b.has(p));
  const extra = [...b].filter((p) => !branchSet.has(p));
  if (missing.length || extra.length) {
    fail(`locale ${lng} branch set differs from the union (missing ${missing.length}, extra ${extra.length})`);
  }
}

// `onboarding.welcome.heroTitleFit` is the one corpus value that is NOT prose:
// it is the enum that picks the Welcome headline's type tier, because the tier
// is a property OF the translation (how wide its two authored lines are) and
// travels with it to all four clients through the same `t()` they already call.
// A corpus value is a translator's to edit, so the one thing that must never
// happen — someone translating "long" into their language and every client
// silently falling back to the untranslated tier — fails generation here.
const HERO_FIT_KEY = 'onboarding.welcome.heroTitleFit';
const HERO_FIT_VALUES = new Set(['regular', 'long']);
for (const lng of LOCALES) {
  const value = HERO_FIT_KEY.split('.').reduce((o, k) => (o ?? {})[k], bundles[lng]);
  if (!HERO_FIT_VALUES.has(value)) {
    fail(`locale ${lng}: ${HERO_FIT_KEY} is ${JSON.stringify(value)} — expected one of ${[...HERO_FIT_VALUES].join(', ')}. It is an enum, not prose; do not translate it.`);
  }
}

const PATHS = [...new Set([...leafSet, ...branchSet])].sort();
const IS_BRANCH = PATHS.map((p) => (branchSet.has(p) ? 1 : 0));

for (let i = 1; i < PATHS.length; i++) {
  if (PATHS[i] <= PATHS[i - 1]) fail(`path table is not strictly sorted at ${i}: ${PATHS[i - 1]} >= ${PATHS[i]}`);
}
// 1323 = 1205 (spec 004 baseline) + 13 desktop-onboarding leaves (spec 007)
// + 25 welcomeWeb paths (spec 006: 16 leaves, 9 branches)
// + 2 in-band fee-hold leaves (spec 013: send.txHeldFees, send.txRejectedFees)
// + 49 onboarding-flow-UI paths (spec 014: onboarding.common branch + its 37
//   leaves, +10 onboarding.login leaves, +1 onboarding.create leaf)
// + 18 wallet-home paths (spec 015: 14 leaves, 4 branches — componentsUi
//   mainNav/dayGroup/commandBar/qrPlaceholder, networkFilter.pillAll,
//   receive.addressLabel, history bare labels + name-only subtitles).
// + 2 settings-domain leaves (spec 017: settings.signOut.keeps — what a
//   sign-out does NOT take with it — and settingsModals.network.rpcChainMismatch
//   — the RPC override refused for serving another chain).
// + 9 erase-this-device paths (spec 017: the `settings.eraseDevice` branch and
//   its 8 leaves — the destructive counterpart to sign-out, whose copy has to
//   name what is lost and what is not).
// + 1 send leaf (spec 017: send.warnCannotConvert — a fiat figure the screen
//   cannot restate in token units. `Continue` refuses it and the ⇅ row reads
//   `0 SYM`; before this key nothing on the screen said why, and the grep for
//   an existing amount/rate string turned up none that fit — `batchRateFailed`
//   and `batchNoPrice` both send you to a manual-rate field this screen has
//   not got).
// + 1 send leaf (spec 017: send.denomToggleNoRate — the ⇄ row shown but
//   inert, which is the one branch `warnCannotConvert` cannot cover: the
//   figure is in TOKEN units and resolves fine, so no amount warning fires,
//   and `warnCannotConvert`'s "switch to {{symbol}}" would tell the user to go
//   where they already are. The refusal was visible (the row dims); the reason
//   was not.
// + 21 contacts-UI leaves (spec 018: contacts.{manage,sectionContacts,
//   countPeople,membersCount,allContacts,addMember,batchSend,batchSendHint,
//   batchSendHintTitled,importFile,importAll,exportAll,importGroup,
//   exportGroup,groupRename,moveGroup,recentActivity,viewAllActivity,
//   deleteContact,actionQr,edit} — existing `contacts` branch, no new branches).
// + 30 net onboarding leaves (spec 019: the v2 create journey). 31 added —
//   welcome.{heroTitle,heroSubtitle}, the key screen's titles/subtitles/
//   counter/badges/CTAs, the three add methods, the progress screen's meter
//   and three task rows, the done screen's address label and identicon hint,
//   the desktop security-key sheet, and login.switchDeviceBtn — against 1
//   removed, `create.ack2`, when the acknowledgement gate went from four
//   boxes to two. The six `create.ack1`/`ack3*` moves are renames and net
//   zero, and no branch was added or removed.
// + 1 more onboarding leaf: `create.nameTitle`. The design's name screen is
//   titled 「给钱包起个名字」, which is not the flow's own label — a screen that
//   asks one question should say what the question is.
// Merged 017 + 018 + 019: the base's 1234 leaf + 78 branch, plus 017's 12
// leaves and its one `settings.eraseDevice` branch, plus 018's 21 contacts
// leaves, plus 019's 30. The three checks below are the arithmetic's witness —
// they fail loudly rather than let a merge invent a corpus.
// + 5 more onboarding leaves, `create.pin*`: the desktop is the only client
//   that speaks CTAP2 itself, so it is the only one that ever has to ask for a
//   security key's PIN. That dialog shipped reading `securityKeyRequiredTitle`
//   ("Plug in a security key") as its heading, which is a different sentence
//   about a different moment; these five are its own.
// + 3 more, `create.touch*`: the desktop is also the only client that has to
//   tell someone their key is BLINKING. Every other client hands the ceremony
//   to a system sheet that says so itself; here the app is the only thing on
//   screen, and shipping without these meant a person watched a spinner while a
//   key waited for a finger three feet away.
// + 3 more, `login.pick*`: a security key can hold several of one person's
//   wallets, and "who are you?" then has more than one answer. Every other
//   client gets that picker from the system sheet; here the app draws it, and
//   without it the first credential the key happens to return wins and the
//   others are unreachable from that computer.
// + 1 more, `create.touchSelectBody`: with two keys plugged in BOTH blink, and
//   the one the person touches is the one that gets used. "Touch it" is the
//   wrong sentence there — it has to say touch the ONE you want.
// + 2 more, `create.keyUnreadable*`: a key that is plugged in and cannot be
//   OPENED is a permissions problem wearing a hardware problem's clothes.
//   Reporting it as "no security key is plugged in" — which is what the code
//   did — sends a person looking at the port instead of at their udev rules.
// + 6 more, `create.step{Naming,Keys,Create}{Label,Detail}`: the desktop's v2
//   onboarding is two columns, and the left one names the step you are on and
//   what it decides. Those are not the screen titles — a rail that repeated
//   the H1 beside it would be saying everything twice — so they are their own
//   short pair per step. Desktop draws them today; the other clients have the
//   strings and can adopt the layout without a corpus change.
// + 1 more, `welcome.heroTitleFit`: the headline's type tier. Measured at the
//   shipped font, the widest authored line runs from 6.9em (zh) to 15.0em (id)
//   — a 2.2x spread that one font size cannot serve, so the tier rides with the
//   copy instead of being guessed per client.
// + 33 more, spec 021's receive / send / assets / addToken / scanner leaves:
//   the wallet-2 flows resolve almost entirely against keys the legacy React
//   Native app already left in the corpus. These are the remainder — the ones
//   the mocks say and nothing existing says: two add-token failure labels, the
//   receive network search and its two QR headlines, the share card's network
//   note, the assets "you received it but can't see it" card, and the send
//   flow's split / sweep / fee-token / import / receipt copy. No new branch:
//   every one hangs off a namespace that already exists.
// + 98 more (spec 022, explore + dApp signing): a whole new `explore.*`
//   namespace (54 keys — the browser home, tabs, the three sheets and the
//   browsing chrome) and 43 additions under `componentsUi.signing` for the
//   rungs of the ERC-7730 degradation ladder the 33 CS mocks walk down —
//   verified-ABI decode, 4byte best-effort, the un-simulatable case, the drain
//   reveal, Safe's inner call, deploy, and the slide-to-confirm labels. The
//   other ~95% of the signing copy was already here, which is why this is 43.
// + 3 more (spec 028, the web scanner's refusals): `componentsUi.scanner.
//   {noCamera,insecureOrigin,cameraUnavailable}`. Native never needed them —
//   a phone has a camera and an app has no origin — but a browser refuses in
//   three ways a person can act on differently, and a viewfinder that just
//   stays black tells them their camera is broken. The corpus was searched
//   first: `permissionText` covers the refusal that can be undone in site
//   settings, and nothing here said "there is no camera", "this page is not on
//   HTTPS" or "something else has the camera". No new branch — all three hang
//   off the existing `componentsUi.scanner`.
// + 2 more (spec 028 US5, the book as a file): `contacts.groupDeleteBody`
//   (deleting a group keeps its contacts — the confirm must SAY the rule the
//   core enforces) and `contacts.importDoneInvalid` (the importer counts rows
//   with no valid address; `importDoneBody` only had words for added/skipped,
//   so the third number had nowhere to go). No new branch.
// + 1 more (spec 028 US5, 6c): `contacts.batchSendNeedsMembers` — the group
//   send's disabled CTA had no words for WHY (an empty group); the founder
//   asked for the hint.
// + 1 more (spec 032 phase 25): `signing.amountUnknown`. When a token's
//   decimals cannot be verified the core no longer prints a number scaled by a
//   guess — 1 USDC came out as "0" on a signing sheet — and says nothing (an
//   em dash) instead. The corpus was searched first: `unverifiedTag`
//   ("Unverified") and `unverifiedWarning` describe the doubt but neither is a
//   value a row can carry, and the row the person reads is the amount itself.
//   No new branch — it hangs off the existing `componentsUi.signing`.
// + 1 more (issue #203): `send.recipientDuplicate` — a split row that pays an
//   address an earlier row already pays. The importer has said "Duplicate —
//   skipped" (`send.batchDup`) since it shipped, but that sentence is about a
//   row that WON'T be sent; these rows will be, exactly as asked for, so the
//   words beside them have to name which earlier recipient they repeat instead
//   of claiming a refusal that never happens. No new branch.
// + 2 more (spec 060, Arc): `addToken.nativeAliasTitle` / `…Message`. Arc's
//   native coin also answers an ERC-20 interface, so pasting that contract into
//   "add a token" finds a real, well-formed token — and adding it would show one
//   balance as two rows and double the person's total. "Not Found" would be the
//   wrong words for a refusal with a reason, so the refusal says what it is.
//   No new branch — it hangs off the existing `addToken`.
// + 4 more (spec 060, found in a real browser): the out-of-gas relayer sheet
//   now says WHO can fix it. On a network Vela ships, the operator runs that
//   relayer and the useful act is telling them (`operatorLead`, `reportBtn`),
//   with self-funding kept behind `selfFundToggle` so nobody is nudged into
//   paying for something that is not theirs to pay for. On a network the
//   person added — a local devnet, an internal chain — the operator may have
//   no way to hold gas there at all, and `customLead` says so.
// + 9 more (spec 068, the fee you can refresh at a speed you can choose): the
//   send form's fee row grew a refresh control (`send.feeRefresh`), a calm
//   line for a quote that is merely old rather than broken (`send.feeStale`),
//   and a folded speed control (`send.feeSpeedLabel`, plus `send.feeSpeedOnce`
//   which says out loud that a per-transaction pick is one-shot and does not
//   rewrite the stored default). Settings gained the stored default itself:
//   `settings.advanced.feeSpeedTitle` / `…Subtitle` for the row, and ONE new
//   branch, `settings.feeSpeed`, carrying the sheet's own `title` /`subtitle`.
//   The tier NAMES were already here (`send.gasTier.*`) and only changed
//   VALUE, so they are not counted above.
// + 3 more (spec 068, the owner's ruling on the speed picker):
//   `send.gasTierHint{Fast,Standard,Slow}`. The heading over those options
//   asks about SPEED, so every option has to BE a speed — naming the slow one
//   "Economy" / 经济便宜 mixed a speed scale with a value judgement and read
//   as a joke. So the NAME went back to the speed (`slow` → Slow / 较慢, a
//   value-only change again) and what that speed BUYS moved to a description
//   line under it, which is the thing that makes the cheap tier attractive.
//   Three FLAT leaves rather than a `send.gasTierHint` branch on purpose: a
//   branch would move the branch pin as well for nothing, and `rapid` — the
//   dead fourth tier nothing offers — must not acquire a description it would
//   then need translating in 15 locales. No new branch.
// 1674 → 1676 (issue 686, 2026-09-21): +2 flat send leaves — `feeSpeedFree`
//   (the line under the folded speed control when the fastest speed costs no
//   more than the person's slower default, so this send takes it) and
//   `feeSpeedSingle` (the statement that replaces the options on a network
//   whose speeds nothing tells apart, such as Tempo). Flat, beside
//   `feeSpeedOnce`, for the same reason as the hints above. No new branch.
// + 11 more (issues 204-206, the multi-recipient send): what the split form and
//   its importer did without saying, and what the person had no way to ask for.
//   `send.batchUnitCaption` names the first choice on the importer;
//   `send.batchTokenHint` — in token mode nothing is converted and the sheet's
//   figures ARE the amounts (the template's 5000 is five thousand coins);
//   `send.batchAddsToRows` / `send.batchReplacesRows` say what importing does to
//   the people already on the form, and `send.batchReplaceInstead` /
//   `send.batchAddInstead` are the way to choose the other; `send.badAmount` is a
//   refused line's or a row's reason beside the existing "Invalid address";
//   `send.splitNeedsAddress` / `send.splitNeedsAmount` name the row a dark
//   Continue is waiting on (`{{n}}`, not `{{count}}`: it is an ordinal);
//   `send.splitRemaining` is what is left to give out; `send.splitFillEmpty`
//   puts one amount in every empty row. Everything ELSE those issues needed was
//   already here, in all fifteen locales, unread. No new branch.
// 1687 (merge of the two above, 2026-09-21): spec 068 and issue 686 added
//   +13 leaf and the one `settings.feeSpeed` branch; issues 204-206 added +11
//   leaf and no branch. The two sets share no path, so they simply add:
//   1662 + 14 + 11 = 1687 = (1575 + 13 + 11) leaf + (87 + 1) branch.
// 1692 (spec 070, 2026-09-22): +5 flat explore leaves — the in-app browser
//   now survives a renderer death and says so (`pageCrashedTitle`,
//   `pageCrashedBody`), its scan button routes every code and names the two it
//   cannot use (`walletConnectUnsupported`, `scanUnrecognized`), and "Copy
//   link" confirms itself (`linkCopied`). Everything else the browser's fixes
//   needed was already in the corpus (`connect.browser.loadFailed`, `.retry`,
//   `.a11yInsecure`, `explore.disconnect`). No new branch.
// 1713 (spec 071, 2026-09-22): the Clear Signer — the fourth "Sign with".
//   +10 flat `componentsUi.signing.clearSigner*` leaves (its name, the promise
//   under it, waiting + the Local Network Access hint, closed / refused /
//   mismatch / timeout, reopen, the desktop tab's "signed, close me") and the
//   `settings.signing` branch with 10 leaves (the default "Sign with" row, the
//   signer page row, its three refusals, reset, save). 1692 + 20 + 1 = 1713.
// 1717 (spec 072, 2026-09-22): +4 `settingsModals.endpoints.reset*` leaves —
//   resetting the service endpoints is destructive (FR-010) and asks first on
//   every shell; the corpus had the button and no question. No new branch.
if (PATHS.length !== 1717) fail(`expected 1717 paths (1628 leaf + 89 branch), got ${PATHS.length}`);
if (leafSet.size !== 1628) fail(`expected 1628 leaf paths, got ${leafSet.size}`);
if (branchSet.size !== 89) fail(`expected 89 branch paths, got ${branchSet.size}`);

/** Pack a bit-per-path bitmap, LSB first within each byte. */
function packBits(bits) {
  const out = new Uint8Array(Math.ceil(bits.length / 8));
  bits.forEach((b, i) => { if (b) out[i >> 3] |= 1 << (i & 7); });
  return out;
}

const branchBitmap = packBits(IS_BRANCH);

/** Rust string literal — escape only what Rust requires. */
function rustStr(s) {
  return `"${s.replace(/\\/g, '\\\\').replace(/"/g, '\\"').replace(/\n/g, '\\n').replace(/\r/g, '\\r').replace(/\t/g, '\\t')}"`;
}

function byteArray(bytes, perLine = 16) {
  const lines = [];
  for (let i = 0; i < bytes.length; i += perLine) {
    lines.push('    ' + [...bytes.slice(i, i + perLine)].map((b) => `0x${b.toString(16).padStart(2, '0')}`).join(', ') + ',');
  }
  return lines.join('\n');
}

const GENERATED_BY = 'GENERATED by scripts/gen-i18n.mjs — do not edit by hand.';

mkdirSync(dirname(PATHS_FILE), { recursive: true });
writeFileSync(PATHS_FILE, `//! ${GENERATED_BY}
//!
//! The SHARED key-path table: every dotted path in the corpus, sorted, interned
//! once for all ${LOCALES.length} locales. Regenerate with \`node scripts/gen-i18n.mjs\`.
//!
//! ${PATHS.length} paths = ${leafSet.size} leaf + ${branchSet.size} branch. Repeated per locale these key bytes
//! would cost ${[...leafSet].reduce((n, p) => n + p.length, 0) * LOCALES.length} bytes; interned once they cost ${PATHS.reduce((n, p) => n + p.length, 0)}.

/// Every path in the corpus, strictly sorted. Lookup is a binary search here, then
/// an O(1) index into the active locale's value table.
pub(crate) static PATHS: [&str; ${PATHS.length}] = [
${PATHS.map((p) => `    ${rustStr(p)},`).join('\n')}
];

/// Bit *i* is set when \`PATHS[i]\` is an object node rather than a translation.
/// A branch is a distinct lookup outcome, not a miss: \`t("home")\` must return the
/// byte-exact diagnostic \`key 'home (en)' returned an object instead of string.\`,
/// which a flat map could never distinguish from an absent key.
pub(crate) static IS_BRANCH: [u8; ${branchBitmap.length}] = [
${byteArray(branchBitmap)}
];

/// Number of entries in [\`PATHS\`]. Value tables carry \`N_PATHS + 1\` offsets.
pub(crate) const N_PATHS: usize = ${PATHS.length};

/// Index of \`path\` in [\`PATHS\`], or \`None\`.
pub(crate) fn path_id(path: &str) -> Option<usize> {
    PATHS.binary_search(&path).ok()
}

/// Whether \`PATHS[id]\` is an object node.
pub(crate) fn is_branch(id: usize) -> bool {
    IS_BRANCH[id >> 3] & (1 << (id & 7)) != 0
}
`);

// ---------------------------------------------------------------------------
// Stage 2 — one value table per locale
// ---------------------------------------------------------------------------

const pathIndex = new Map(PATHS.map((p, i) => [p, i]));

function flatten(obj, prefix, out) {
  for (const k of Object.keys(obj)) {
    const v = obj[k];
    const p = prefix ? `${prefix}.${k}` : k;
    if (v && typeof v === 'object' && !Array.isArray(v)) flatten(v, p, out);
    else out.set(p, v);
  }
  return out;
}

mkdirSync(CATALOG_DIR, { recursive: true });
const stats = [];
/** locale -> 2 or 4, the byte width its OFFSETS array is emitted at. */
const OFFSET_WIDTHS = new Map();

for (const lng of LOCALES) {
  const flat = flatten(bundles[lng], '', new Map());

  // Build the blob in PATHS order so the offset array is dense and monotonic.
  let blob = '';
  const offsets = [0];
  const present = new Array(PATHS.length).fill(0);
  for (let i = 0; i < PATHS.length; i++) {
    const v = flat.get(PATHS[i]);
    if (v !== undefined) {
      blob += v;
      present[i] = 1;
    }
    offsets.push(Buffer.byteLength(blob, 'utf8'));
  }

  const blobBytes = Buffer.byteLength(blob, 'utf8');
  // Offset width is chosen PER LOCALE, not once for the corpus. u16 everywhere
  // was the original call because u32 everywhere puts 64-bit ja+en at 135,992 —
  // 647 bytes OVER the SC-005 budget — while u16 lands it at 131,168 on every
  // pointer width. That reasoning is about ja+en, the two locales SC-005
  // measures, and it still holds: they stay u16 and the budget is untouched.
  //
  // What it never covered is `ru`, whose blob passed 64 KiB when spec 017 added
  // the erase-this-device copy (65,115 bytes before it, 420 to spare). Pinning
  // every locale to the widest one's need is what forced that choice; picking
  // per locale costs the 2,648 extra bytes only where they are needed — and the
  // only build that compiles a catalog in at all is the desktop app, which
  // takes `i18n-all` precisely because it has no size budget. The web build
  // compiles ZERO locales (runtime JSON, FR-015), so it ships none of these
  // arrays.
  //
  // Still fail loudly past u32: a wrapped offset would slice a value in half.
  const offsetWidth = blobBytes >= 65_536 ? 4 : 2;
  if (blobBytes >= 4_294_967_296) {
    fail(`${lng}: value blob is ${blobBytes} bytes, which does not fit u32 offsets.`);
  }
  if (offsets.length !== PATHS.length + 1) fail(`${lng}: expected ${PATHS.length + 1} offsets, got ${offsets.length}`);

  const presentBitmap = packBits(present);
  const leafCount = present.reduce((a, b) => a + b, 0);
  stats.push({ lng, leafCount, blobBytes, tableBytes: blobBytes + offsets.length * offsetWidth + presentBitmap.length });
  OFFSET_WIDTHS.set(lng, offsetWidth);

  writeFileSync(join(CATALOG_DIR, `${modOf(lng)}.rs`), `//! ${GENERATED_BY}
//!
//! Compiled-in value table for \`${lng}\` — ${leafCount} translations, ${blobBytes} blob bytes.
//! Gated by the \`${featureOf(lng)}\` cargo feature; the DEFAULT feature set is zero
//! locales, because all 15 compiled in measured 1,315,023 wasm bytes against the
//! 1,000,000 ceiling at rust/scripts/build-web.mjs:42.

/// Every translation for this locale, concatenated in \`PATHS\` order.
pub(super) static BLOB: &str = ${rustStr(blob)};

/// \`BLOB[OFFSETS[i]..OFFSETS[i + 1]]\` is the value for \`PATHS[i]\`, when present.
/// The width is per locale — \`u16\` while the blob fits 64 KiB, \`u32\` beyond it.
/// See the residency-budget note in catalog.rs.
pub(super) static OFFSETS: [u${offsetWidth * 8}; ${offsets.length}] = [
${(() => { const l = []; for (let i = 0; i < offsets.length; i += 16) l.push('    ' + offsets.slice(i, i + 16).join(', ') + ','); return l.join('\n'); })()}
];

/// Bit *i* is set when this locale defines \`PATHS[i]\`. Distinguishes "absent, fall
/// through to en" from "present and empty", which are different renderings.
pub(super) static PRESENT: [u8; ${presentBitmap.length}] = [
${byteArray(presentBitmap)}
];
`);
}

// The module file, gating each locale behind its feature.
writeFileSync(join(CATALOG_DIR, 'mod.rs'), `//! ${GENERATED_BY}
//!
//! One compiled-in value table per locale, each behind its own cargo feature so a
//! build carries only the languages it ships (FR-014). Locales not compiled in are
//! still reachable at runtime through \`Catalog::from_json\` (FR-015), which is the
//! route the web build uses for all of them.

${LOCALES.map((l) => `#[cfg(feature = "${featureOf(l)}")]\npub(crate) mod ${modOf(l)};`).join('\n')}

/// The compiled-in table for \`lang\`, if its feature is enabled.
///
/// Returns \`(blob, offsets, present)\`. The offset width is per locale — see
/// \`StaticOffsets\` and the residency-budget note in catalog.rs.
pub(crate) fn embedded(
    lang: &str,
) -> Option<(
    &'static str,
    crate::i18n::catalog::StaticOffsets,
    &'static [u8],
)> {
    match lang {
${LOCALES.map((l) => `        #[cfg(feature = "${featureOf(l)}")]\n        "${l}" => Some((\n            ${modOf(l)}::BLOB,\n            crate::i18n::catalog::StaticOffsets::U${OFFSET_WIDTHS.get(l) * 8}(&${modOf(l)}::OFFSETS),\n            &${modOf(l)}::PRESENT,\n        )),`).join('\n')}
        _ => None,
    }
}
`);

// ---------------------------------------------------------------------------
// Stage 4 — one merged JSON document per locale, for on-demand loading (FR-014)
// ---------------------------------------------------------------------------
//
// This is what `Catalog::from_json` consumes and what the web route fetches. It
// exists because compiling catalogs into the wasm is measurably the wrong trade:
// all 15 came to 1,315,023 bytes against a 1,000,000 ceiling, and even ONE locale
// costs more over the wire compiled in (+31,862 brotli'd) than fetched as plain
// JSON (15,353). Fetching also delivers the actual requirement — a Japanese user
// downloads `ja`, not a 15-locale blob.
//
// Emitted with the same `JSON.stringify(doc, null, 1)` convention as the vector
// corpus, so a diff is reviewable rather than a single reflowed line.

mkdirSync(ASSET_DIR, { recursive: true });
const assetStats = [];
for (const lng of LOCALES) {
  const file = join(ASSET_DIR, `${lng}.json`);
  writeFileSync(file, `${JSON.stringify(bundles[lng], null, 1)}\n`);
  assetStats.push({ lng, bytes: Buffer.byteLength(readFileSync(file)) });
}

// A stale asset is a wrong translation shipped to production, so prove the merge
// round-trips before anyone trusts it: every asset must reparse to exactly the
// bundle it came from.
for (const lng of LOCALES) {
  const back = readJson(join(ASSET_DIR, `${lng}.json`));
  if (JSON.stringify(back) !== JSON.stringify(bundles[lng])) {
    fail(`${lng}: emitted asset does not round-trip back to the merged bundle`);
  }
}

// ---------------------------------------------------------------------------
// Stage 5 — day-period and weekday tables (FR-021)
// ---------------------------------------------------------------------------
//
// Extracted from the generating machine's ICU rather than transcribed by hand.
// Only 518 bytes of strings, but the house rule from feature 003's FR-009 applies
// regardless: mechanically generated data can be re-derived and diffed, whereas a
// hand-typed Turkish "Çar" or Vietnamese "Thứ 4" is a silent wrong-day bug nobody
// reviews. Requires a full-ICU node — asserted, not assumed.

if (!process.versions.icu || Intl.DateTimeFormat.supportedLocalesOf(['ru']).length !== 1) {
  fail('this Node lacks full ICU — the day-period and weekday tables would be wrong');
}

const dtRows = LOCALES.map((lng) => {
  const hourFmt = new Intl.DateTimeFormat(lng, { hour: 'numeric', minute: '2-digit', hour12: true, timeZone: 'UTC' });
  const partsPm = hourFmt.formatToParts(new Date(Date.UTC(2026, 5, 13, 21, 5)));
  const partsAm = hourFmt.formatToParts(new Date(Date.UTC(2026, 5, 13, 9, 5)));
  const pm = partsPm.find((p) => p.type === 'dayPeriod')?.value ?? 'PM';
  const am = partsAm.find((p) => p.type === 'dayPeriod')?.value ?? 'AM';
  // Position matters: zh/zh-TW/zh-HK/ja/ko/tr write the marker BEFORE the hour,
  // which today's hardcoded English `AM`/`PM` suffix gets wrong for six locales.
  const di = partsPm.findIndex((p) => p.type === 'dayPeriod');
  const hi = partsPm.findIndex((p) => p.type === 'hour');
  const periodFirst = di >= 0 && hi >= 0 && di < hi;

  const wkFmt = new Intl.DateTimeFormat(lng, { weekday: 'short', timeZone: 'UTC' });
  // 2026-06-07 is a Sunday, so index 0 is Sunday — matching `Date.getDay()`.
  const weekdays = Array.from({ length: 7 }, (_, i) => wkFmt.format(new Date(Date.UTC(2026, 5, 7 + i))));
  return { lng, am, pm, periodFirst, weekdays };
});

writeFileSync(DATETIME_FILE, `//! ${GENERATED_BY}
//!
//! Day-period markers and short weekday names for the ${LOCALES.length} shipped locales,
//! extracted from ICU ${process.versions.icu}. Regenerate with \`node scripts/gen-i18n.mjs\`.
//!
//! This closes the last two host-\`Intl\` dependencies on the formatting path: the
//! hardcoded English \`AM\`/\`PM\` in \`locale-format.ts\` (wrong in WORDING for 8 locales
//! and in POSITION for 6) and \`activity.ts\`'s \`toLocaleDateString\` weekday lookup,
//! which is unreliable on Hermes for exactly the reason the plural rules were.

/// \`(locale, am, pm, period_before_hour, [Sun..Sat])\`.
pub(crate) static DATETIME: [(&str, &str, &str, bool, [&str; 7]); ${dtRows.length}] = [
${dtRows.map((r) => `    (${rustStr(r.lng)}, ${rustStr(r.am)}, ${rustStr(r.pm)}, ${r.periodFirst}, [${r.weekdays.map(rustStr).join(', ')}]),`).join('\n')}
];

/// Row for \`locale\`, falling back to \`en\` — the same fallback the resolver uses.
pub(crate) fn row(locale: &str) -> &'static (&'static str, &'static str, &'static str, bool, [&'static str; 7]) {
    let primary = locale.split(['-', '_']).next().unwrap_or(locale);
    DATETIME
        .iter()
        .find(|r| r.0 == locale)
        .or_else(|| DATETIME.iter().find(|r| r.0 == primary))
        .unwrap_or(&DATETIME[0])
}
`);

// ---------------------------------------------------------------------------
// Canonical formatting
//
// The generated Rust must come out exactly as `cargo fmt` would leave it, or the
// two CI gates contradict each other: `cargo fmt --all --check` demands one
// shape, and `gen:i18n` + `git diff --exit-code` demands the other, so whichever
// runs second is red and neither is fixable without breaking the other. Emitting
// canonical output here is the only arrangement where both can pass.
// ---------------------------------------------------------------------------

const RUST_OUTPUTS = [
  PATHS_FILE,
  DATETIME_FILE,
  join(CATALOG_DIR, 'mod.rs'),
  ...LOCALES.map((lng) => join(CATALOG_DIR, `${modOf(lng)}.rs`)),
];

try {
  execFileSync('rustfmt', ['--edition', '2021', ...RUST_OUTPUTS], { stdio: 'pipe' });
} catch (e) {
  console.error(
    'gen-i18n: rustfmt failed on the generated Rust. It is required, not optional —\n' +
      'without it `cargo fmt --all --check` and the generated-artefact diff gate\n' +
      'disagree and CI cannot be green.\n' +
      String(e.stderr ?? e),
  );
  process.exit(1);
}

// ---------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------

const keyBlobBytes = PATHS.reduce((n, p) => n + p.length, 0);
console.log(`paths.rs   : ${PATHS.length} paths (${leafSet.size} leaf + ${branchSet.size} branch), key blob ${keyBlobBytes} bytes, branch bitmap ${branchBitmap.length} bytes`);
for (const s of stats) {
  console.log(`  ${s.lng.padEnd(6)} ${String(s.leafCount).padStart(5)} values  blob ${String(s.blobBytes).padStart(6)}  table ${String(s.tableBytes).padStart(6)} bytes`);
}
const en = stats.find((s) => s.lng === 'en');
const ja = stats.find((s) => s.lng === 'ja');
const shared = keyBlobBytes + branchBitmap.length + PATHS.length * 8; // +ptr array (wasm32)
console.log(`\nSC-005 check — ja + en resident, shared table included:`);
console.log(`  shared ${shared} + ja ${ja.tableBytes} + en ${en.tableBytes} = ${shared + ja.tableBytes + en.tableBytes} bytes (budget 135,345 for the per-locale halves)`);
console.log(`  per-locale halves only: ${ja.tableBytes + en.tableBytes} bytes`);
const assetTotal = assetStats.reduce((n, a) => n + a.bytes, 0);
const assetEn = assetStats.find((a) => a.lng === 'en').bytes;
const assetJa = assetStats.find((a) => a.lng === 'ja').bytes;
console.log(`assets     : ${LOCALES.length} files, ${assetTotal} bytes total -> ${relative(REPO_ROOT, ASSET_DIR)}/`);
console.log(`  on-demand: a ja reader fetches ${assetJa} + ${assetEn} = ${assetJa + assetEn} bytes, not ${assetTotal}`);
const dtBytes = dtRows.reduce((n, r) => n + Buffer.byteLength(r.am) + Buffer.byteLength(r.pm) + r.weekdays.reduce((m, w) => m + Buffer.byteLength(w), 0), 0);
console.log(`datetime   : ${dtRows.length} locales, ${dtBytes} bytes of strings (ICU ${process.versions.icu}) -> ${relative(REPO_ROOT, DATETIME_FILE)}`);
