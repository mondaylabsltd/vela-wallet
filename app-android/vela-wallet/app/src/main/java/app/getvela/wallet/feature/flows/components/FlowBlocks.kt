package app.getvela.wallet.feature.flows.components

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.ui.draw.scale
import androidx.compose.ui.geometry.Offset
import androidx.compose.runtime.setValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.OffsetMapping
import androidx.compose.ui.text.input.TransformedText
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.tween
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.times
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaBorder
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaLeading
import app.getvela.wallet.core.designsystem.tokens.VelaMonoFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaMotion
import app.getvela.wallet.core.designsystem.tokens.VelaOnAccent
import app.getvela.wallet.core.designsystem.tokens.VelaOpacity
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.identicon.IdenticonImage
import app.getvela.wallet.feature.flows.AddressCardModel
import app.getvela.wallet.feature.flows.AmountFieldModel
import app.getvela.wallet.feature.flows.FeeRowModel
import app.getvela.wallet.feature.flows.FeeSpeedModel
import app.getvela.wallet.feature.flows.ReceiptStage
import app.getvela.wallet.feature.flows.RecipientActionModel
import app.getvela.wallet.feature.flows.RecipientFieldModel
import app.getvela.wallet.feature.flows.SendTokenCardModel
import app.getvela.wallet.feature.flows.SummaryLineModel
import app.getvela.wallet.feature.flows.TokenMarkModel
import app.getvela.wallet.feature.wallet.components.TokenIcon
import kotlin.math.min
import uniffi.vela_core_uniffi.qrMatrix

/** The blocks of the wallet flows (spec 021 components 8, 16–22, 24–26). */

/**
 * The account card above every QR (component 17): whose address this is,
 * spelled out in full, with one copy button.
 *
 * The address wraps to exactly two lines and never truncates. R2 is the screen
 * a person reads an address OFF, and an ellipsis in the middle of it would
 * defeat the only job the screen has.
 */
@Composable
fun AddressCard(
    account: AddressCardModel,
    modifier: Modifier = Modifier,
    copied: Boolean = false,
    onCopy: () -> Unit = {},
) {
    val colors = VelaTheme.colors
    Row(
        modifier = modifier
            .fillMaxWidth()
            .padding(vertical = VelaSpacing.lg),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        IdenticonImage(seed = account.identiconSeed, size = VelaSizing.doneAvatar, name = account.name)
        Spacer(modifier = Modifier.width(VelaSpacing.lg))
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = account.name,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.semibold,
                fontSize = VelaTextSize.lg,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            listOf(account.lines.first, account.lines.second)
                .filter { it.isNotEmpty() }
                .forEach { line ->
                    Text(
                        text = line,
                        color = colors.fgMuted,
                        fontFamily = VelaMonoFontFamily,
                        fontSize = VelaTextSize.sm,
                        maxLines = 1,
                    )
                }
        }
        FlowIconButton(
            icon = if (copied) VelaIcons.Check else VelaIcons.Copy,
            label = account.copyLabel,
            tint = if (copied) colors.successBase else colors.fgMuted,
            onClick = onCopy,
        )
    }
}

/**
 * The receive QR card (component 18) — R2, R3 and R4.
 *
 * Two decisions carried from the SPEC sheet:
 *
 * - **A fixed square.** The card does not scale with the text. At 1.35x the
 *   copy around it grows and the screen scrolls; the code stays the size it
 *   was, because a code that shrinks to make room for its caption stops
 *   scanning.
 * - **Something in the middle.** The network mark on R2, the token on R3, the
 *   account's own identicon on the share card — R4's centre is an anti-forgery
 *   mark: a card whose address was doctored would carry artwork that no longer
 *   matches the characters printed under it.
 *
 * **The modules are real when a payload is given.** They were the deterministic
 * demo pattern spec 015 established, on the reasoning that a code which looked
 * scannable but was not would be worse than one that plainly is not — and by
 * spec 041 the screen around it had become entirely real: this person's name,
 * this person's address, their own identicon, at full size. The pattern stopped
 * plainly not being a code and started looking exactly like one, which is the
 * failure that reasoning was guarding against, arrived from the other side.
 *
 * The gallery still passes no payload, so its canon is unchanged.
 */
