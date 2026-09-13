package app.getvela.wallet.feature.settings.components

import androidx.compose.foundation.background
import androidx.compose.ui.layout.onSizeChanged
import app.getvela.wallet.core.platform.rememberVelaHaptic
import app.getvela.wallet.core.platform.VelaHaptic
import androidx.compose.runtime.setValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.getValue
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.TextAutoSize
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.marks.RemoteLogo
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaBorder
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaMonoFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.feature.settings.CalloutModel
import app.getvela.wallet.feature.settings.CalloutTone
import app.getvela.wallet.feature.settings.ChainMarkModel
import app.getvela.wallet.feature.settings.SettingsIcon
import app.getvela.wallet.feature.settings.SettingsTone
import app.getvela.wallet.feature.settings.StatusPillModel

/**
 * The settings vocabulary's smallest pieces (spec 023).
 *
 * Every one of the forty mocks in `design/settings/` is assembled from these
 * plus the row/list components beside them. Nothing here reads a model bigger
 * than it draws, and nothing formats.
 */

/** Model glyph → drawable. Models stay UI-type free; this is the one bridge. */
fun settingsIcon(icon: SettingsIcon): ImageVector = when (icon) {
    SettingsIcon.Contacts -> VelaIcons.UsersRound
    SettingsIcon.Feedback -> VelaIcons.MessageSquareText
    SettingsIcon.Globe -> VelaIcons.Globe
    SettingsIcon.Coins -> VelaIcons.Coins
    SettingsIcon.Hash -> VelaIcons.Hash
    SettingsIcon.Calendar -> VelaIcons.Calendar
    SettingsIcon.Clock -> VelaIcons.Clock
    SettingsIcon.Network -> VelaIcons.Network
    SettingsIcon.Server -> VelaIcons.Server
    SettingsIcon.Plus -> VelaIcons.Plus
    SettingsIcon.Zap -> VelaIcons.Zap
    SettingsIcon.HardDrive -> VelaIcons.HardDrive
    SettingsIcon.Info -> VelaIcons.Info
    SettingsIcon.Sun -> VelaIcons.Sun
    SettingsIcon.Moon -> VelaIcons.Moon
    SettingsIcon.Monitor -> VelaIcons.Monitor
}

/**
 * The one badge every settings screen uses. Latency, reachability, provider
 * state and compatibility are all this object in the mocks, differing only in
 * tone — so they are one component and not four.
 */
@Composable
fun VelaStatusPill(pill: StatusPillModel, modifier: Modifier = Modifier) {
    val colors = VelaTheme.colors
    val (fg, bg) = when (pill.tone) {
        SettingsTone.Ok -> colors.successBase to colors.successSoft
        SettingsTone.Warn -> colors.warningBase to colors.warningSoft
        SettingsTone.Error -> colors.errorBase to colors.errorSoft
        // Unset, not failed — the mocks grey these rather than colouring them.
        SettingsTone.Neutral -> colors.fgSubtle to colors.bgRaised
    }
    Row(
        modifier = modifier
            .clip(RoundedCornerShape(VelaRadius.full))
            .background(bg)
            .padding(horizontal = VelaSpacing.md, vertical = VelaSpacing.xs),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.sm),
    ) {
        if (pill.dot) {
            Box(
                modifier = Modifier
                    .size(VelaSpacing.md)
                    .clip(RoundedCornerShape(VelaRadius.full))
                    .background(fg),
            )
        }
        Text(
            text = pill.label,
            color = fg,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.medium,
            fontSize = VelaTextSize.sm,
        )
    }
}

/**
 * The tinted explanation box. Eight mocks use it; `Success` swaps the triangle
 * for a check, because a green triangle reads as an alarm.
 */
@Composable
fun VelaCallout(callout: CalloutModel, modifier: Modifier = Modifier) {
    val colors = VelaTheme.colors
    val (fg, bg, icon) = when (callout.tone) {
        CalloutTone.Warning -> Triple(colors.warningBase, colors.warningSoft, VelaIcons.TriangleAlert)
        CalloutTone.Danger -> Triple(colors.errorBase, colors.errorSoft, VelaIcons.TriangleAlert)
        CalloutTone.Info -> Triple(colors.infoBase, colors.infoSoft, VelaIcons.Info)
        CalloutTone.Success -> Triple(colors.successBase, colors.successSoft, VelaIcons.Check)
    }
    Row(
        modifier = modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(VelaRadius.lg))
            .background(bg)
            .padding(VelaSpacing.lg),
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
    ) {
        Icon(
            imageVector = icon,
            contentDescription = null,
            tint = fg,
            // Optical alignment with the first line, not the box.
            modifier = Modifier.padding(top = VelaSpacing.xs).size(VelaIconSize.md),
        )
        Text(
            text = callout.text,
            color = fg,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.base,
            lineHeight = VelaTextSize.base * 1.4f,
        )
    }
}

