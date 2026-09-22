package app.getvela.wallet.feature.contacts.components

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.Orientation
import androidx.compose.foundation.gestures.draggable
import androidx.compose.foundation.gestures.rememberDraggableState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.Dp
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaMonoFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaMotion
import app.getvela.wallet.core.designsystem.tokens.VelaOnAccent
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.feature.contacts.ContactModel
import app.getvela.wallet.feature.wallet.components.IdenticonAvatar
import app.getvela.wallet.feature.wallet.components.WalletMetrics
import kotlin.math.roundToInt
import kotlinx.coroutines.launch

/**
 * Contact row (spec vocabulary #1): identicon seeded by the FULL address,
 * name that truncates on one line, middle-truncated address underneath in mono.
 * The `member` variant (group detail) is the same anatomy at the smaller
 * avatar size; `selected` is the raised wash the desktop rail/list uses and the
 * component board shows.
 *
 * Swipe-left reveals 转账 (accent) + 删除 (error) in 250ms ease-out; the
 * fixture's `revealed` flag forces the state so the gallery renders it without
 * a gesture (research D5). 删除 never deletes here — it raises the destructive
 * confirmation (action sink).
 */
@Composable
fun ContactRow(
    contact: ContactModel,
    modifier: Modifier = Modifier,
    avatarSize: Dp = WalletMetrics.avatarSize,
    selected: Boolean = false,
    revealed: Boolean = false,
    swipeSendLabel: String? = null,
    swipeDeleteLabel: String? = null,
    onClick: () -> Unit = {},
    onSwipeSend: () -> Unit = {},
    onSwipeDelete: () -> Unit = {},
) {
    val swipeable = swipeSendLabel != null && swipeDeleteLabel != null
    if (!swipeable) {
        ContactRowContent(
            contact = contact,
            avatarSize = avatarSize,
            selected = selected,
            modifier = modifier.clickable(onClick = onClick),
        )
        return
    }

    val density = LocalDensity.current
    val revealWidth = with(density) { (ContactsMetrics.swipeActionWidth * 2).toPx() }
    val offsetX = remember { Animatable(if (revealed) -revealWidth else 0f) }
    val scope = rememberCoroutineScope()
    val reduced = rememberReducedMotion()
    val revealSpec = remember(reduced) {
        tween<Float>(
            durationMillis = if (reduced) 0 else VelaMotion.durationNormal,
            easing = FastOutSlowInEasing,
        )
    }
    var rowHeightPx by remember { mutableIntStateOf(0) }

    // Fixture-driven states (c1 ↔ c1s) settle to their pinned offset.
    LaunchedEffect(revealed, revealWidth) {
        offsetX.animateTo(if (revealed) -revealWidth else 0f, revealSpec)
    }

    Box(modifier = modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier
                .align(Alignment.CenterEnd)
                .height(with(density) { rowHeightPx.toDp() }),
        ) {
            SwipeAction(
                icon = VelaIcons.ArrowUpRight,
                label = swipeSendLabel,
                background = VelaTheme.colors.accentBase,
                onClick = onSwipeSend,
            )
            SwipeAction(
                icon = VelaIcons.Trash2,
                label = swipeDeleteLabel,
                background = VelaTheme.colors.errorBase,
                onClick = onSwipeDelete,
            )
        }
        ContactRowContent(
            contact = contact,
            avatarSize = avatarSize,
            selected = selected,
            modifier = Modifier
                .onSizeChanged { rowHeightPx = it.height }
                .offset { IntOffset(offsetX.value.roundToInt(), 0) }
                .background(VelaTheme.colors.bgBase)
                .draggable(
                    orientation = Orientation.Horizontal,
                    state = rememberDraggableState { delta ->
                        scope.launch {
                            offsetX.snapTo((offsetX.value + delta).coerceIn(-revealWidth, 0f))
                        }
                    },
                    onDragStopped = {
                        scope.launch {
                            val target = if (offsetX.value < -revealWidth / 2f) -revealWidth else 0f
                            offsetX.animateTo(target, revealSpec)
                        }
                    },
                )
                .clickable(onClick = onClick),
        )
    }
}

@Composable
private fun ContactRowContent(
    contact: ContactModel,
    avatarSize: Dp,
    selected: Boolean,
    modifier: Modifier = Modifier,
) {
    val colors = VelaTheme.colors
    Row(
        modifier = modifier
            .fillMaxWidth()
            .then(if (selected) Modifier.background(colors.bgRaised) else Modifier)
            .heightIn(min = VelaSizing.hitTarget)
            .padding(vertical = VelaSpacing.lg),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        IdenticonAvatar(
            seed = contact.addressFull,
            size = avatarSize,
            contentDescription = contact.name,
        )
        Spacer(modifier = Modifier.width(VelaSpacing.lg))
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = contact.name,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.semibold,
                fontSize = VelaTextSize.lg,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                text = contact.addressDisplay,
                color = colors.fgMuted,
                fontFamily = VelaMonoFontFamily,
                fontWeight = VelaFontWeight.regular,
                fontSize = VelaTextSize.base,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

@Composable
private fun SwipeAction(
    icon: ImageVector,
    label: String,
    background: Color,
    onClick: () -> Unit,
) {
    Column(
        modifier = Modifier
            .width(ContactsMetrics.swipeActionWidth)
            .fillMaxHeight()
            .background(background)
            .clickable(onClick = onClick),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Icon(
            imageVector = icon,
            contentDescription = null,
            tint = VelaOnAccent,
            modifier = Modifier.size(VelaIconSize.lg),
        )
        Spacer(modifier = Modifier.height(VelaSpacing.sm))
        Text(
            text = label,
            color = VelaOnAccent,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.medium,
            fontSize = VelaTextSize.sm,
            textAlign = TextAlign.Center,
            maxLines = 1,
        )
    }
}