@Composable
fun QrCard(
    label: String,
    modifier: Modifier = Modifier,
    /**
     * What the code encodes.
     *
     * `null` draws the placeholder pattern — the gallery's canon, so its
     * screenshots stay identical between runs. **A real screen must always pass
     * this.** A receive code that encodes nothing looks completely finished and
     * does nothing when somebody scans it, which is the one failure on this
     * screen that is not discovered until money fails to arrive.
     */
    payload: String? = null,
    centre: (@Composable () -> Unit)? = null,
) {
    val ink = VelaTheme.colors.fixed.shadowInk
    // The SAME encoder every Vela platform uses, over the bridge. Four
    // hand-rolled encoders would be four subtly different codes, and the one
    // that failed would fail only on somebody else's camera.
    val matrix = remember(payload) {
        payload?.let { text -> runCatching { qrMatrix(text) }.getOrNull() }
    }
    Box(
        modifier = modifier
            .size(VelaSizing.qrCard)
            .semantics { contentDescription = label }
            // White in BOTH appearances: a code is read by a camera, and
            // inverting it in dark mode is the classic way to make one
            // unscannable.
            .background(VelaOnAccent, RoundedCornerShape(VelaRadius.xl))
            .padding(VelaSpacing.xl3),
        contentAlignment = Alignment.Center,
    ) {
        Canvas(modifier = Modifier.fillMaxWidth().height(VelaSizing.qrCard - VelaSpacing.xl3 * 2)) {
            val cells = matrix?.width?.toInt() ?: QR_MODULES
            val module = min(size.width, size.height) / cells
            for (r in 0 until cells) {
                for (c in 0 until cells) {
                    val dark = matrix?.modules?.getOrNull(r * cells + c) ?: qrCell(r, c)
                    if (dark) {
                        drawRect(
                            color = ink,
                            topLeft = androidx.compose.ui.geometry.Offset(c * module, r * module),
                            size = Size(module, module),
                        )
                    }
                }
            }
        }
        centre?.let {
            Box(
                modifier = Modifier
                    // The cut-out reads as part of the card, so it takes the
                    // card's white rather than a theme surface that would flip
                    // underneath it.
                    .background(VelaOnAccent, CircleShape)
                    .padding(VelaSpacing.xs),
            ) { it() }
        }
    }
}

/**
 * The deterministic demo pattern (spec 015 data-model.md, ported here).
 *
 * Three standard finder squares plus xorshift32-seeded noise. Identical on
 * every platform and every run, so screenshots diff cleanly. Denser than the
 * spec-015 placeholder because R2 draws the code large, where 21 modules read
 * as a chequerboard rather than a code.
 */
private const val QR_MODULES = 29
private const val QR_SEED = 0xbeef

private val QR_CELLS: Array<BooleanArray> by lazy {
    var s = QR_SEED
    fun next(): Int {
        s = s xor (s shl 13)
        s = s xor (s ushr 17)
        s = s xor (s shl 5)
        return s
    }

    val n = QR_MODULES
    Array(n) { r ->
        BooleanArray(n) { c ->
            val inFinder = (r < 7 && c < 7) || (r < 7 && c >= n - 7) || (r >= n - 7 && c < 7)
            if (inFinder) {
                val lr = if (r < 7) r else r - (n - 7)
                val lc = if (c < 7) c else c - (n - 7)
                minOf(lr, lc, 6 - lr, 6 - lc) != 1
            } else {
                if ((next() and 3) == 0) false else next() % 2 == 0
            }
        }
    }
}

private fun qrCell(r: Int, c: Int): Boolean = QR_CELLS[r][c]

/**
 * SD2's amount (component 8): the number, big and centred, with its fiat
 * equivalent and the toggle that swaps which of the two you type.
 *
 * The figure is the largest type on the screen because it is the one thing the
 * person came to decide. The fiat line stays subordinate even when the
 * denominations swap — the amount being ENTERED leads, whichever it is.
 */
