package app.getvela.wallet.feature.onboarding.flow

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.times
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.components.VelaPrimaryButton
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.passkey.PasskeyDirectory
import app.getvela.wallet.core.passkey.PasskeyProviderMark
import app.getvela.wallet.core.designsystem.tokens.VelaBorder
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaLeading
import app.getvela.wallet.core.designsystem.tokens.VelaMonoFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaOpacity
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.feature.onboarding.core.CreateKeyRow
import app.getvela.wallet.feature.onboarding.core.KeyMethod
import app.getvela.wallet.feature.settings.components.SettingsDivider

/** The founding-set cap, mirroring the core's `MAX_MULTI_KEYS`. */
const val MAX_KEYS: Int = 7

/**
 * The founding key list — the screen spec 014 never had, and the only place a
 * multi-key wallet can be assembled.
 *
 * Everything on it is a rendering of `CreateView`; nothing here decides. The
 * three gates the core enforces (at most seven keys, every key confirmed, a sole
 * key must be backed up) surface as a disabled control with a stated reason
 * rather than as a tap that quietly does nothing.
 */
@Composable
fun ColumnScope.KeysScreen(
    keys: List<CreateKeyRow>,
    canAddKey: Boolean,
    canFinish: Boolean,
    needsSecondKey: Boolean,
    busy: Boolean,
    /**
     * Issue #475: the heading over the three places, as the core words it
     * (`CreateView.add_heading_key`) — "Add a passkey" with no key yet, "Add
     * another" with room for one more, "Limit of 7 reached" at the cap. It is
     * the screen's ONLY add affordance.
     */
    addHeadingKey: String,
    /**
     * Issue #475: the core's word that the three places are drawn open with
     * no fold to tap (`CreateView.methods_pinned` — no key yet, and one may
     * be added). Otherwise they fold under the heading.
     */
    methodsPinned: Boolean,
    addMethods: List<KeyMethod> = KeyMethod.entries,
    /**
     * Spec 102: the page chosen with "Use a trusted signing page" (its address,
     * whose keys it reaches, its integrity line) — `null` while the keys are
     * made in the app.
     */
    signingPage: app.getvela.wallet.feature.settings.components.SigningPageItemModel? = null,
    /** Spec 102: a page may still be chosen — only before the first key. */
    canChoosePage: Boolean = false,
    onChooseOwnPage: () -> Unit = {},
    onClearOwnPage: () -> Unit = {},
    /** Spec 102: "Trust this version" on the chosen page, when its check asks. */
    onTrustPage: (String) -> Unit = {},
    onAddKey: (KeyMethod) -> Unit,
    onConfirmKey: (Int) -> Unit,
    onRemoveKey: (Int) -> Unit,
    onFinish: () -> Unit,
) {
    val strings = LocalVelaStrings.current
    val colors = VelaTheme.colors
    var pickerOpen by remember { mutableStateOf(false) }
    val full = keys.size >= MAX_KEYS

    // An EMPTY list keeps the three methods expanded: the first key's method
    // is the person's choice too (the Xiaomi lock-out fix — its system sheet
    // cannot reach a security key), and an empty list with a collapsed "+"
    // is a puzzle, not a step. WHEN that holds is the core's word
    // (`methods_pinned`, issue #475); only the fold's open/closed is ours.
    val pickerShown = methodsPinned || (pickerOpen && canAddKey)

    Column(
        modifier = Modifier
            .weight(1f)
            .fillMaxWidth()
            .verticalScroll(rememberScrollState()),
    ) {
        Text(
            text = strings.t(
                if (needsSecondKey) I18nKeys.Create.KEYS_TITLE_BLOCKED else I18nKeys.Create.KEYS_TITLE,
            ),
            color = colors.fgBase,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.bold,
            fontSize = VelaTextSize.xl3,
        )
        Spacer(modifier = Modifier.height(VelaSpacing.md))
        Text(
            text = strings.t(
                when {
                    needsSecondKey -> I18nKeys.Create.KEYS_SUBTITLE_BLOCKED
                    full -> I18nKeys.Create.KEYS_SUBTITLE_FULL
                    else -> I18nKeys.Create.KEYS_SUBTITLE
                },
            ),
            color = colors.fgMuted,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.lg,
            lineHeight = VelaLeading.normal * VelaTextSize.lg,
        )

        if (needsSecondKey) {
            Spacer(modifier = Modifier.height(VelaSpacing.xl))
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(VelaRadius.lg))
                    .background(colors.accentSoft)
                    .padding(VelaSpacing.lg),
                verticalAlignment = Alignment.Top,
            ) {
                Icon(
                    imageVector = VelaIcons.TriangleAlert,
                    contentDescription = null,
                    tint = colors.accentBase,
                    modifier = Modifier.size(VelaIconSize.base),
                )
                Spacer(modifier = Modifier.size(VelaSpacing.md))
                Text(
                    text = strings.t(I18nKeys.Create.NEED_SECOND_KEY_HINT),
                    color = colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.base,
                    lineHeight = VelaLeading.normal * VelaTextSize.base,
                )
            }
        }

        Spacer(modifier = Modifier.height(VelaSpacing.xl3))

        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = strings.t(I18nKeys.Create.KEYS_LABEL),
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.semibold,
                fontSize = VelaTextSize.sm,
            )
            Text(
                // Mono: it is a count, and a count that jitters in width as it
                // changes reads as the layout moving rather than the number.
                text = strings.t(
                    I18nKeys.Create.KEY_COUNT,
                    mapOf("current" to keys.size.toString(), "max" to MAX_KEYS.toString()),
                ),
                color = colors.fgMuted,
                fontFamily = VelaMonoFontFamily,
                fontSize = VelaTextSize.sm,
            )
        }

        Spacer(modifier = Modifier.height(VelaSpacing.md))

        keys.forEachIndexed { index, key ->
            if (index > 0) HorizontalDivider(color = colors.borderBase, thickness = VelaBorder.hairline)
            KeyRow(
                key = key,
                busy = busy,
                // Row 0 is the pinned key: not removable, and its name IS the
                // wallet name. Removing it is `start over`, not a row action.
                removable = index > 0,
                onConfirm = { onConfirmKey(index) },
                onRemove = { onRemoveKey(index) },
            )
        }

        // The key rows end on a hairline; an empty list has none to end.
        if (keys.isNotEmpty()) HorizontalDivider(color = colors.borderBase, thickness = VelaBorder.hairline)

        // Issue #475: ONE add affordance. It was a "+ Add a passkey" row over
        // an "Add another" list that did the same thing — and said "another"
        // with no key yet.
        AddHeading(
            text = strings.t(addHeadingKey),
            pinned = methodsPinned,
            open = pickerShown,
            enabled = canAddKey,
            onToggle = { pickerOpen = !pickerOpen },
        )

        if (pickerShown) {
            AddMethodPicker(
                allowed = addMethods,
                signingPage = signingPage,
                canChoosePage = canChoosePage,
                onChooseOwnPage = onChooseOwnPage,
                onClearOwnPage = onClearOwnPage,
                onTrustPage = onTrustPage,
            ) { method ->
                pickerOpen = false
                onAddKey(method)
            }
        } else if (signingPage != null) {
            // The page every key of this wallet is made on, said under the
            // list even when the picker is folded away.
            OwnPageEntry(chosen = signingPage, onOpen = {}, onClear = null, clearLabel = "", onTrust = onTrustPage)
        }

        Spacer(modifier = Modifier.height(VelaSpacing.xl3))
        Text(
            text = strings.t(I18nKeys.Create.KEYS_HINT),
            color = colors.fgSubtle,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.base,
            lineHeight = VelaLeading.normal * VelaTextSize.base,
        )
        Spacer(modifier = Modifier.height(VelaSpacing.xl3))
    }

    VelaPrimaryButton(
        text = strings.t(
            if (needsSecondKey) I18nKeys.Create.ADD_SECOND_KEY_BTN else I18nKeys.Create.CREATE_WALLET_BTN,
        ),
        onClick = onFinish,
        enabled = canFinish,
        loading = busy,
        modifier = Modifier.fillMaxWidth(),
    )
    Spacer(modifier = Modifier.height(VelaSpacing.xl))
}

