package app.getvela.wallet.core.designsystem.components

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.focusable
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.disabled
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.onClick
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.style.TextAlign
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaAccentFill
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaOnAccent
import app.getvela.wallet.core.designsystem.tokens.VelaOpacity
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.platform.VelaHaptic
import app.getvela.wallet.core.platform.rememberVelaHaptic
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/**
 * R22 (review 1-3): the confirm's friction, as the core sets it
 * (`ConfirmState.hold_ms` / a multisig door's `hold_ms`) — the ONE component
 * the dApp signing sheet's confirm, the hand-off card's Open and every
 * multisig sign / propose / approve / execute button use.
 *
 * - [holdMs] `0`: a plain button, exactly as before ([VelaValueButton], or
 *   [VelaDangerButton] when [danger]).
 * - Otherwise a **press and hold**: a fill grows across the button over
 *   [holdMs]; releasing early does nothing (the fill drains); holding to the
 *   end fires [onConfirm] once. "Hold to confirm" ([holdHint]) is said under
 *   it and is the accessibility hint.
 * - **The accessible equivalent** (WCAG 2.5.1 / 2.5.7): a TalkBack, Switch
 *   Access or any accessibility-service click, and Enter / Space from a
 *   keyboard, do not hold — the FIRST activation arms the button and
 *   announces [confirmAgain] ("Press again to confirm", a polite live
 *   region); a SECOND within [windowMs] confirms; after the window it
 *   disarms by itself. Pointer and touch users get the hold.
 *
 * The rules (how long, when) are the core's; [HoldGate] is only the
 * component's own state, kept apart so a JVM test can drive it.
 */
@Composable
fun VelaHoldToConfirmButton(
    text: String,
    onConfirm: () -> Unit,
    holdMs: Int,
    holdHint: String,
    confirmAgain: String,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    loading: Boolean = false,
    /** The danger style (`anyway || danger`): the error fill, never the accent. */
    danger: Boolean = false,
    /** The neutral secondary style for a plain press beside a main one (r16-4); a hold is never secondary. */
    secondary: Boolean = false,
    windowMs: Long = HoldGate.DEFAULT_WINDOW_MS,
    /**
     * Keep the hint line's room while there is no hold (the signing sheet's
     * bottom-anchored footer: a verdict that changes the friction moves nothing).
     */
    keepHintRoom: Boolean = false,
    /** The hint line's test tag. */
    hintTag: String = HOLD_HINT_TAG,
) {
    if (holdMs <= 0) {
        Column(modifier = Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(VelaSpacing.sm)) {
            when {
                danger -> VelaDangerButton(text = text, onClick = onConfirm, modifier = modifier, enabled = enabled, loading = loading)
                secondary -> VelaSecondaryButton(text = text, onClick = onConfirm, modifier = modifier, enabled = enabled, loading = loading)
                else -> VelaValueButton(text = text, onClick = onConfirm, modifier = modifier, enabled = enabled, loading = loading)
            }
            if (keepHintRoom) HintLine(holdHint, visible = false, tag = hintTag)
        }
        return
    }
    val colors = VelaTheme.colors
    val haptic = rememberVelaHaptic()
    val scope = rememberCoroutineScope()
    val confirm by rememberUpdatedState(onConfirm)
    val progress = remember { Animatable(0f) }
    var armed by remember { mutableStateOf(false) }
    val active = enabled && !loading
    val gate = remember(windowMs) { HoldGate(windowMs) }

    // The accessible path's window: armed, it disarms by itself.
    LaunchedEffect(armed) {
        if (armed) {
            delay(windowMs)
            gate.expire()
            armed = false
        }
    }
    // Shut or busy: nothing stays armed or half-held.
    LaunchedEffect(active) {
        if (!active) {
            gate.expire()
            armed = false
            progress.snapTo(0f)
        }
    }

    fun assistive(): Boolean {
        if (!active) return false
        when (gate.activate(System.currentTimeMillis())) {
            HoldGate.Step.Armed -> {
                armed = true
                haptic(VelaHaptic.Press)
            }
            HoldGate.Step.Confirmed -> {
                armed = false
                haptic(VelaHaptic.Press)
                confirm()
            }
        }
        return true
    }

    val fill = if (danger) colors.errorBase else VelaAccentFill
    val ink = VelaOnAccent
    Column(modifier = Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(VelaSpacing.sm)) {
        Box(
            modifier = modifier
                .fillMaxWidth()
                .graphicsLayer { alpha = if (enabled) 1f else VelaOpacity.disabled }
                .heightIn(min = VelaSizing.controlLg)
                .clip(RoundedCornerShape(VelaRadius.full))
                .background(fill)
                // The hold's progress: a veil growing from the start edge.
                .drawBehind {
                    val p = progress.value
                    if (p > 0f) drawRect(HOLD_VEIL, size = Size(size.width * p, size.height))
                }
                .semantics(mergeDescendants = true) {
                    role = Role.Button
                    stateDescription = if (armed) confirmAgain else holdHint
                    if (!active) disabled()
                    // TalkBack / Switch Access / any accessibility click: the two-press path.
                    onClick(label = holdHint) { assistive() }
                }
                .onKeyEvent { event ->
                    val activation = event.key == Key.Enter || event.key == Key.Spacebar || event.key == Key.NumPadEnter
                    if (activation && event.type == KeyEventType.KeyUp) assistive() else activation
                }
                .focusable(enabled = active)
                .pointerInput(active, holdMs) {
                    if (!active) return@pointerInput
                    detectTapGestures(
                        onPress = {
                            // A touch is the hold — never the two-press path.
                            gate.expire()
                            armed = false
                            haptic(VelaHaptic.Press)
                            var done = false
                            val run: Job = scope.launch {
                                progress.snapTo(0f)
                                progress.animateTo(1f, tween(durationMillis = holdMs, easing = LinearEasing))
                                done = true
                                haptic(VelaHaptic.Press)
                                confirm()
                            }
                            tryAwaitRelease()
                            if (!done) {
                                // Released early: nothing happens; the fill drains.
                                run.cancel()
                                scope.launch { progress.animateTo(0f, tween(durationMillis = DRAIN_MS)) }
                            } else {
                                scope.launch { progress.snapTo(0f) }
                            }
                        },
                    )
                },
            contentAlignment = Alignment.Center,
        ) {
            ButtonLabel(text = text, color = ink, loading = loading)
        }
        HintLine(if (armed) confirmAgain else holdHint, visible = true, tag = hintTag)
    }
}