@Composable
fun AmountInput(
    amount: AmountFieldModel,
    modifier: Modifier = Modifier,
    onDenom: () -> Unit = {},
    /** Spec 043: present ⇒ the figure is typed here, in the same type as the drawn one. */
    onValueChange: ((String) -> Unit)? = null,
) {
    val colors = VelaTheme.colors
    Column(
        modifier = modifier
            .fillMaxWidth()
            .padding(vertical = VelaSpacing.xl3),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        val heroStyle = TextStyle(
            color = colors.fgBase,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.bold,
            fontSize = VelaTextSize.xl5,
            lineHeight = VelaLeading.amountHero * VelaTextSize.xl5,
            textAlign = TextAlign.Center,
        )
        // Issue 231: the unit ON the figure — a sign tight before it ("$4.00"),
        // a ticker or code after it, quieter and smaller ("0.00075 BNB").
        val units = remember(amount.unitPrefix, amount.unitSuffix, colors.fgMuted) {
            amountUnits(amount.unitPrefix, amount.unitSuffix, colors.fgMuted)
        }
        if (onValueChange != null && amount.raw != null) {
            // Local echo (spec 043, device-found): every keystroke goes to the
            // machine, but the field shows what was typed until the machine's
            // OWN value changes for another reason (Max, ⇄). Driving the field
            // straight from the round trip dropped characters under fast
            // typing — "0.001" arrived as ".01".
            var typed by remember { mutableStateOf(amount.raw) }
            // Every value this field SENT, so a machine view that lags behind
            // the typing (device-found: a 42-character paste lost six
            // characters to stale intermediate views) is recognised as an echo
            // and ignored; only a value the field never sent — Max, ⇄, a
            // picked contact, the core's own normalisation — resyncs it.
            val sent = remember { ArrayDeque<String>().apply { addLast(amount.raw) } }
            LaunchedEffect(amount.raw) { if (amount.raw !in sent) typed = amount.raw }
            BasicTextField(
                value = typed,
                onValueChange = { next ->
                    typed = next
                    sent.addLast(next)
                    if (sent.size > 256) sent.removeFirst()
                    onValueChange(next)
                },
                singleLine = true,
                textStyle = heroStyle,
                cursorBrush = SolidColor(colors.accentBase),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
                // The unit is drawn, never typed: the field's text stays the bare figure.
                visualTransformation = units,
                modifier = Modifier.fillMaxWidth(),
                decorationBox = { inner ->
                    Box(contentAlignment = Alignment.Center) {
                        // The placeholder carries the unit; the empty field draws nothing
                        // (it drew " XDAI" too, on top of the placeholder's — device-found).
                        if (typed.isEmpty()) Text(text = units.filter(AnnotatedString("0")).text, style = heroStyle.copy(color = colors.fgSubtle), maxLines = 1)
                        inner()
                    }
                },
            )
        } else {
            Text(text = units.filter(AnnotatedString(amount.value)).text, style = heroStyle, maxLines = 1)
        }
        Spacer(modifier = Modifier.height(VelaSpacing.sm))
        if (amount.denomShown) {
            // Issue 197: shown only where the core offers the swap, and live
            // only where pressing it would change something.
            Row(
                modifier = Modifier
                    .clickable(enabled = amount.denomEnabled, onClick = onDenom)
                    .alpha(if (amount.denomEnabled) 1f else VelaOpacity.disabled)
                    .padding(VelaSpacing.xs),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    text = amount.fiat,
                    color = colors.fgMuted,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.lg,
                    maxLines = 1,
                )
                Spacer(modifier = Modifier.width(VelaSpacing.xs))
                Icon(
                    imageVector = VelaIcons.ChevronsUpDown,
                    contentDescription = amount.denomLabel,
                    tint = colors.fgMuted,
                    modifier = Modifier.size(VelaIconSize.sm),
                )
            }
        } else if (amount.fiat.isNotEmpty()) {
            Text(
                text = amount.fiat,
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.lg,
                maxLines = 1,
                modifier = Modifier.padding(VelaSpacing.xs),
            )
        }
    }
}

/**
 * The amount's unit as a transformation of what is typed: the sign before the
 * digits, the ticker or code after them (set off by a space, smaller and
 * lighter, as the web draws it). The caret maps through it, so the unit can be
 * neither typed over nor deleted.
 */
private fun amountUnits(prefix: String?, suffix: String?, unitColor: Color): VisualTransformation {
    if (prefix.isNullOrEmpty() && suffix.isNullOrEmpty()) return VisualTransformation.None
    val lead = prefix.orEmpty()
    val tail = suffix?.let { " $it" }.orEmpty()
    return VisualTransformation { text ->
        // Nothing typed: no unit either. The placeholder beside it already says
        // "0 XDAI"; a unit on the empty field drew a second "XDAI" over it.
        if (text.isEmpty()) return@VisualTransformation TransformedText(text, OffsetMapping.Identity)
        val drawn = buildAnnotatedString {
            withStyle(SpanStyle(color = unitColor)) { append(lead) }
            append(text)
            withStyle(SpanStyle(color = unitColor, fontSize = VelaTextSize.xl3, fontWeight = VelaFontWeight.medium)) { append(tail) }
        }
        val length = text.length
        TransformedText(
            drawn,
            object : OffsetMapping {
                override fun originalToTransformed(offset: Int): Int = offset + lead.length
                override fun transformedToOriginal(offset: Int): Int = (offset - lead.length).coerceIn(0, length)
            },
        )
    }
}

/**
 * The big signed amount (component 19): A2's and A3's transaction figure, T2's
 * balance, SD3's confirmation total.
 *
 * Money in is green; money out is plain ink, not red. Red in this product means
 * something went wrong, and a transfer you chose to make did not.
 */
@Composable
fun AmountHero(
    amount: String,
    fiat: String,
    modifier: Modifier = Modifier,
    positive: Boolean = false,
    centred: Boolean = false,
) {
    val colors = VelaTheme.colors
    Column(
        modifier = modifier
            .fillMaxWidth()
            .padding(top = VelaSpacing.lg, bottom = VelaSpacing.xl),
        horizontalAlignment = if (centred) Alignment.CenterHorizontally else Alignment.Start,
    ) {
        Text(
            text = amount,
            color = if (positive) colors.successBase else colors.fgBase,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.bold,
            fontSize = VelaTextSize.xl4,
            lineHeight = VelaLeading.amountHero * VelaTextSize.xl4,
            maxLines = 1,
        )
        Text(
            text = fiat,
            color = colors.fgSubtle,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.lg,
            maxLines = 1,
        )
    }
}