/** A chain's circular avatar — one letter over its own brand colour. */
@Composable
fun VelaChainMark(mark: ChainMarkModel, size: Dp = VelaSpacing.xl4) {
    // Spec 047: the chain's own logo when the chain-data endpoint has one; the
    // letter over the brand colour stays the fallback (the web's RemoteLogo).
    RemoteLogo(urls = listOfNotNull(mark.logoUrl), size = size) {
        Box(
            modifier = Modifier
                .size(size)
                .clip(RoundedCornerShape(VelaRadius.full))
                .background(Color(mark.colorArgb)),
            contentAlignment = Alignment.Center,
        ) {
            Text(
                text = mark.letter,
                color = Color.White,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.bold,
                fontSize = VelaTextSize.base,
            )
        }
    }
}

/** The hairline the mocks draw between rows. */
@Composable
fun SettingsDivider(modifier: Modifier = Modifier) {
    Box(
        modifier = modifier
            .fillMaxWidth()
            .height(VelaBorder.hairline)
            .background(VelaTheme.colors.borderBase),
    )
}

/**
 * The small caps label above a group of rows — 外观 / 区域格式 / 高级. 高级 is
 * the one that collapses (ST1b), so the chevron is optional and the whole label
 * becomes tappable only when it is present.
 */
@Composable
fun SettingsSectionLabel(
    label: String,
    collapsible: Boolean = false,
    collapsed: Boolean = false,
    onToggle: () -> Unit = {},
) {
    val colors = VelaTheme.colors
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .then(if (collapsible) Modifier.clickable(onClick = onToggle) else Modifier)
            .padding(top = VelaSpacing.xl2, bottom = VelaSpacing.md),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = label,
            color = colors.fgSubtle,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.base,
            modifier = Modifier.weight(1f),
        )
        if (collapsible) {
            Icon(
                imageVector = if (collapsed) VelaIcons.ChevronDown else VelaIcons.ChevronRight,
                contentDescription = null,
                tint = colors.fgSubtle,
                modifier = Modifier.size(VelaIconSize.sm),
            )
        }
    }
}

/**
 * A labelled mono field. Every endpoint on ST9b / ST11 / ST12 / SR2 / SR5 is
 * one of these: a label row that may carry a latency pill, the value in a
 * sunken box, an optional in-field action, and an optional hint under it.
 *
 * **Pass [onValueChange] and it accepts typing; omit it and it renders exactly
 * as it always has.** Spec 023 drew ten of these and gave none of them an
 * input, which was right while every value came from a fixture and wrong the
 * moment a person is expected to enter an RPC URL. The default keeps every
 * existing call site and every gallery state pixel-identical, so making a
 * field editable is a decision taken one call site at a time rather than a
 * change to what settings look like.
 */