@Composable
private fun KeyRow(
    key: CreateKeyRow,
    busy: Boolean,
    removable: Boolean,
    onConfirm: () -> Unit,
    onRemove: () -> Unit,
) {
    val strings = LocalVelaStrings.current
    val colors = VelaTheme.colors
    Row(
        modifier = Modifier.fillMaxWidth().padding(vertical = VelaSpacing.lg),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        // Who is holding this key, when the core's AAGUID catalog knows: the
        // vault's own mark and its own name. When it does not — a hardware key,
        // an authenticator that reported nothing — the row says what it always
        // said, from `method`.
        // Who is holding this key: the compiled catalog's name, then the
        // directory's for a model no catalog carries, then the method line.
        val holder = key.providerName.ifEmpty {
            PasskeyDirectory.holder(key.aaguid, VelaTheme.isDark)?.name.orEmpty()
        }
        val drewMark = PasskeyProviderMark(
            key = key,
            label = holder.ifEmpty { strings.t(providerLineFor(key.kind)) },
            size = VelaSizing.controlSm,
        )
        if (!drewMark) {
            // Nothing to draw from the key itself: the method glyph, as before.
            Box(
                modifier = Modifier
                    .size(VelaSizing.controlSm)
                    .clip(RoundedCornerShape(VelaRadius.md))
                    .background(colors.bgSunken),
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    imageVector = when (key.kind) {
                        KeyMethod.SecurityKey -> VelaIcons.Link2
                        else -> VelaIcons.Wallet
                    },
                    contentDescription = null,
                    tint = colors.fgMuted,
                    modifier = Modifier.size(VelaIconSize.md),
                )
            }
        }
        Spacer(modifier = Modifier.size(VelaSpacing.lg))
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = key.name,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.semibold,
                fontSize = VelaTextSize.lg,
            )
            Text(
                text = holder.ifEmpty { strings.t(providerLineFor(key.kind)) },
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.sm,
            )
        }

        // One trailing slot, as the design draws it. A key that has not confirmed
        // its membership has no status to show yet, so the retry TAKES that slot
        // rather than crowding in beside it.
        if (key.confirmed) {
            KeyBadge(synced = key.synced)
        } else {
            Text(
                text = strings.t(I18nKeys.Create.CONFIRM_KEY_BTN),
                color = colors.accentBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.semibold,
                fontSize = VelaTextSize.base,
                modifier = Modifier
                    .clickable(enabled = !busy, onClick = onConfirm)
                    .padding(VelaSpacing.sm)
                    .alpha(if (busy) VelaOpacity.disabled else 1f),
            )
        }

        if (removable) {
            Spacer(modifier = Modifier.size(VelaSpacing.md))
            Icon(
                imageVector = VelaIcons.Close,
                contentDescription = strings.t(I18nKeys.Create.REMOVE_KEY_BTN),
                tint = colors.fgSubtle,
                modifier = Modifier
                    .size(VelaIconSize.lg)
                    .clickable(enabled = !busy, onClick = onRemove)
                    .alpha(if (busy) VelaOpacity.disabled else 1f),
            )
        }
    }
}