/**
 * The send receipt's centrepiece (component 20) — SD4a's spinner, SD4b's clock,
 * SD4c's tick, and the failure cross.
 *
 * One disc size for all four so the mark does not resize as the transaction
 * moves between them: the person is watching this circle, and a circle that
 * jumps when the state changes reads as a new screen rather than as progress on
 * the one they were already looking at.
 *
 * Issue 199 (the web's `StatusHero`): the wait can run to minutes, and a still
 * grey clock reads as a hang. So the submitted disc wears a ring OUTSIDE it
 * that fills as the chain's usual time passes ([progress], the screen's clock
 * and curve), and breathes while it does; `null` while submitted means "no
 * estimate for this chain" and the ring circles instead of filling. The same
 * ring closes and turns green on confirmation, so the tick arrives as the end
 * of what the person was watching.
 */
@Composable
fun StatusHero(
    stage: ReceiptStage,
    title: String,
    captions: List<String>,
    modifier: Modifier = Modifier,
    progress: Float? = null,
) {
    val colors = VelaTheme.colors
    val (disc, tint) = when (stage) {
        ReceiptStage.Submitting -> colors.bgSunken to colors.accentBase
        ReceiptStage.Submitted -> colors.bgSunken to colors.fgMuted
        ReceiptStage.Confirmed -> colors.successSoft to colors.successBase
        ReceiptStage.Failed -> colors.errorSoft to colors.errorBase
    }
    Column(
        modifier = modifier
            .fillMaxWidth()
            .padding(top = VelaSpacing.xl6, bottom = VelaSpacing.xl3),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        val ringed = stage == ReceiptStage.Submitted || stage == ReceiptStage.Confirmed
        val drawn by animateFloatAsState(
            targetValue = if (stage == ReceiptStage.Confirmed) 1f else (progress ?: 0.25f),
            // One second per step, linear, because the screen ticks once a
            // second: the arc is always mid-move. Confirmation closes it at the
            // slow duration.
            animationSpec = tween(
                durationMillis = if (stage == ReceiptStage.Confirmed) VelaMotion.durationSlow else 1000,
                easing = LinearEasing,
            ),
            label = "receiptRing",
        )
        val arcColor by animateColorAsState(
            targetValue = if (stage == ReceiptStage.Confirmed) colors.successBase else colors.accentBase,
            label = "receiptRingColor",
        )
        val wait = rememberInfiniteTransition(label = "receiptWait")
        val roam by wait.animateFloat(
            initialValue = 0f,
            targetValue = 360f,
            animationSpec = infiniteRepeatable(animation = tween(durationMillis = 2400, easing = LinearEasing)),
            label = "receiptRingRoam",
        )
        val breathe by wait.animateFloat(
            initialValue = 1f,
            targetValue = 1.04f,
            animationSpec = infiniteRepeatable(
                animation = tween(durationMillis = 1200),
                repeatMode = RepeatMode.Reverse,
            ),
            label = "receiptDiscBreathe",
        )
        val trackColor = colors.borderBase
        // The ring clears the disc by a gutter rather than outlining it; the
        // disc keeps its one size inside.
        Box(
            modifier = Modifier.size(VelaSizing.statusHero + VelaSpacing.md * 2),
            contentAlignment = Alignment.Center,
        ) {
            if (ringed) {
                Canvas(
                    modifier = Modifier
                        .matchParentSize()
                        .rotate(if (stage == ReceiptStage.Submitted && progress == null) roam else 0f),
                ) {
                    val stroke = VelaBorder.emphasis.toPx() * 1.25f
                    val inset = stroke / 2f
                    val arc = Size(size.width - stroke, size.height - stroke)
                    if (stage != ReceiptStage.Confirmed) {
                        drawArc(
                            color = trackColor,
                            startAngle = 0f,
                            sweepAngle = 360f,
                            useCenter = false,
                            topLeft = Offset(inset, inset),
                            size = arc,
                            style = Stroke(width = stroke),
                        )
                    }
                    drawArc(
                        color = arcColor,
                        startAngle = -90f,
                        sweepAngle = 360f * drawn,
                        useCenter = false,
                        topLeft = Offset(inset, inset),
                        size = arc,
                        style = Stroke(width = stroke, cap = StrokeCap.Round),
                    )
                }
            }
        Box(
            modifier = Modifier
                .size(VelaSizing.statusHero)
                .scale(if (stage == ReceiptStage.Submitted) breathe else 1f)
                .background(disc, CircleShape),
            contentAlignment = Alignment.Center,
        ) {
            when (stage) {
                ReceiptStage.Submitting -> SpinnerArc(color = tint)
                ReceiptStage.Submitted -> Icon(
                    imageVector = VelaIcons.Clock,
                    contentDescription = null,
                    tint = tint,
                    modifier = Modifier.size(VelaIconSize.xl2),
                )
                ReceiptStage.Confirmed -> Icon(
                    imageVector = VelaIcons.Check,
                    contentDescription = null,
                    tint = tint,
                    modifier = Modifier.size(VelaIconSize.xl2),
                )
                ReceiptStage.Failed -> Icon(
                    imageVector = VelaIcons.Close,
                    contentDescription = null,
                    tint = tint,
                    modifier = Modifier.size(VelaIconSize.xl2),
                )
            }
        }
        }
        Spacer(modifier = Modifier.height(VelaSpacing.xl - VelaSpacing.md))
        Text(
            text = title,
            color = colors.fgBase,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.bold,
            fontSize = VelaTextSize.xl2,
            lineHeight = VelaLeading.hero * VelaTextSize.xl2,
            textAlign = TextAlign.Center,
        )
        captions.forEachIndexed { index, caption ->
            Spacer(modifier = Modifier.height(VelaSpacing.sm))
            Text(
                text = caption,
                // The second caption is the one that says "you can leave" —
                // true, useful, and not what the person is waiting to read.
                color = if (index == 0) colors.fgMuted else colors.fgSubtle,
                fontFamily = VelaFontFamily,
                fontSize = if (index == 0) VelaTextSize.base else VelaTextSize.sm,
                textAlign = TextAlign.Center,
            )
        }
    }
}