@Composable
fun VelaUrlField(
    label: String,
    value: String,
    modifier: Modifier = Modifier,
    placeholder: String? = null,
    hint: String? = null,
    badge: StatusPillModel? = null,
    tone: SettingsTone? = null,
    action: String? = null,
    onValueChange: ((String) -> Unit)? = null,
    keyboard: KeyboardType = KeyboardType.Uri,
    /** Spec 048: the in-field action (检查密钥 / 获取密钥) does something. */
    onAction: (() -> Unit)? = null,
) {
    val colors = VelaTheme.colors
    val border = when (tone) {
        SettingsTone.Error -> colors.errorBase
        SettingsTone.Ok -> colors.successBase
        // A hairline even at rest: on dark, sunken and base are one step apart
        // and the box would otherwise have no edge at all.
        else -> colors.borderBase
    }
    Column(modifier = modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(VelaSpacing.md)) {
        if (label.isNotEmpty() || badge != null) {
            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
                if (label.isNotEmpty()) {
                    Text(
                        text = label,
                        color = colors.fgSubtle,
                        fontFamily = VelaFontFamily,
                        fontSize = VelaTextSize.sm,
                        modifier = Modifier.weight(1f),
                    )
                } else {
                    Spacer(modifier = Modifier.weight(1f))
                }
                if (badge != null) VelaStatusPill(badge)
            }
        }
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .heightIn(min = VelaSizing.controlMd)
                .clip(RoundedCornerShape(VelaRadius.lg))
                .background(colors.bgSunken)
                .border(VelaBorder.hairline, border, RoundedCornerShape(VelaRadius.lg))
                .padding(horizontal = VelaSpacing.lg),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
        ) {
            if (onValueChange == null) {
                Text(
                    text = value.ifEmpty { placeholder.orEmpty() },
                    color = if (value.isEmpty()) colors.fgSubtle else colors.fgBase,
                    fontFamily = VelaMonoFontFamily,
                    fontSize = VelaTextSize.base,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f),
                )
            } else {
                BasicTextField(
                    value = value,
                    onValueChange = onValueChange,
                    singleLine = true,
                    textStyle = TextStyle(
                        color = colors.fgBase,
                        fontFamily = VelaMonoFontFamily,
                        fontSize = VelaTextSize.base,
                    ),
                    cursorBrush = SolidColor(colors.accentBase),
                    keyboardOptions = KeyboardOptions(
                        keyboardType = keyboard,
                        // No autocorrect and no capitalisation: this field
                        // holds URLs and chain ids, and a keyboard that
                        // helpfully capitalises "https" produces a URL that
                        // silently fails to connect.
                        autoCorrectEnabled = false,
                        capitalization = KeyboardCapitalization.None,
                        imeAction = ImeAction.Done,
                    ),
                    modifier = Modifier.weight(1f),
                    decorationBox = { field ->
                        if (value.isEmpty() && placeholder != null) {
                            Text(
                                text = placeholder,
                                color = colors.fgSubtle,
                                fontFamily = VelaMonoFontFamily,
                                fontSize = VelaTextSize.base,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }
                        field()
                    },
                )
            }
            if (action != null) {
                Text(
                    modifier = Modifier.clickable(enabled = onAction != null) { onAction?.invoke() },
                    text = action,
                    color = colors.infoBase,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.base,
                )
            }
        }
        if (hint != null) {
            Text(
                text = hint,
                color = colors.fgSubtle,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
                lineHeight = VelaTextSize.base * 1.4f,
            )
        }
    }
}

/**
 * The product's ONE segmented control (design review 2026-07): three-up for the
 * theme picker, two-up for the avatar style.
 */
@Composable
fun VelaSegmentedControl(
    label: String,
    segments: List<Triple<String, String, ImageVector?>>,
    selectedId: String,
    modifier: Modifier = Modifier,
    onSelect: (String) -> Unit = {},
) {
    val segmentHaptic = rememberVelaHaptic()
    val colors = VelaTheme.colors
    Row(
        // The label is the GROUP's name, not a visible caption: the phone mock
        // shows the control alone under the 语言 row, so the only place it can
        // be said is to a screen reader.
        modifier = modifier
            .semantics { contentDescription = label }
            .fillMaxWidth()
            .clip(RoundedCornerShape(VelaRadius.lg))
            .background(colors.bgSunken)
            // Dark mode sinks sunken BELOW raised, so the unselected track
            // needs a hairline to stay legible against bg.base.
            .border(VelaBorder.hairline, colors.borderBase, RoundedCornerShape(VelaRadius.lg))
            .padding(VelaSpacing.sm),
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.sm),
    ) {
        segments.forEach { (id, text, icon) ->
            val selected = id == selectedId
            Row(
                modifier = Modifier
                    .weight(1f)
                    .heightIn(min = VelaSizing.controlSm)
                    .clip(RoundedCornerShape(VelaRadius.md))
                    .background(if (selected) colors.bgRaised else Color.Transparent)
                    .clickable { segmentHaptic(VelaHaptic.Select); onSelect(id) },
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.Center,
            ) {
                if (icon != null) {
                    Icon(
                        imageVector = icon,
                        contentDescription = null,
                        tint = if (selected) colors.fgBase else colors.fgMuted,
                        modifier = Modifier.size(VelaIconSize.sm),
                    )
                    Spacer(modifier = Modifier.width(VelaSpacing.md))
                }
                Text(
                    text = text,
                    color = if (selected) colors.fgBase else colors.fgMuted,
                    fontFamily = VelaFontFamily,
                    fontWeight = if (selected) VelaFontWeight.semibold else VelaFontWeight.regular,
                    maxLines = 1,
                    // Three equal thirds of a 392dp screen do not hold "Follow
                    // System" at the base size, and clipping turned it into
                    // "Follow" — a different, wrong promise, with no ellipsis
                    // to admit it. Shrinking is the only failure here that
                    // still tells the truth.
                    autoSize = TextAutoSize.StepBased(
                        minFontSize = VelaTextSize.xs,
                        maxFontSize = VelaTextSize.base,
                    ),
                )
            }
        }
    }
}