@Composable
private fun HintLine(text: String, visible: Boolean, tag: String) {
    Text(
        text = text,
        color = VelaTheme.colors.fgMuted,
        fontFamily = VelaFontFamily,
        fontSize = VelaTextSize.sm,
        textAlign = TextAlign.Center,
        modifier = Modifier
            .fillMaxWidth()
            .testTag(tag)
            .then(
                if (visible) {
                    // "Press again to confirm" is announced when it is said.
                    Modifier.semantics { liveRegion = LiveRegionMode.Polite }
                } else {
                    Modifier.alpha(0f).clearAndSetSemantics {}
                },
            ),
    )
}

/** The hold's progress veil: the fill a shade darker as it grows (both themes). */
private val HOLD_VEIL = Color.Black.copy(alpha = 0.24f)

/** How fast an early release drains the fill. */
private const val DRAIN_MS = 160

/** The hint line under a hold button. */
const val HOLD_HINT_TAG = "vela.hold.hint"

/**
 * The accessible path's state (R22): first activation arms, a second within
 * the window confirms, after the window the next one arms again. Pure; the
 * window is the core's (`confirmAgainWindowMs`).
 */
class HoldGate(private val windowMs: Long) {
    enum class Step { Armed, Confirmed }

    private var armedAt: Long? = null

    /** One accessible activation at [nowMs]. */
    fun activate(nowMs: Long): Step {
        val at = armedAt
        return if (at != null && nowMs - at <= windowMs) {
            armedAt = null
            Step.Confirmed
        } else {
            armedAt = nowMs
            Step.Armed
        }
    }

    /** The window ran out, or a touch started a hold: disarmed. */
    fun expire() {
        armedAt = null
    }

    val armed: Boolean get() = armedAt != null

    companion object {
        /** The core's window (`confirm_again_window_ms`), read once; 5 s if the core cannot be asked. */
        val DEFAULT_WINDOW_MS: Long by lazy {
            runCatching { uniffi.vela_core_uniffi.confirmAgainWindowMs().toLong() }.getOrDefault(5_000L)
        }

        /** The core's hold (`hold_to_confirm_ms`), read once; 1.5 s if the core cannot be asked. */
        val HOLD_MS: Int by lazy {
            runCatching { uniffi.vela_core_uniffi.holdToConfirmMs().toInt() }.getOrDefault(1_500)
        }
    }
}