/** One revolution at the CTA spinner's speed: one wait speed in the product. */
@Composable
private fun SpinnerArc(color: Color, modifier: Modifier = Modifier) {
    val transition = rememberInfiniteTransition(label = "receiptSpinner")
    val angle = transition.animateFloat(
        initialValue = 0f,
        targetValue = 360f,
        animationSpec = infiniteRepeatable(
            animation = tween(durationMillis = VelaMotion.durationSlow * 2, easing = LinearEasing),
        ),
        label = "receiptSpinnerAngle",
    )
    Canvas(
        modifier = modifier
            .size(VelaIconSize.xl2)
            .rotate(angle.value),
    ) {
        drawArc(
            color = color,
            startAngle = 0f,
            sweepAngle = 270f,
            useCenter = false,
            style = Stroke(width = VelaBorder.emphasis.toPx() * 2, cap = StrokeCap.Round),
        )
    }
}

/**
 * T4's guidance card (component 21): the CTA on top, then the question a person
 * with an empty asset list is actually asking — "it arrived, so why can't I see
 * it?" — and its answer.
 *
 * The question is set as a heading rather than as body copy because it is the
 * part someone scanning the screen needs to recognise as theirs.
 */
@Composable
fun HintCard(
    title: String,
    body: String,
    modifier: Modifier = Modifier,
    cta: (@Composable () -> Unit)? = null,
) {
    val colors = VelaTheme.colors
    Column(
        modifier = modifier
            .fillMaxWidth()
            .border(VelaBorder.hairline, colors.borderBase, RoundedCornerShape(VelaRadius.xl))
            .padding(VelaSpacing.lg),
    ) {
        cta?.let {
            it()
            Spacer(modifier = Modifier.height(VelaSpacing.lg))
        }
        Text(
            text = title,
            color = colors.fgBase,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.semibold,
            fontSize = VelaTextSize.base,
        )
        Spacer(modifier = Modifier.height(VelaSpacing.md))
        Text(
            text = body,
            color = colors.fgMuted,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.sm,
            lineHeight = VelaTextSize.sm * VelaLeading.relaxed,
        )
    }
}

/**
 * The inline explanation banner (component 22): SD1b's "these are greyed out
 * because a multi-token send stays on one network", SD2d's "every token goes to
 * the same address".
 *
 * It exists because both screens do something surprising — grey out rows a
 * person can see, or accept one address for several tokens — and the cheapest
 * fix for a surprise is to say why, next to it.
 */
@Composable
fun NoticeBanner(
    text: String,
    modifier: Modifier = Modifier,
    mark: TokenMarkModel? = null,
) {
    val colors = VelaTheme.colors
    Row(
        modifier = modifier
            .fillMaxWidth()
            .background(colors.bgRaised, RoundedCornerShape(VelaRadius.lg))
            .padding(horizontal = VelaSpacing.lg, vertical = VelaSpacing.md),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        mark?.let {
            TokenIcon(mark = it, inline = true)
            Spacer(modifier = Modifier.width(VelaSpacing.md))
        }
        Text(
            text = text,
            color = colors.fgMuted,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.sm,
            lineHeight = VelaTextSize.sm * VelaLeading.normal,
        )
    }
}