@Composable
private fun KeyBadge(synced: Boolean) {
    val strings = LocalVelaStrings.current
    val colors = VelaTheme.colors
    val text = strings.t(
        if (synced) I18nKeys.Create.KEY_SYNCED_BADGE else I18nKeys.Create.KEY_DEVICE_ONLY_BADGE,
    )
    Box(
        modifier = Modifier
            .clip(RoundedCornerShape(VelaRadius.full))
            .background(if (synced) colors.successSoft else colors.bgSunken)
            .padding(horizontal = VelaSpacing.md, vertical = VelaSpacing.sm),
    ) {
        Text(
            text = text,
            color = if (synced) colors.successBase else colors.fgMuted,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.medium,
            fontSize = VelaTextSize.xs,
        )
    }
}

/**
 * One place a key lives, as a chooser draws it — the keys screen and the
 * sign-in sheet both — in Settings' row metrics (issue #475): a full
 * control's height, the title, ONE line under it (cut with an ellipsis, never
 * wrapped into the next row's title), a chevron.
 */
@Composable
internal fun KeyPlaceRow(title: String, body: String, enabled: Boolean = true, onClick: () -> Unit) {
    val colors = VelaTheme.colors
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(enabled = enabled, role = Role.Button, onClick = onClick)
            .heightIn(min = VelaSizing.controlLg)
            .padding(vertical = VelaSpacing.lg)
            .alpha(if (enabled) 1f else VelaOpacity.disabled)
            .semantics { contentDescription = title },
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
    ) {
        Column(modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(VelaSpacing.sm)) {
            Text(
                text = title,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.semibold,
                fontSize = VelaTextSize.lg,
            )
            Text(
                text = body,
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.sm,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        if (enabled) {
            Icon(
                imageVector = VelaIcons.ChevronRight,
                contentDescription = null,
                tint = colors.fgSubtle,
                modifier = Modifier.size(VelaIconSize.sm),
            )
        }
    }
}

/**
 * The heading over the three places (issue #475) — the screen's one add
 * affordance, in the core's words.
 *
 * [pinned] (no key yet): a plain section label; the places under it are open
 * and there is nothing to fold. Otherwise it is the fold's own row — "+ Add
 * another", closed until tapped — and at the cap, or while a ceremony is in
 * flight, it is said and cannot be tapped ([enabled]).
 */
@Composable
private fun AddHeading(text: String, pinned: Boolean, open: Boolean, enabled: Boolean, onToggle: () -> Unit) {
    val colors = VelaTheme.colors
    if (pinned) {
        Text(
            text = text,
            color = colors.fgMuted,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.semibold,
            fontSize = VelaTextSize.sm,
            modifier = Modifier.padding(top = VelaSpacing.lg, bottom = VelaSpacing.sm).semantics { heading() },
        )
        return
    }
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(enabled = enabled, role = Role.Button, onClick = onToggle)
            .heightIn(min = VelaSizing.controlLg)
            .padding(vertical = VelaSpacing.lg)
            .semantics(mergeDescendants = true) {},
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
    ) {
        // The "+" belongs to an add that can happen; a full set, or a
        // ceremony under way, is only said.
        if (enabled) {
            Icon(
                imageVector = VelaIcons.Plus,
                contentDescription = null,
                tint = colors.accentBase,
                modifier = Modifier.size(VelaIconSize.md),
            )
        }
        Text(
            text = text,
            color = if (enabled) colors.accentBase else colors.fgMuted,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.semibold,
            fontSize = VelaTextSize.lg,
            modifier = Modifier.weight(1f),
        )
        if (enabled) {
            Icon(
                imageVector = VelaIcons.ChevronDown,
                contentDescription = null,
                tint = colors.fgSubtle,
                modifier = Modifier.size(VelaIconSize.sm).rotate(if (open) 180f else 0f),
            )
        }
    }
}