/**
 * A ——●—— A. The tick row plus the two glyph ends, sized to what they promise.
 *
 * Spec 048 (the founder: 调整字号大小不能滑动，只能点小圆点，没有震动; then
 * 滑动很不跟手): a real slider — dragged or tapped, snapping to its steps, one
 * Detent per step crossed — the web's range input over its row of tick dots.
 * The thumb follows the finger on local state; the scale is committed when
 * the finger lifts. Committing per step re-laid the whole page out under the
 * drag, and the gesture died after one step.
 */
@Composable
fun VelaTextScaleSlider(steps: Int, index: Int, modifier: Modifier = Modifier, onChange: (Int) -> Unit = {}) {
    val colors = VelaTheme.colors
    val haptic = rememberVelaHaptic()
    val dragging = remember { mutableStateOf<Int?>(null) }
    val emitted = remember { mutableStateOf(index) }
    LaunchedEffect(index) { if (dragging.value == null) emitted.value = index }
    val change = rememberUpdatedState(onChange)
    val shown = dragging.value ?: index
    Row(
        modifier = modifier.fillMaxWidth().padding(vertical = VelaSpacing.lg),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
    ) {
        Text(
            text = "A",
            color = colors.fgBase,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.bold,
            fontSize = VelaTextSize.base,
        )
        Box(
            modifier = Modifier
                .weight(1f)
                .height(VelaIconSize.xl2)
                .semantics { contentDescription = "text-scale-slider" },
        ) {
            // The width is read through state, not a gesture key, so a re-layout
            // never restarts the pointerInput mid-drag.
            val widthPx = remember { mutableStateOf(0f) }
            fun stepAt(x: Float): Int {
                val width = widthPx.value
                if (steps <= 1 || width <= 0f) return 0
                val slot = width / steps
                return (x / slot).toInt().coerceIn(0, steps - 1)
            }
            fun settle(x: Float, commit: Boolean) {
                val next = stepAt(x)
                if (next != emitted.value) {
                    emitted.value = next
                    haptic(VelaHaptic.Detent)
                }
                if (commit) {
                    dragging.value = null
                    if (next != index) change.value(next)
                } else {
                    dragging.value = next
                }
            }
            fun release() {
                val at = dragging.value ?: return
                val slot = if (steps <= 1) 0f else widthPx.value / steps
                settle(at * slot + slot / 2f, commit = true)
            }
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .onSizeChanged { widthPx.value = it.width.toFloat() }
                    .pointerInput(steps) {
                        detectHorizontalDragGestures(
                            onDragStart = { position -> settle(position.x, commit = false) },
                            onDragEnd = { release() },
                            onDragCancel = { release() },
                            onHorizontalDrag = { event, _ -> event.consume(); settle(event.position.x, commit = false) },
                        )
                    }
                    .pointerInput(steps) { detectTapGestures { position -> settle(position.x, commit = true) } },
            ) {
                Row(
                    modifier = Modifier.fillMaxSize(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    repeat(steps) { i ->
                        Box(
                            modifier = Modifier
                                .size(VelaIconSize.lg)
                                .semantics { contentDescription = "text-scale-$i" },
                            contentAlignment = Alignment.Center,
                        ) {
                            Box(
                                modifier = Modifier
                                    .size(if (i == shown) VelaIconSize.lg else VelaSpacing.sm)
                                    .clip(RoundedCornerShape(VelaRadius.full))
                                    .background(if (i == shown) colors.fgMuted else colors.borderStrong),
                            )
                        }
                    }
                }
            }
        }
        Text(
            text = "A",
            color = colors.fgBase,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.bold,
            fontSize = VelaTextSize.xl2,
        )
    }
}