/**
 * The send form's token card (component 16): which token is being sent, off
 * which chain, out of how much — and the Max that fills the amount with all of
 * it.
 */
@Composable
fun TokenHeaderCard(
    token: SendTokenCardModel,
    modifier: Modifier = Modifier,
    onMax: () -> Unit = {},
) {
    val colors = VelaTheme.colors
    Row(
        modifier = modifier
            .fillMaxWidth()
            .background(colors.bgRaised, RoundedCornerShape(VelaRadius.lg))
            .padding(VelaSpacing.lg),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        TokenIcon(mark = token.mark)
        Spacer(modifier = Modifier.width(VelaSpacing.lg))
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = token.symbol,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.semibold,
                fontSize = VelaTextSize.lg,
                maxLines = 1,
            )
            Text(
                text = token.detail,
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.sm,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        token.max?.let { max ->
            Spacer(modifier = Modifier.width(VelaSpacing.md))
            Box(
                modifier = Modifier
                    .background(colors.bgSunken, CircleShape)
                    .clickable(onClick = onMax)
                    .padding(horizontal = VelaSpacing.lg, vertical = VelaSpacing.sm),
            ) {
                Text(
                    text = max,
                    color = colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.semibold,
                    fontSize = VelaTextSize.sm,
                    maxLines = 1,
                )
            }
        }
    }
}

/**
 * SD2's and SD2d's recipient field.
 *
 * The identicon sits INSIDE the field, next to the characters it is drawn from.
 * Address poisoning works by matching the first and last few characters of an
 * address you have used before; the artwork is the part that does not match,
 * and it only helps if it is where the eye already is.
 */
@Composable
fun RecipientField(
    field: RecipientFieldModel,
    modifier: Modifier = Modifier,
    onPick: () -> Unit = {},
    onScan: () -> Unit = {},
    /** Spec 043: present ⇒ the address is typed or pasted here. */
    onValueChange: ((String) -> Unit)? = null,
) {
    val colors = VelaTheme.colors
    Column(modifier = modifier.fillMaxWidth()) {
        Text(
            text = field.label,
            color = colors.fgSubtle,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.sm,
        )
        Spacer(modifier = Modifier.height(VelaSpacing.sm))
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .background(colors.bgRaised, RoundedCornerShape(VelaRadius.lg))
                .padding(VelaSpacing.lg),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            IdenticonImage(seed = field.identiconSeed, size = VelaIconSize.xl2, name = field.name)
            Spacer(modifier = Modifier.width(VelaSpacing.md))
            Column(modifier = Modifier.weight(1f)) {
                if (onValueChange != null && field.raw != null) {
                    val monoStyle = TextStyle(
                        color = colors.fgBase,
                        fontFamily = VelaMonoFontFamily,
                        fontSize = VelaTextSize.base,
                    )
                    // Same local echo as the amount: a pasted address must
                    // not lose characters to the round trip.
                    var typed by remember { mutableStateOf(field.raw) }
                    // Every value this field SENT, so a machine view that lags behind
                    // the typing (device-found: a 42-character paste lost six
                    // characters to stale intermediate views) is recognised as an echo
                    // and ignored; only a value the field never sent — Max, ⇄, a
                    // picked contact, the core's own normalisation — resyncs it.
                    val sent = remember { ArrayDeque<String>().apply { addLast(field.raw) } }
                    LaunchedEffect(field.raw) { if (field.raw !in sent) typed = field.raw }
                    BasicTextField(
                        value = typed,
                        onValueChange = { next ->
                            typed = next
                            sent.addLast(next)
                            if (sent.size > 256) sent.removeFirst()
                            onValueChange(next)
                        },
                        maxLines = 2,
                        textStyle = monoStyle,
                        cursorBrush = SolidColor(colors.accentBase),
                        modifier = Modifier.fillMaxWidth(),
                        decorationBox = { inner ->
                            if (typed.isEmpty()) {
                                Text(text = "0x…", style = monoStyle.copy(color = colors.fgSubtle))
                            }
                            inner()
                        },
                    )
                } else {
                    listOf(field.lines.first, field.lines.second)
                        .filter { it.isNotEmpty() }
                        .forEach { line ->
                            Text(
                                text = line,
                                color = colors.fgBase,
                                fontFamily = VelaMonoFontFamily,
                                fontSize = VelaTextSize.base,
                            )
                        }
                }
            }
            // Issue #270: the field's doors look like buttons — a filled disc with
            // an edge, not a bare grey glyph that read as decoration. They are how
            // a wrong pick or a wrong scan is put right.
            val door = Modifier
                .clip(CircleShape)
                .background(colors.bgBase, CircleShape)
                .border(1.dp, colors.borderBase, CircleShape)
            FlowIconButton(
                icon = VelaIcons.UserRound,
                label = field.pickLabel,
                modifier = door,
                tint = colors.fgBase,
                onClick = onPick,
            )
            field.scanLabel?.let {
                Spacer(modifier = Modifier.width(VelaSpacing.sm))
                FlowIconButton(icon = VelaIcons.QrCode, label = it, modifier = door, tint = colors.fgBase, onClick = onScan)
            }
        }
        field.note?.let {
            Spacer(modifier = Modifier.height(VelaSpacing.sm))
            Text(
                text = it,
                color = colors.fgSubtle,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.sm,
            )
        }
    }
}