/**
 * Where a founding key is minted — the three places a key lives — and, before
 * the first key, "Use a trusted signing page" (spec 102).
 *
 * Unlike the browser, this client OWNS the picker — Credential Manager shows the
 * providers it knows about, not a this-device / nearby-device / security-key
 * choice — so the person's selection here is honoured at the ceremony rather
 * than merely recorded. The list IS `KeyMethod`: three places, no fourth.
 *
 * The own-page entry is not a place: it says WHERE the keys will belong (a
 * page on the person's own domain mints them there, R3), and so it is offered
 * only while no key exists — the first key commits the set to one domain.
 * Once a page is chosen the entry shows it, with its domain and integrity
 * line, and a ✕ back to the app.
 */
@Composable
private fun AddMethodPicker(
    allowed: List<KeyMethod>,
    signingPage: app.getvela.wallet.feature.settings.components.SigningPageItemModel?,
    canChoosePage: Boolean,
    onChooseOwnPage: () -> Unit,
    onClearOwnPage: () -> Unit,
    onTrustPage: (String) -> Unit,
    onPick: (KeyMethod) -> Unit,
) {
    val strings = LocalVelaStrings.current
    val colors = VelaTheme.colors
    Column(modifier = Modifier.fillMaxWidth().padding(bottom = VelaSpacing.md)) {
        // Settings' rows (issue #475): a hairline between them, a full
        // control's height, the title and ONE line under it. They had no
        // divider and the subtitle ran on, so one option's second line nearly
        // touched the next option's title.
        KeyMethod.entries.forEachIndexed { index, method ->
            if (index > 0) SettingsDivider()
            val (title, body) = methodCopy(method, KeyChooser.Create, strings)
            KeyPlaceRow(title = title, body = body, enabled = method in allowed) { onPick(method) }
        }
        if (signingPage != null || canChoosePage) {
            // Not a fourth place: its own group, a section's gap below the three.
            Spacer(modifier = Modifier.height(VelaSpacing.xl))
            SettingsDivider()
            OwnPageEntry(
                chosen = signingPage,
                onOpen = onChooseOwnPage,
                onClear = onClearOwnPage.takeIf { canChoosePage },
                clearLabel = strings.t(I18nKeys.Create.REMOVE_KEY_BTN),
                onTrust = onTrustPage,
            )
        }
    }
}
