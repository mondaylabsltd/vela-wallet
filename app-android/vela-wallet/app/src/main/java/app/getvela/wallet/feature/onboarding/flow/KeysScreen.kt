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
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
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
    // is a puzzle, not a step.
    val pickerShown = (pickerOpen || keys.isEmpty()) && canAddKey

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

        HorizontalDivider(color = colors.borderBase, thickness = VelaBorder.hairline)

        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clickable(enabled = canAddKey) { pickerOpen = !pickerOpen }
                .padding(vertical = VelaSpacing.lg)
                .alpha(if (canAddKey) 1f else VelaOpacity.disabled),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = "+",
                color = colors.accentBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.bold,
                fontSize = VelaTextSize.xl2,
            )
            Spacer(modifier = Modifier.size(VelaSpacing.lg))
            Text(
                text = strings.t(
                    if (full) I18nKeys.Create.KEY_LIMIT_REACHED else I18nKeys.Create.ADD_KEY_BTN,
                ),
                color = if (full) colors.fgMuted else colors.accentBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.semibold,
                fontSize = VelaTextSize.lg,
            )
        }

        if (pickerShown) {
            AddMethodPicker { method ->
                pickerOpen = false
                onAddKey(method)
            }
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
            label = holder.ifEmpty { strings.t(providerLineFor(key.method)) },
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
                    imageVector = when (key.method) {
                        KeyMethod.SecurityKey -> VelaIcons.Link2
                        // Spec 075: a page you read — the web's lucide `eye`.
                        KeyMethod.ClearSigner -> VelaIcons.Eye
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
                text = holder.ifEmpty { strings.t(providerLineFor(key.method)) },
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
 * The four ways to mint a founding key.
 *
 * Unlike the browser, this client OWNS the picker — Credential Manager shows the
 * providers it knows about, not a this-device / nearby-device / security-key
 * choice — so the person's selection here is honoured at the ceremony rather
 * than merely recorded.
 *
 * All four routes are live: platform (this device), scan (mint the key on a
 * phone over caBLE), a security key, and — spec 075 — the Clear Signer, a page
 * the person reads which runs the ceremony itself. The list IS `KeyMethod`, so
 * a route the core gains appears here without a second list to keep in step.
 */
@Composable
private fun AddMethodPicker(onPick: (KeyMethod) -> Unit) {
    val strings = LocalVelaStrings.current
    val colors = VelaTheme.colors
    Column(modifier = Modifier.fillMaxWidth().padding(bottom = VelaSpacing.md)) {
        Text(
            text = strings.t(I18nKeys.Create.ADD_METHOD_LABEL),
            color = colors.fgMuted,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.semibold,
            fontSize = VelaTextSize.sm,
            modifier = Modifier.padding(vertical = VelaSpacing.md),
        )
        KeyMethod.entries.forEach { method ->
            val available = true
            val (titleKey, bodyKey) = methodCopy(method)
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable(enabled = available) { onPick(method) }
                    .padding(vertical = VelaSpacing.lg)
                    .alpha(if (available) 1f else VelaOpacity.disabled)
                    .semantics { contentDescription = strings.t(titleKey) },
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = strings.t(titleKey),
                        color = colors.fgBase,
                        fontFamily = VelaFontFamily,
                        fontWeight = VelaFontWeight.semibold,
                        fontSize = VelaTextSize.lg,
                    )
                    Text(
                        text = strings.t(
                            if (available) bodyKey else I18nKeys.Create.METHOD_HYBRID_UNAVAILABLE,
                        ),
                        color = colors.fgMuted,
                        fontFamily = VelaFontFamily,
                        fontSize = VelaTextSize.sm,
                        lineHeight = VelaLeading.normal * VelaTextSize.sm,
                    )
                }
                if (available) {
                    Icon(
                        imageVector = VelaIcons.ChevronRight,
                        contentDescription = null,
                        tint = colors.fgSubtle,
                        modifier = Modifier.size(VelaIconSize.lg),
                    )
                }
            }
        }
    }
}