/**
 * SD2b's three ways to add a recipient (component 24): by hand, from contacts,
 * or from a spreadsheet.
 *
 * Outline pills, never accent: they add a ROW to a form, and the accent in this
 * product is reserved for the button that actually moves the money.
 */
@Composable
fun GhostPillRow(
    items: List<RecipientActionModel>,
    modifier: Modifier = Modifier,
    onSelect: (RecipientActionModel) -> Unit = {},
) {
    val colors = VelaTheme.colors
    Row(
        modifier = modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.sm),
    ) {
        items.forEach { item ->
            Box(
                modifier = Modifier
                    .weight(1f)
                    .border(VelaBorder.hairline, colors.borderStrong, CircleShape)
                    .clickable { onSelect(item) }
                    .padding(vertical = VelaSpacing.md),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    text = item.label,
                    color = colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.medium,
                    fontSize = VelaTextSize.sm,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
    }
}

/**
 * The total line above the fee (component 25).
 *
 * Deliberately not a fact row: that row is a fact ABOUT the transaction inside a
 * card, and this is a running sum of what the form above it currently says.
 */
@Composable
fun SummaryLine(summary: SummaryLineModel, modifier: Modifier = Modifier) {
    val colors = VelaTheme.colors
    Row(
        modifier = modifier
            .fillMaxWidth()
            .padding(vertical = VelaSpacing.md),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column {
            Text(
                text = summary.label,
                color = colors.fgSubtle,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.sm,
            )
            // "2.25 ETH left" — under the label, where the eye starts the line.
            summary.remaining?.let {
                Text(
                    text = it,
                    color = colors.fgMuted,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.xs,
                )
            }
        }
        Spacer(modifier = Modifier.weight(1f))
        Text(
            text = summary.value,
            // Over the balance: the refusal colour while the rows are still
            // being typed, not after Continue has been refused.
            color = if (summary.over) colors.errorBase else colors.fgBase,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.semibold,
            fontSize = VelaTextSize.base,
        )
    }
}

/**
 * The network-fee row (component 26), on every send form.
 *
 * A row and not a card: the fee is a fact about the transfer. The row opens
 * the fee-coin sheet; beside it — outside its own click, so measuring again
 * never opens the sheet — the refresh control (spec 068; Android's since 069),
 * and under it the stale line, whose room is kept so Continue never moves
 * under a thumb. The speed control ([FeeSpeedControl]) lives under that.
 */
@Composable
fun FeeRow(
    fee: FeeRowModel,
    modifier: Modifier = Modifier,
    onOpen: () -> Unit = {},
    onRefresh: (() -> Unit)? = null,
) {
    val colors = VelaTheme.colors
    Column(modifier = modifier.fillMaxWidth()) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Row(
                modifier = Modifier
                    .weight(1f)
                    .background(colors.bgRaised, RoundedCornerShape(VelaRadius.lg))
                    .clickable(onClick = onOpen)
                    .padding(VelaSpacing.lg),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    text = fee.label,
                    color = colors.fgMuted,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.base,
                )
                Spacer(modifier = Modifier.weight(1f))
                TokenIcon(mark = fee.mark, inline = true)
                Spacer(modifier = Modifier.width(VelaSpacing.sm))
                Text(
                    text = fee.value,
                    color = colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.base,
                    maxLines = 1,
                )
                Spacer(modifier = Modifier.width(VelaSpacing.sm))
                Icon(
                    imageVector = VelaIcons.ChevronRight,
                    contentDescription = fee.openLabel,
                    tint = colors.fgMuted,
                    modifier = Modifier.size(VelaIconSize.sm),
                )
            }
            if (fee.refreshLabel != null) {
                Spacer(modifier = Modifier.width(VelaSpacing.md))
                Box(
                    modifier = Modifier
                        .size(VelaSizing.hitTarget)
                        .background(colors.bgRaised, RoundedCornerShape(VelaRadius.lg))
                        .clickable(enabled = onRefresh != null) { onRefresh?.invoke() },
                    contentAlignment = Alignment.Center,
                ) {
                    // Dimmed while a measurement is out — whoever started it —
                    // so a second tap is never ambiguous.
                    Icon(
                        imageVector = VelaIcons.RefreshCw,
                        contentDescription = fee.refreshLabel,
                        tint = if (fee.refreshing) colors.fgSubtle else colors.fgMuted,
                        modifier = Modifier.size(VelaIconSize.sm),
                    )
                }
            }
        }
        if (fee.refreshLabel != null) {
            // Calm and muted: an old figure is not a fault. Always the line's
            // full height, so nothing jumps when it appears.
            Text(
                text = fee.staleNote ?: " ",
                color = colors.fgSubtle,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.sm,
                modifier = Modifier.padding(horizontal = VelaSpacing.lg, vertical = VelaSpacing.xs),
            )
        }
    }
}

