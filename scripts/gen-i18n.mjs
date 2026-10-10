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
//   +10 flat `componentsUi.signing.trustedSigner*` leaves (its name, the promise
//   under it, waiting + the Local Network Access hint, closed / refused /
//   mismatch / timeout, reopen, the desktop tab's "signed, close me") and the
//   `settings.signing` branch with 10 leaves (the default "Sign with" row, the
//   signer page row, its three refusals, reset, save). 1692 + 20 + 1 = 1713.
// 1717 (spec 072, 2026-09-22): +4 `settingsModals.endpoints.reset*` leaves —
//   resetting the service endpoints is destructive (FR-010) and asks first on
//   every shell; the corpus had the button and no question. No new branch.
// 1733 (spec 075, 2026-09-22): the Clear Signer across devices. +10
//   `componentsUi.signing.trustedSigner*` leaves (where the signer is — this
//   device or another — the pairing sheet, its waiting line, the six-digit
//   code and its confirmation, copy link, the tunnel unreachable) and +6
//   `settings.signing.tunnel*` (the Settings row, as the page row). No new
//   branch. (The service was called the relay until 2026-09-23, when the owner
//   renamed it the tunnel — passkeys' own word for the same thing. The six keys
//   and `trustedSignerTunnelDown` were renamed with it; the BUNDLER keeps `relay`
//   in `componentsUi.gas.relayerFee`, `componentsUi.treasuryBootstrap.*` and
//   `settingsModals.endpoints.bundler*`.)
// 1738 (spec 075, 2026-09-22): the Bluetooth route. +5
//   `componentsUi.signing.trustedSigner*` leaves — the third row in "where is
//   your Clear Signer", what it means (and that the app must stay open), the
//   NAME to look for in the browser's device list, and the two ways Bluetooth
//   can be unavailable (permission refused, radio off). The peripherals
//   shipped borrowing the dApp flow's "Bluetooth permission is needed", which
//   never said which device to pick. No new branch.
// 1739 (spec 075, 2026-09-22): +1 `trustedSignerBluetoothUnsupported`. A device
//   with no peripheral role at all was being told to switch Bluetooth on,
//   which cannot help it (iOS, T041: `unsupported` and `unavailable` had
//   nowhere else to go).
// 1740 (spec 075, 2026-09-22): +1 `trustedSignerNearbyLost`. A pairing that
//   worked and then dropped was borrowing "this device cannot pair this way",
//   which is false and sends the person looking for the wrong problem (found
//   on the radio, T043).
// 1742 (spec 075, 2026-09-23): +2 `onboarding.create.methodBlocked{Hint,
//   Signer}`. A wallet's keys all belong to ONE relying party, because the
//   registry files a unit under one `rpId` — so once the first key is minted
//   the routes that would mint for a different party are off. They used to be
//   offered and failed at the publish, with nothing said (owner, 2026-09-23).
//   Two strings, not three: a dimmed row keeps its own caption, and the third
//   would have said "off" where the dimming already does (SC-005 budget).
// 1745 (2026-09-23): +3 — `settings.account.remove{,Body}` and
//   `settings.signOut.descMany`. A device holding six wallets could sign out
//   of all six or none, and the dialog's copy was true of one and quietly
//   false of six (owner: 「有时候不想退出所有，只想退出单个」).
// 1722 (spec 075, 2026-09-23): −23. The Clear Signer's two CROSS-DEVICE
//   channels went, and every word they needed went with them: "where is your
//   Clear Signer?" and its three answers, the pairing link and its code, the
//   four Bluetooth troubles, and the tunnel's own Settings row. The owner cut
//   them because only a page THIS device fetched can be checked against what
//   it is supposed to be — 「客户端支持回环 + 蓝牙就够了」, then 「我确定砍掉
//   蓝牙」. A shrinking ledger is as load-bearing as a growing one: a string
//   nothing draws is a string nobody notices going wrong.
// 1691 (spec 081, 2026-09-22): the self-call guard's blocked sheet needs four
//   leaves under the existing `componentsUi.signing` branch —
//   `selfCallBlockedTitle`, `...Body`, `...LegBody` (the batch wording, with
//   the 1-based step) and `...SafeTx` (the typed-data case, which names no
//   function). No new branch: 1687 + 4 = 1691 = 1603 leaf + 88 branch.
// 1692 (spec 081, FR-009): + `settingsModals.addNetwork.singleKeyOnly`. The
//   network check now asks for Safe's passkey signer factory too, which only a
//   wallet with more than one key needs — so a chain can be genuinely usable
//   and still refuse such a wallet. One sentence for that state; without it a
//   person reads "Compatible" beside two red crosses. 1691 + 1 = 1692.
// 1693 (spec 081, FR-008): + `componentsUi.signing.descriptorFetchedWarning`.
//   A descriptor fetched from the chain-data service was shown as "verified"
//   with nothing having authenticated it. Now only a built-in descriptor — or
//   a fetched one byte-equal to the built-in copy — earns that word, and this
//   is the sentence the other case needs. 1692 + 1 = 1693.
// MERGE (2026-09-24): main and 075 both counted up from 1687 and arrived at
//   different totals — main at 1693 (spec 081's four blocked-sheet leaves, the
//   single-key network sentence, the fetched-descriptor warning), 075 at 1722
//   (the Trusted Signer, then −23 as its cross-device channels went). The
//   corpus is the UNION of both, so the total is neither, and the number below
//   is the merged corpus's own — not a guess, and not either side's.
// 1729 (spec 078, 2026-09-26): + `common.gotIt`. An alert's acknowledgement
//   ("知道了" / "Got it") had no word of its own: Android and the desktop
//   borrowed the receipt's "Done", and iOS hard-coded English "OK" in every
//   language. One leaf under the existing `common` branch: 1728 + 1 = 1729.
// 1730 (082, 2026-09-26): + `settingsModals.keys.signsHere` — the keys list
//   marks the key this device signs with ("当前登录" / "Signed in"). One leaf
//   under the existing `settingsModals.keys` branch: 1729 + 1 = 1730.
// 1740 (078, 2026-09-26): + ten `componentsUi.bugReport.*` leaves — the
//   report takes up to five screenshots (label, add, hint, the public-on-
//   GitHub warning, remove, limit, unsupported, the web drop hint), and says
//   when they could not be uploaded or carried into the GitHub form. All under
//   the existing `componentsUi.bugReport` branch: 1730 + 10 = 1740.
// 1743 (078, 2026-09-27): + `componentsUi.bugReport.{viewScreenshot, closeViewer,
//   removeFromViewer}` — a screenshot tile opens a preview (founder: "上传的截图要
//   能点击放大预览"): the tile's a11y label, the viewer's close and its remove
//   button. Same branch: 1740 + 3 = 1743.
// 1744 (078, 2026-09-27): + `settings.sections.community` — the settings
//   group holding the official X / Telegram / Discord links (founder). The
//   rows are brand names and handles, never translated. 1743 + 1 = 1744.
// 1773 (078, 2026-09-27): + 29 `componentsUi.signing.{intent*,label*,valueUnlimited}`
//   leaves — the clear-signing sheet showed descriptor words ("Approve",
//   "Amount", "Spender", "Unlimited") in English on a Chinese sheet (founder).
//   The core names each one (`clear_signing::ClearTerm`, whose serialized name
//   IS the leaf) and the shells translate it; the 19 words the old app already
//   had are reused, these are the rest. Same branch: 1744 + 29 = 1773.
// 1782 (079, 2026-09-28): + 5 `explore.{load{Offline,NotFound,Certificate,
//   Retrying},chainDown}` and 4 `componentsUi.signing.{stillConfirming,
//   unknownOutcome,openSigner,signerDown}` — the browser says why a page did not open and that it
//   is retrying, names an unreachable chain, and the signing sheet says what
//   happens after the signature (owner's device pass on the Xiaomi). "Signed",
//   "Submitting" and the generic load failure reuse `signHandoff.signed`,
//   `send.txSubmitting` and `connect.browser.loadFailed`; offline, timeout and
//   refused share one sentence (to a person all three are "the network, retrying").
//   That is what keeps ja + en inside SC-005, which main left ~900 bytes of.
//   Same branches: 1773 + 9 = 1782.
// 1784 (082, 2026-09-28): + `componentsUi.signing.maybeSent` — a submit whose
//   reply was lost says the payment may have gone and not to send it again
//   (owner ruling 1; G21: a landed payment was reported "failed — try again"), +
//   `explore.loadProxy` (the proxy itself cannot be reached, ruling 3) and
//   `explore.requestOpen` (why a tab switch is held, ruling 10); − the unread
//   `send.txErrorTimeout`, whose "submitted but timed out" is what the tracker
//   rules forbid. `componentsUi.signing.simUnavailableWarning` is reworded
//   shorter ("couldn’t check", FR-012). Net −21 bytes of ja + en, so SC-005
//   holds without raising its cap. Same branches: 1782 + 3 − 1 = 1784.
// 1785 (082 round 2, 2026-09-29): + `componentsUi.signing.refused` — a
//   transaction the relay refused says so, and that nothing was sent, with no
//   "try again" (RJ3; G36: a refused op was answered ok and the sheet said
//   失败 · 请重试). Paid for by trimming `componentsTx.receipt.failedHint` to
//   its first two sentences: the explorer link sits right under it, and "go
//   back and try again" is wrong after a revert (RJ6). The zh / zh-TW
//   `send.txBackgroundHint` gain their comma (0 B of ja + en). Net ≈ −58 B of
//   ja + en, SC-005 unraised. Same branches: 1784 + 1 = 1785.
// 1786 (issue 333, 2026-10-01): + `contacts.importFailEncoding` — a contact
//   file that is not UTF-8 (a GBK CSV from Excel on Chinese Windows) is refused
//   with how to save it, where every shell used to import its names as U+FFFD.
//   Same branches: 1785 + 1 = 1786.
// 1788 (spec 090, 2026-10-02): + `receive.includeNetwork` and
//   `receive.includeNetworkHint` — the receive code's opt-in "include network"
//   switch (ERC-681) and the calm line under it: some wallets cannot read
//   that code. Same branches: 1786 + 2 = 1788.
// 1791 (091, 2026-10-02): + `about.{debugMode,debugModeBody,debugModeRevealed}`
//   — the hidden developer switch in About (seven taps on the version reveal
//   it): its name, the one line saying what it does and that it is for
//   development only, and the notice when it appears. The ja + en residency
//   budget moves to 139,800 (owner, 2026-10-02). Same branches: 1788 + 3 = 1791.
// 1797 (092, 2026-10-02): the home line over networks the wallet cannot reach
//   says so without "RPC" (`assets.unreachable{One,Many}`), and the list behind
//   it names them all under that same line (`assets.unreachable{Body,None}`), each with what was
//   last read there (`assets.{lastSeen,lastSeenUnpriced,lastSeenEmpty,notReadYet}`);
//   − `assets.rpcUnavailable{Single,Multiple}`. The ja + en residency budget
//   moves to 140,800 (owner, 2026-10-02). Same branches: 1791 + 8 − 2 = 1797.
// 1798 (093, 2026-10-02): + `history.dappRowTitle` — every dApp interaction is
//   an Activity row titled "{{intent}} on {{place}}" (在 {{place}} {{intent}});
//   every other word it needs is reused. Same branches: 1797 + 1 = 1798.
// 1803 (094, 2026-10-02): + the extension's own notices, branch `connect.ext`
//   with `{installedNote, accessNote, accessAllow}` — a fresh install asks for
//   the tabs open before it to be reloaded, and limited site access is said in
//   plain words with its one-click grant — and `onboarding.common.siteAccessBody`,
//   the passkey Chrome refused for that reason (was Chrome's raw SecurityError).
//   1798 + 4 leaves + 1 branch = 1803 (1713 leaf + 90 branch).
// 1824 (095, 2026-10-02): + `about.{linkPrivacy,linkTerms,linkSupport}` —
//   Settings → About links the privacy policy, terms and support page on every
//   shell (App Review 5.1.1(i): the policy reachable outside onboarding; the
//   onboarding's `ack2PrivacyPolicy`/`ack2Terms` are inflected for their
//   sentence, e.g. ru accusative, so not reused) — and the 17 leaves of the new
//   `componentsUi.appMenu` branch: the macOS menu bar (About, Edit, View,
//   Window), which was English-only. 1803 + 3 + 17 leaves + 1 branch = 1824.
// 1826 (096 C, 2026-10-02): + `componentsUi.gas.feeCoinSpent` — the fee coin a
//   person chose is one the transaction itself may spend, so too little may be
//   left for the fee — and `send.recipientTokenContract`, said before the
//   slide when the recipient is a token's own contract. The ja + en residency
//   budget moves to 141,800 (owner, 2026-10-02). Same branches: 1824 + 2 = 1826.
// 1829 (096 B, 2026-10-02): + `componentsUi.signing.{labelOrder,valueAll,
//   warnOrderTerms}` — a CoW order named by its id, `type(uint256).max` read as
//   "All", and the plain caution that a pre-signed order's amounts are not on
//   the sheet. Same branches: 1826 + 3 = 1829.
// 1841 (098, 2026-10-03): + `componentsUi.relayUnreachable.{title,
//   operatorLead,customLead,settingsHint,reportBtn,retryBtn,closeBtn}` — the
//   sheet for a relay that cannot serve the chain, which used to send through
//   and fail after the passkey — and `componentsUi.treasuryBootstrap.
//   {balanceLine,networkLine,watching,qrLabel}` for the funding sheet that now
//   closes by itself. 11 leaves, 1 branch: 1829 + 12 = 1841.
// 1843 (098 §5.1, 2026-10-03): + `settingsModals.network.relayNotice` and
//   `settingsModals.rpcProviders.relayNotice` — said where an RPC or a provider
//   key is set: the relay is sent the RPC the wallet uses, key and all. Same
//   branches: 1841 + 2 = 1843.
// 1848 (098 follow-up, 2026-10-03): + `componentsUi.gas.reason{Quote,FeeToken,
//   Simulation,QuoteHigh}` — the fee row says which relay failure it was
//   instead of "check your connection" for all four — and `send.txRelayFunding`,
//   said while the relay tops up its gas. Minus `componentsUi.funding.
//   denialNetworkError` (the line those four replace) and `componentsUi.
//   treasuryBootstrap.networkLine` (098's, read by no shell). Same branches:
//   1843 + 5 - 2 = 1846.
// 1820 (same day): - the other 26 `componentsUi.funding.*` strings of the
//   retired gas-account funding sheet (title, lead, the denial* reasons, …),
//   read by no shell since the treasury sheet replaced it (098 §4) — the room
//   the five lines above needed under SC-005, instead of a budget move. The
//   four still read (cancel, showQr, autoCheckNote, checking) stay. Same
//   branches: 1846 - 26 = 1820.
// 1876 (099, 2026-10-04): the dApp browser's layers in words — 41 leaves under
//   a new `componentsUi.browserStatus` branch (with its `page`, `provider` and
//   `reason` branches), 9 under a new `componentsUi.signing.confirmBlock`
//   branch (why the slide is shut), and `send.txRelaySending` (the landing
//   before the relay has sent): 1728 + 51 = 1779 leaves, 92 + 5 = 97 branches.
// 1876 (issue #409, 2026-10-04): `onboarding.create.successMessage` said "Any
//   of your 1 keys" to a one-key wallet. It is now plural by `{{count}}` —
//   `successMessage_{one,other}`, plus `_many` (es-MX, pt-BR, fr, it, ru) and
//   `_few` (ru): 4 paths for the 1 they replace. The ja + en room they needed
//   under SC-005 came from three strings no shell has drawn since the done and
//   progress screens lost them (`create.identiconHint`, `create.walletAddressLabel`,
//   `create.progressMeterLabel`), not from a budget move. Same branches:
//   1779 − 1 + 4 − 3 = 1779 leaves — the count is unchanged, the set is not.
// 1878 (099, same day): + `explore.closeOtherTabs`, `explore.closeTabsToRight`
//   — the browser's batch close, beside `closeAllTabs`. Same branches.
// 1878 (merge of #409 into main after 099, 2026-10-04): #409's set (1779 leaves)
//   + 099's two batch-close keys = 1781 leaves, 97 branches.
// 1878 (issue #408, 2026-10-04): + `componentsUi.gas.rowShort` — a fee coin
//   that cannot pay says why under its greyed row, need and have in its own
//   unit — and `componentsUi.gas.noCoinPays`, the line under the fee when not
//   one coin on offer can pay it (it used to name the coin in force: "Insufficient
//   ETH for gas fees" over a wallet whose USDT was short too). Same branches:
//   1779 + 2 = 1781 leaves.
// 1877 (issue #408): − `componentsUi.signing.gasEstimateFailed` — no client
//   draws it (the fee row's failed state says what failed); its bytes pay for
//   the two above inside the unchanged SC-005 budget. 1780 leaves.
// 1879 (merge of #417 into #408, 2026-10-04): #417's 1781 leaves + #408's net
//   one (+ rowShort, noCoinPays, − gasEstimateFailed) = 1782 leaves, 97 branches.
// 1877 (100, 2026-10-04): a page may ask Vela to add a network — + `connect.
//   browser.{addLead,addFromSite}` (who asks; "the name and coin are the
//   site's") and `componentsUi.browserStatus.reason.badRpc`; the sheet's other
//   words are Settings' own (`settingsModals.addNetwork.*`, `addToken.label*`,
//   `addToken.errorNotCompatible`, `assets.rpcFixWrongChain`). Minus `onboarding.
//   login.alertNotFound{Title,Body}`, read by no client (git grep: neither
//   the full path nor the leaf name appears outside the corpus and the
//   generated tables) — the room the three need under SC-005, instead of a
//   budget move. Same branches: 1779 + 3 - 2 = 1780 leaves.
// 1880 (merge of #419 into 100, 2026-10-04): #419's 1782 leaves (main + #409,
//   #408; #411 moved no key) + 100's net one (+ addLead, addFromSite, badRpc,
//   − alertNotFound{Title,Body}) = 1783 leaves, 97 branches.
// 1881 (issue #430, 2026-10-06): + `contacts.importFailEmpty` — a file with a
//   header and no rows says it holds no contacts, not "use a JSON or CSV file".
//   1784 leaves.
// 1880 (issue #438, 2026-10-07): − `componentsUi.signing.confirmBlock.feeShort`
//   — a short fee coin is said under the fee ("Insufficient ETH for gas fees",
//   or `componentsUi.gas.noCoinPays`), and the slide said it a second time;
//   `ConfirmBlock::FeeShort` now names no line, so no client draws this one.
//   Same branches: 1783 leaves.
// 1887 (web analytics + app prompt, 2026-10-07): + the `settings.analytics`
//   branch ({title, subtitle} — the web wallet's and extension's "Share
//   anonymous usage statistics" switch) and the `home.getApp` branch ({title,
//   body, scanHint} — the "Get Vela on your phone" card, off until the store
//   links exist). "App Store" and "Google Play" are product names, not prose,
//   and stay out of the corpus. 1788 leaves, 99 branches.
// 1888 (issue #446, 2026-10-07): + `onboarding.common.phoneLinkFailed` — a
//   sign-in or create whose link to the other device failed (the core's
//   `PromptKind::{SignInFailed,CreateFailed}.phone_link`) says to scan again,
//   where it used to say to set up Face ID. 1789 leaves, 99 branches.
// 1890 (issue #450, 2026-10-07): + `onboarding.common.keyUnavailable{Title,Body}`
//   — a security key this device could not use is said as the key's problem
//   (the core's `PromptKind::NotSupported{Create,Login}.security_key`), where
//   it said "biometric authentication is not available". 1791 leaves, 99
//   branches.
// 1891 (signing-sheet polish, 2026-10-08): the registry backup's words are
//   ClearTerms, so they live under `componentsUi.signing` — `settingsModals.
//   backup.{intent,publicKeys}` move to `componentsUi.signing.
//   {intentBackUpPublicKeys,labelPublicKeys}`, + `componentsUi.signing.
//   {labelNetwork,labelAddress}` (the backup's rows now say the network the
//   wallet's own sheet no longer draws as a header chip; values are each
//   locale's existing `addToken.labelNetwork` / `contacts.addressLabel`), and
//   − `settingsModals.backup.registeredAs` (the footer's signing account
//   already names the wallet). 1791 − 3 + 4 = 1792 leaves, 99 branches.
// 1889 (issue #461, 2026-10-08): − `componentsUi.signing.{slideToConfirm,
//   slideConfirmAction}` — the signing sheet confirms with the shells' shared
//   primary button, labelled with the action alone ("Confirm swap", "Sign",
//   "Back up public keys"), as the Send confirm screen always did; the slide
//   and its "Slide to confirm ·" prefix are gone. 1790 leaves, 99 branches.
// 1890 (issue #462, 2026-10-08): + `home.updating` — every shell draws the
//   same "↻ Updated <ago>" control under the total, and while the refresh a
//   person asked for is out (`BalanceView.refreshing`) its label reads
//   "Updating…" with the glyph spinning. No existing key said exactly that
//   (`home.balanceStale` is a sentence about some balances, not a label).
//   1791 leaves, 99 branches.
// 1884 (issue #465, 2026-10-08): − `explore.{groupOptions,newGroup,
//   moveToGroup,hiddenTag,hiddenCount,systemGroup}` — Explore has no custom
//   groups any more (the core dropped them; a stored document's `groups` is
//   ignored and not written back), and the Manage groups sheet keeps two
//   rows, Favorites and Recent dApps, each with its eye and no "System"
//   meta. `explore.manageGroups`, `siteCount`, `hide`, `show` and `edit`
//   stay: that sheet still uses them. Contact groups are another feature and
//   keep every key. 1785 leaves, 99 branches.
// 1888 (review of #462/#465/#468, 2026-10-08): + `home.refreshBalance` —
//   the refresh control's name before any balance read has settled, when it
//   is the glyph alone and screen readers announced a bare "button" (or,
//   on iOS, "Updating…" over a control at rest). `explore.siteCount` becomes
//   `siteCount_{one,few,many,other}` with `{{count}}`, each locale its own
//   CLDR categories — the Manage groups row read "1 sites". And
//   `send.recipientPickAria` keeps its path but now names the contact pick
//   alone: scanning has its own door on every shell (#468). 1789 leaves,
//   99 branches.
// 1891 (dApp-browser navigation, 2026-10-09): `explore.openTabs` — the
//   header of the home's resume rows, counting every open tab — becomes
//   `openTabs_{one,few,many,other}` with `{{count}}`, each locale its own
//   CLDR categories, as `siteCount` did: the most common case, one tab,
//   read "1 tabs open". Shells pass the count. 1789 − 1 + 4 = 1792 leaves,
//   99 branches.
// 1890 (device pass, 2026-10-09): + `onboarding.login.
//   alertSignInFailedBodyAndroid` — Android's sign-in failure, "{{message}}
//   / Make sure this device has a screen lock or fingerprint set up and try
//   again." The shell picks the key, and `alertSignInFailedBody` names Face
//   ID and Touch ID, an iPhone's and a Mac's words; Android had borrowed
//   `onboarding.common.openBiometricSettings` for its second paragraph,
//   which is a button's label (no full stop, an imperative in de/ja/ko).
//   Each locale's screen-lock word is its `create.methodPlatformBody`'s.
//   Its 249 bytes of `en`+`ja` put the SC-005 reduction at 85.79%, under the
//   85.8% claimed; minus `onboarding.create.alertNotDiscoverable{Title,
//   Body}`, read by no client — every shell says a passkey that did not sync
//   with `onboarding.common.notDiscoverable{Title,Body}` (git grep: neither
//   the full path nor the leaf name appears outside the corpus and the
//   generated tables) — the room it needs, instead of a budget move. Same
//   branches: 1792 + 1 - 2 = 1791 leaves.
// 1905 (spec 102, 2026-10-09): where you review and sign — in Vela, or on a
//   page you trust. The Trusted Signer stops being a fourth key method and
//   becomes each account's signing VENUE. + the `settings.venue` branch (title,
//   subtitle, the two rows and their lines, the two reasons a venue cannot
//   reach an account's keys), + `componentsUi.signing.{handoffTitle,handoffKey}`
//   (the hand-off card), + the `componentsUi.signing.integrity` branch (ten
//   lines: checking, matches the published build list · checked, trusted here,
//   unchecked, mismatch, blocked, ask to trust, could not check, no version,
//   all blocked), + `settings.signing.{pageAdd,pageDuplicate,keysOn}` (the
//   saved-pages list that replaces the free-text page field), +
//   `onboarding.create.{ownPageTitle,ownPageBody}` ("Use my own signing
//   page"). `settings.signing.{title,subtitle}` and the waiting / closed /
//   refused / mismatch / timeout lines are reworded (they promised a choice per
//   signature, and named a "Trusted Signer"). Minus what only served the fourth
//   method or the free-text field — `componentsUi.signing.{trustedSignerTitle,
//   trustedSignerBody,signWith,trustedSignerDoneTab}`, `settings.signing.
//   {pageTitle,pageSubtitle,pageForeign,pageReset}`, `onboarding.create.
//   {methodBlockedHint,methodBlockedSigner}` — and three dead duplicates of
//   `onboarding.common` lines no client reads (git grep: neither the full path
//   nor the leaf name appears outside the corpus and the generated tables):
//   `onboarding.create.{alertNotDiscoverableTitle,alertNotDiscoverableBody,
//   verifyStuckHint}` — the ja + en room the new lines need under SC-005,
//   instead of a budget move. 1792 − 13 + 25 = 1804 leaves, 99 + 2 = 101
//   branches.
// 1913 (spec 102 core round, 2026-10-09): the owner's naming (D6) and the
//   gaps the shells hit in Phase 2. + `settings.venue.blockedWeb` (the web
//   opens no signing page: its page rows are drawn disabled with this
//   reason, and a custom-domain account's refusal there says it), +
//   `settings.signing.{pageSelfHosted,pageTrust,pageRename,pageRemove,
//   pageName}` ("Self-hosted · <domain>", "Trust this version" — the answer
//   to the integrity line's ask-to-trust question — and the signing-pages
//   list's own rename / remove / name words, which the phones borrowed from
//   other screens), + `componentsUi.signing.{ceremonyCreate,ceremonySignIn,
//   ceremonyConfirm}` (a key ceremony on a self-hosted page has its own
//   title, not the hand-off card's "Review and sign"). Renamed:
//   `onboarding.create.{ownPageTitle,ownPageBody}` → `{signingPageTitle,
//   signingPageBody}` — the entry is "Use a trusted signing page" now and
//   lists Vela's official page too, so "own page" named it wrongly. Reworded
//   (D6): `settings.venue.{inVela,page}` ("Review and sign in Vela" / "… on a
//   trusted signing page"), `settings.signing.{pageOfficial,pageAdd}` ("Vela's
//   official signing page", "Add a self-hosted signing page") and
//   `componentsUi.signing.handoffTitle`. Minus `home.rescanNativeNote`, the
//   longest of the `home.rescan*` lines spec 004's research found with zero
//   call sites (still none in any shell: git grep, every 102 worktree) — the
//   ja + en room the new lines need under SC-005 instead of a budget move.
//   1804 − 2 + 2 + 1 + 5 + 3 − 1 = 1812 leaves, 101 branches.
// 1914 (main's device pass merged into spec 102, 2026-10-09): the 1890
//   entry above landed on main beside 1905 and 1913. Its minus
//   (`onboarding.create.alertNotDiscoverable{Title,Body}`) is 1905's too, so
//   only its plus is new here: `onboarding.login.alertSignInFailedBodyAndroid`.
//   1812 + 1 = 1813 leaves, 101 branches.
// 1915 (spec 102 integration polish, 2026-10-09): the hand-off card's key is
//   a row, as the signing page draws it — 「确认方式 | 这台设备」 — not a
//   sentence a locale must inflect a place title into ("用 这台设备 确认",
//   "Confirm with This device"). − `componentsUi.signing.handoffKey`
//   ("Confirm with {{key}}"), + `componentsUi.signing.{confirmWithLabel,
//   newKeyOnLabel}` (the row's label: "Confirm with", and a create
//   ceremony's "New key on" — the page's `field.confirmWith` /
//   `field.keyOn`, word for word in all fifteen). `handoffTitle` loses its
//   possessive (D6: only a self-hosted page is "your own").
//   1813 − 1 + 2 = 1814 leaves, 101 branches.
// 1867 (correctness batch, 2026-10-09): − the 54 `clearSigning.scenario*`
//   leaves (the retired clear-signing test page's scenario names; no reader
//   in the core or any shell — git grep, every worktree), which pays the
//   ja + en room for six new lines under SC-005 rather than a budget move:
//   + `onboarding.login.registryUnreachable{Title,Body}` (sign-in could not
//   look up the passkey's wallet: a free retry, never the rebuild),
//   + `componentsUi.signing.confirmBlock.previousPending` (one transaction
//   in flight per account and network), + `componentsUi.signing.wentFirst`
//   (the relay's `nonce_used`), + `componentsUi.gas.reason{ChainDown,
//   Internal}` (issue #483: the fee row's own sentences).
//   1814 − 54 + 6 = 1766 leaves, 101 branches.
// 1869 (correctness batch integration, 2026-10-10): the fee's failure says
//   one truth on its row and under the held confirm — the core retries it
//   by itself, so + `componentsUi.signing.confirmBlock.feeRetrying` (the
//   footer while it does; `feeFailed`'s "tap it" is left for a failure only
//   a tap retries) — and Continue's failed estimate says the chain out of
//   reach by its name: + `send.alertEstimateChainDownBody`.
//   `componentsUi.gas.reasonInternal` is reworded in place (it now also
//   stands on the home and in that alert, where nothing is retrying).
//   1766 + 2 = 1768 leaves, 101 branches.
// 1873 (correctness batch polish, 2026-10-10): the relay turning a submit
//   back because the account's previous transaction still holds the nonce
//   is "not sent yet", said calmly — not "Failed" over the held confirm's
//   "waiting…" line: + `componentsUi.signing.notSent{Title,Body}` (the
//   sheet's and Send's). A fee the relay answered would fail says what a
//   tap on its row does: + `componentsUi.gas.payWithAnotherCoin` (the
//   row's figure; the tap opens the coins), + `componentsUi.signing
//   .confirmBlock.feeWouldFail` (the line under the held confirm, a fact
//   asking for no tap — `feeFailed`'s "tap it to retry" was untrue there).
//   1768 + 4 = 1772 leaves, 101 branches.
if (PATHS.length !== 1873) fail(`expected 1873 paths (1772 leaf + 101 branch), got ${PATHS.length}`);
if (leafSet.size !== 1772) fail(`expected 1772 leaf paths, got ${leafSet.size}`);
if (branchSet.size !== 101) fail(`expected 101 branch paths, got ${branchSet.size}`);

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
console.log(`  shared ${shared} + ja ${ja.tableBytes} + en ${en.tableBytes} = ${shared + ja.tableBytes + en.tableBytes} bytes (budget 145,400 for the per-locale halves — tests/i18n_residency.rs SC005_BUDGET)`);
console.log(`  per-locale halves only: ${ja.tableBytes + en.tableBytes} bytes`);
const assetTotal = assetStats.reduce((n, a) => n + a.bytes, 0);
const assetEn = assetStats.find((a) => a.lng === 'en').bytes;
const assetJa = assetStats.find((a) => a.lng === 'ja').bytes;
console.log(`assets     : ${LOCALES.length} files, ${assetTotal} bytes total -> ${relative(REPO_ROOT, ASSET_DIR)}/`);
console.log(`  on-demand: a ja reader fetches ${assetJa} + ${assetEn} = ${assetJa + assetEn} bytes, not ${assetTotal}`);
const dtBytes = dtRows.reduce((n, r) => n + Buffer.byteLength(r.am) + Buffer.byteLength(r.pm) + r.weekdays.reduce((m, w) => m + Buffer.byteLength(w), 0), 0);
console.log(`datetime   : ${dtRows.length} locales, ${dtBytes} bytes of strings (ICU ${process.versions.icu}) -> ${relative(REPO_ROOT, DATETIME_FILE)}`);