/**
 * The speed control under the fee row (spec 068), folded until opened. Every
 * decision in it is the `fee_speed` core's (spec 069).
 *
 * Folded: the word and the tier in force — THEIR default, never a hardcoded
 * one. Opened: the one-shot promise first (a person about to change one
 * payment needs to know every later one is untouched), then three options —
 * name, its own fee, its gas bid, what it buys, a tick — or, on a network with
 * one speed, that one statement instead.
 */
@Composable
fun FeeSpeedControl(
    speed: FeeSpeedModel,
    modifier: Modifier = Modifier,
    onToggle: () -> Unit = {},
    onPick: (String) -> Unit = {},
) {
    val colors = VelaTheme.colors
    Column(modifier = modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clickable(onClick = onToggle)
                .padding(horizontal = VelaSpacing.lg, vertical = VelaSpacing.md),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(text = speed.label, color = colors.fgSubtle, fontFamily = VelaFontFamily, fontSize = VelaTextSize.base)
            Spacer(modifier = Modifier.weight(1f))
            Text(text = speed.value, color = colors.fgBase, fontFamily = VelaFontFamily, fontSize = VelaTextSize.base, maxLines = 1)
            Spacer(modifier = Modifier.width(VelaSpacing.sm))
            Icon(
                imageVector = VelaIcons.ChevronDown,
                contentDescription = null,
                tint = colors.fgMuted,
                modifier = Modifier.size(VelaIconSize.sm).rotate(if (speed.open) 180f else 0f),
            )
        }
        // Folded AND open: the screen must never say "Fast" over a Settings
        // row that says "Slow" without saying why.
        speed.freeNote?.let { SpeedNote(it) }
        if (!speed.open) return@Column
        val single = speed.singleNote
        if (single != null) {
            SpeedNote(single)
            return@Column
        }
        SpeedNote(speed.onceNote)
        speed.options.forEach { option ->
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .background(
                        if (option.selected) colors.bgRaised else Color.Transparent,
                        RoundedCornerShape(VelaRadius.md),
                    )
                    .clickable { onPick(option.id) }
                    .padding(horizontal = VelaSpacing.lg, vertical = VelaSpacing.md),
                verticalAlignment = Alignment.Top,
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text(
                            text = option.label,
                            color = if (option.selected) colors.accentBase else colors.fgBase,
                            fontFamily = VelaFontFamily,
                            fontWeight = if (option.selected) VelaFontWeight.semibold else VelaFontWeight.regular,
                            fontSize = VelaTextSize.base,
                            modifier = Modifier.weight(1f),
                        )
                        Text(text = option.value, color = colors.fgBase, fontFamily = VelaFontFamily, fontSize = VelaTextSize.base, maxLines = 1)
                    }
                    if (speed.gasPriceLine) {
                        // Named, because an unnamed "3,244 wei" under a fee reads
                        // as a second charge; held open empty while measuring.
                        Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
                            Text(
                                text = option.gasPrice?.let { "${speed.gasPriceLabel}  $it" } ?: " ",
                                color = colors.fgSubtle,
                                fontFamily = VelaMonoFontFamily,
                                fontSize = VelaTextSize.sm,
                                maxLines = 1,
                            )
                        }
                    }
                    Text(text = option.detail, color = colors.fgSubtle, fontFamily = VelaFontFamily, fontSize = VelaTextSize.sm)
                }
                Spacer(modifier = Modifier.width(VelaSpacing.md))
                Box(modifier = Modifier.size(VelaIconSize.sm)) {
                    if (option.selected) {
                        Icon(imageVector = VelaIcons.Check, contentDescription = null, tint = colors.accentBase, modifier = Modifier.size(VelaIconSize.sm))
                    }
                }
            }
        }
    }
}

@Composable
private fun SpeedNote(text: String) {
    Text(
        text = text,
        color = VelaTheme.colors.fgSubtle,
        fontFamily = VelaFontFamily,
        fontSize = VelaTextSize.sm,
        modifier = Modifier.padding(horizontal = VelaSpacing.lg).padding(bottom = VelaSpacing.sm),
    )
}
