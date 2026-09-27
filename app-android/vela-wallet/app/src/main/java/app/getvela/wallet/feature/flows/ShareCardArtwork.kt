package app.getvela.wallet.feature.flows

import android.content.Context
import android.graphics.Bitmap
import android.graphics.Paint
import android.graphics.Typeface
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.clipPath
import androidx.compose.ui.graphics.drawscope.scale
import androidx.compose.ui.graphics.drawscope.translate
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.PlatformTextStyle
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.TextMeasurer
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import androidx.core.content.res.ResourcesCompat
import app.getvela.wallet.R
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaMonoFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaOnAccent
import app.getvela.wallet.core.identicon.IdenticonImage
import app.getvela.wallet.feature.flows.components.QR_MODULES
import app.getvela.wallet.feature.flows.components.qrCell
import kotlin.math.floor
import kotlin.math.max
import kotlin.math.min
import kotlin.math.roundToInt
import uniffi.vela_core_uniffi.QrMatrix
import uniffi.vela_core_uniffi.shareCardQrMatrix

/**
 * R4 — what 保存图片 produces (spec 021; redrawn in spec 048, and again on
 * 2026-09-27 to the WeChat Pay collection card the founder holds it against).
 *
 * The composition, top to bottom: the app icon's orange field with the
 * headline and, under it, the one network this address may be paid on; a
 * white sheet with generous orange all round, holding the code — the
 * NETWORK's logo in its centre, where a payer's eye lands before it scans —
 * and under the code the account itself: its identicon on the left, the name
 * and the whole address in two mono lines beside it; then the field closes
 * over a white foot in one downward curve, and the app icon and the wordmark
 * stand on the white.
 *
 * Three things ride together on purpose: the address in readable text so a
 * person can check it without a scanner, the code so a camera can, and the
 * account's identicon — DERIVED from the address, so a card someone doctored
 * to swap the address carries artwork that no longer matches it. The
 * identicon sits beside the address it is checked against.
 *
 * The code is encoded at level H (`shareCardQrMatrix`, the core's): the logo
 * plate covers about 7% of it, and a picture that travels through chat apps
 * is recompressed on the way. The in-app receive code (`QrCard`) stays at
 * level M — only the saved card carries a logo over its modules.
 *
 * Every position is the web's `SHARE_CARD` geometry (`share-image.ts`,
 * `composeShareSvg`), copied by hand: CSS px at 1× are dp here, and the
 * capture draws at density 2, as the web rasterises at 2×. The height is the
 * content's — a two-line headline makes a taller card. Drawn as one canvas
 * (plus the identicon, which is the app's own [IdenticonImage]) so every line
 * of text sits on the baseline the web puts it on, not where a text box would.
 *
 * Not a screen. It ends up in someone's photo library and then in a chat, so
 * every colour is fixed rather than themed: away from the app, the card has
 * to say what it is on its own. Where the space offered is narrower than the
 * card (the gallery on a phone), the whole card scales down like a picture.
 *
 * [logo] is the network's logo, already loaded — the capture waits for it
 * ([ShareCardCapture]); `null` draws the lettered disc.
 */
@Composable
fun ShareCardArtwork(model: ShareCardModel, modifier: Modifier = Modifier, logo: ImageBitmap? = null) {
    val ink = VelaTheme.colors.fixed.shadowInk
    val density = LocalDensity.current
    val measurer = rememberTextMeasurer(cacheSize = 24)
    val context = LocalContext.current
    val emboldenCjk = remember(context) { !CjkBold.honoured(context) }
    val layout = remember(model, density, measurer, ink, emboldenCjk) {
        ShareCardLayout.of(model, CardText(measurer, density, emboldenCjk), ink)
    }
    // The SAME encoder every Vela platform uses, over the bridge, at level H.
    // A blank code is the gallery's: its placeholder pattern, as `QrCard` draws.
    val matrix = remember(model.code) {
        model.code.takeIf { it.isNotBlank() }?.let { text -> runCatching { shareCardQrMatrix(text) }.getOrNull() }
    }
    BoxWithConstraints(modifier = modifier) {
        val fit = if (constraints.hasBoundedWidth) min(1f, maxWidth.value / C.WIDTH) else 1f
        Box(modifier = Modifier.requiredSize((C.WIDTH * fit).dp, (layout.height * fit).dp)) {
            Canvas(modifier = Modifier.matchParentSize()) {
                scale(fit, pivot = Offset.Zero) {
                    drawCard(layout, matrix, logo, model.networkMark.badgeColor, ink, fit)
                }
            }
            IdenticonImage(
                seed = model.identiconSeed,
                size = (C.IDENTICON * fit).dp,
                tappable = false,
                modifier = Modifier.offset((layout.identiconX * fit).dp, (layout.identiconY * fit).dp),
            )
        }
    }
}

/**
 * The card's geometry in dp at 1× — the web's `SHARE_CARD`, number for
 * number. Proportions are the WeChat card's: a sheet about two thirds of the
 * width, the code about 70% of the sheet, the curve dipping about a fifteenth
 * of the width, and a brand line about half the width.
 */
private object C {
    const val WIDTH = 480f
    const val TOP = 52f

    /** Headline: one line at 32 when it fits, shrinking to 26, then two lines. */
    const val HEADLINE_SIZE = 32f
    const val HEADLINE_MIN_SIZE = 26f
    const val HEADLINE_LEADING = 1.25f

    /** Widest a line of text on the orange may run. */
    const val TEXT_WIDTH = 400f
    const val NOTE_GAP = 10f
    const val NOTE_SIZE = 15f
    const val NOTE_LINE = 20f
    const val SHEET_GAP = 28f
    const val SHEET_WIDTH = 320f
    const val SHEET_RADIUS = 20f
    const val SHEET_PAD = 48f
    const val SHEET_PAD_BOTTOM = 40f
    const val QR = 224f

    /** The white plate the network logo sits on, in the code's centre. */
    const val PLATE = 60f
    const val PLATE_RADIUS = 16f
    const val LOGO = 44f
    const val TICKER_SIZE = 14f
    const val IDENTITY_GAP = 26f
    const val IDENTICON = 48f
    const val IDENTITY_TEXT_GAP = 12f
    const val NAME_SIZE = 17f
    const val NAME_LINE = 22f
    const val ADDRESS_SIZE = 12.5f
    const val ADDRESS_LINE = 17f
    const val NAME_ADDRESS_GAP = 3f

    /** Sheet bottom to where the curve leaves the card's edges. */
    const val CURVE_GAP = 52f

    /** How far the curve dips at the centre. */
    const val CURVE_DEPTH = 32f

    /** The curve's lowest point to the card's bottom. */
    const val FOOT = 112f
    const val ICON = 52f
    const val ICON_GAP = 12f
    const val WORDMARK_SIZE = 32f
}

/**
 * The app icon, verbatim from `docs/design/icon/app-icon.svg` — THE mark every
 * platform's icon is rendered from. Drawn here rather than read from the
 * package manager, whose icon an OEM launcher may have masked into a circle or
 * a squircle; and not the in-app sailboat, which is a different drawing. Its
 * plate's orange is the card's field (founder, 2026-08-15: the icon's #F46D50,
 * not the UI accent).
 */
private object AppIconArt {
    const val VIEW_BOX = 68f
    const val PLATE_XY = 1f
    const val PLATE_SIZE = 66f
    const val PLATE_RX = 18f
    val plate = Color(0xFFF46D50)
    val paths: List<Pair<Path, Color>> by lazy {
        listOf(
            "M33,12C25,21,20,32,17,42L33,42L33,12Z" to Color(0xFFFFF3EC),
            "M36,19C45,25,50,34,52,42L36,42L36,19Z" to Color(0xFFFFC6B0),
            "M13,46L55,46C52,52,47,55,40,55L28,55C21,55,16,52,13,46Z" to Color(0xFF5A4037),
        ).map { (d, fill) -> PathParser().parsePathString(d).toPath() to fill }
    }
}

/** Text in the card's faces, measured in dp (the card's units) at the capture's density. */
private class CardText(
    private val measurer: TextMeasurer,
    private val density: Density,
    /** [CjkBold]: this phone's fallback face ignores a bold request, so the card supplies the weight. */
    private val emboldenCjk: Boolean,
) {
    fun layout(text: String, size: Float, weight: FontWeight, color: Color, mono: Boolean = false): TextLayoutResult =
        measurer.measure(
            text = AnnotatedString(text),
            style = TextStyle(
                color = color,
                fontFamily = if (mono) VelaMonoFontFamily else VelaFontFamily,
                fontWeight = weight,
                // In dp, not sp: the card is a picture, and a picture does not
                // follow the phone's font-size setting.
                fontSize = with(density) { size.dp.toSp() },
                platformStyle = PlatformTextStyle(includeFontPadding = false),
            ),
            softWrap = false,
            maxLines = 1,
            density = density,
        )

    fun width(text: String, size: Float, weight: FontWeight, mono: Boolean = false): Float =
        if (text.isEmpty()) 0f else layout(text, size, weight, Color.Unspecified, mono).getLineRight(0) / density.density

    /**
     * The CJK glyphs of a bold [text] once more, as outlines only — drawn
     * over the fill they are Android's own fake bold, for those glyphs alone,
     * and only on a phone whose fallback face would not draw them bold
     * ([CjkBold]). Fake-bolding the whole line would thicken the Latin, which
     * Jakarta already draws bold. `null` when there is nothing to embolden.
     * Same text, same style, so the glyphs land exactly on the fill's.
     */
    fun cjkOutline(text: String, size: Float, weight: FontWeight, color: Color): TextLayoutResult? {
        if (!emboldenCjk || weight < FontWeight.SemiBold || text.none { isCjk(it) }) return null
        val annotated = buildAnnotatedString {
            text.forEach { ch -> withStyle(SpanStyle(color = if (isCjk(ch)) color else Color.Transparent)) { append(ch) } }
        }
        return measurer.measure(
            text = annotated,
            style = TextStyle(
                fontFamily = VelaFontFamily,
                fontWeight = weight,
                fontSize = with(density) { size.dp.toSp() },
                platformStyle = PlatformTextStyle(includeFontPadding = false),
                // Skia's fake-bold width at display sizes: a 32nd of the size.
                drawStyle = Stroke(width = size * density.density / 32f),
            ),
            softWrap = false,
            maxLines = 1,
            density = density,
        )
    }

    /** CJK and beyond (the web's estimator's line); both halves of a surrogate pair land on the same side. */
    private fun isCjk(ch: Char): Boolean = ch.code >= 0x2E80
}

/**
 * Does this phone draw CJK bold when a bold line asks for it?
 *
 * Jakarta has no CJK, so 扫码向我转账 and a Chinese name fall back to the
 * system face, and whether that face answers a bold request is the phone's
 * business. Measured 2026-09-27 as the ink of 国大表 in Jakarta Bold over
 * Jakarta Regular: a Pixel (API 34) 1.40, the Xiaomi (MIUI 13) 1.12 — where
 * asking the system for weight 700 changed nothing and only fake-bold did,
 * while Latin measured 1.60 on both. Below [THRESHOLD] the card emboldens its
 * CJK itself. Measured once per process, with the faces the card resolves to.
 */
internal object CjkBold {
    private const val THRESHOLD = 1.25f

    @Volatile private var measured: Boolean? = null

    fun honoured(context: Context): Boolean = measured ?: measure(context).also { measured = it }

    private fun measure(context: Context): Boolean = runCatching {
        val regular = ResourcesCompat.getFont(context, R.font.plus_jakarta_sans_regular) ?: return true
        val bold = ResourcesCompat.getFont(context, R.font.plus_jakarta_sans_bold) ?: return true
        ink(bold) >= ink(regular) * THRESHOLD
    }.getOrDefault(true)

    /** Pixels more than half covered by 国大表 set at 120 px in [face]. */
    private fun ink(face: Typeface): Int {
        val bitmap = Bitmap.createBitmap(420, 160, Bitmap.Config.ARGB_8888)
        android.graphics.Canvas(bitmap).drawText(
            "国大表", 10f, 120f,
            Paint(Paint.ANTI_ALIAS_FLAG).apply { typeface = face; textSize = 120f },
        )
        val pixels = IntArray(bitmap.width * bitmap.height)
        bitmap.getPixels(pixels, 0, bitmap.width, 0, 0, bitmap.width, bitmap.height)
        bitmap.recycle()
        return pixels.count { (it ushr 24) > 128 }
    }
}

/** One line of text, placed: its left edge and its baseline, in dp; [outline] emboldens its CJK. */
private class Placed(val text: TextLayoutResult, val x: Float, val baseline: Float, val outline: TextLayoutResult? = null)

/**
 * Every position on the card, computed once — `composeShareSvg`'s arithmetic,
 * in the same order and with the same names.
 */
private class ShareCardLayout(
    val height: Float,
    val headline: List<Placed>,
    val note: Placed,
    val sheetTop: Float,
    val sheetBottom: Float,
    val qrTop: Float,
    val centreY: Float,
    val ticker: Placed,
    val identiconX: Float,
    val identiconY: Float,
    val name: Placed,
    val address: List<Placed>,
    val edge: Float,
    val iconX: Float,
    val iconY: Float,
    val wordmark: Placed,
) {
    companion object {
        /** The baseline that centres a line of `size` text on `centre` (the web's `baseline`). */
        private fun baseline(centre: Float, size: Float): Float = centre + size * 0.35f

        fun of(model: ShareCardModel, text: CardText, ink: Color): ShareCardLayout {
            val bold = VelaFontWeight.bold
            val paper = VelaOnAccent
            val cx = C.WIDTH / 2
            fun centred(line: String, size: Float, weight: FontWeight, color: Color, centre: Float): Placed {
                val laid = text.layout(line, size, weight, color)
                return Placed(
                    laid,
                    cx - text.width(line, size, weight) / 2,
                    baseline(centre, size),
                    text.cjkOutline(line, size, weight, color),
                )
            }

            // The orange: headline, then the network it may be paid on.
            val (headlineSize, headlineLines) = fitHeadline(model.headline) { line, size -> text.width(line, size, bold) }
            val headlineLine = headlineSize * C.HEADLINE_LEADING
            val headline = headlineLines.mapIndexed { i, line ->
                centred(line, headlineSize, bold, paper, C.TOP + headlineLine * (i + 0.5f))
            }
            val noteTop = C.TOP + headlineLine * headlineLines.size + C.NOTE_GAP
            val noteWidth = text.width(model.networkNote, C.NOTE_SIZE, VelaFontWeight.medium)
            val noteSize = if (noteWidth <= C.TEXT_WIDTH) {
                C.NOTE_SIZE
            } else {
                max(11f, floor(C.NOTE_SIZE * C.TEXT_WIDTH / noteWidth))
            }
            val note = centred(model.networkNote, noteSize, VelaFontWeight.medium, paper, noteTop + C.NOTE_LINE / 2)

            // The sheet and its code.
            val sheetTop = noteTop + C.NOTE_LINE + C.SHEET_GAP
            val qrTop = sheetTop + C.SHEET_PAD
            val cy = qrTop + C.QR / 2
            val ticker = centred(model.networkMark.ticker, C.TICKER_SIZE, bold, paper, cy)

            // The account: identicon left, name and the whole address beside it.
            val idTop = qrTop + C.QR + C.IDENTITY_GAP
            val textHeight = C.NAME_LINE + C.NAME_ADDRESS_GAP + C.ADDRESS_LINE * 2
            val textRoom = C.QR - C.IDENTICON - C.IDENTITY_TEXT_GAP
            val name = truncate(model.name, textRoom) { text.width(it, C.NAME_SIZE, bold) }
            val lines = listOf(model.lines.first, model.lines.second).filter { it.isNotEmpty() }
            val widest = (listOf(text.width(name, C.NAME_SIZE, bold)) +
                lines.map { text.width(it, C.ADDRESS_SIZE, VelaFontWeight.regular, mono = true) }).max()
            val blockWidth = C.IDENTICON + C.IDENTITY_TEXT_GAP + min(textRoom, widest)
            val idX = cx - blockWidth / 2
            val idY = idTop + (textHeight - C.IDENTICON) / 2
            val textX = idX + C.IDENTICON + C.IDENTITY_TEXT_GAP
            val namePlaced = Placed(
                text.layout(name, C.NAME_SIZE, bold, ink),
                textX,
                baseline(idTop + C.NAME_LINE / 2, C.NAME_SIZE),
                text.cjkOutline(name, C.NAME_SIZE, bold, ink),
            )
            val addressTop = idTop + C.NAME_LINE + C.NAME_ADDRESS_GAP
            val address = lines.mapIndexed { i, line ->
                Placed(
                    text.layout(line, C.ADDRESS_SIZE, VelaFontWeight.regular, ink.copy(alpha = 0.5f), mono = true),
                    textX,
                    baseline(addressTop + C.ADDRESS_LINE * (i + 0.5f), C.ADDRESS_SIZE),
                )
            }
            val sheetBottom = idTop + textHeight + C.SHEET_PAD_BOTTOM

            // The field closes over the foot in one curve that dips at the
            // centre — the WeChat card's direction — and the brand stands on
            // the white.
            val edge = sheetBottom + C.CURVE_GAP
            val lowest = edge + C.CURVE_DEPTH
            val height = lowest + C.FOOT
            val wordmarkWidth = text.width(model.wordmark, C.WORDMARK_SIZE, bold)
            val brandX = cx - (C.ICON + C.ICON_GAP + wordmarkWidth) / 2
            val brandCentre = lowest + C.FOOT / 2
            val wordmark = Placed(
                text.layout(model.wordmark, C.WORDMARK_SIZE, bold, ink),
                brandX + C.ICON + C.ICON_GAP,
                baseline(brandCentre, C.WORDMARK_SIZE),
            )

            return ShareCardLayout(
                height = height,
                headline = headline,
                note = note,
                sheetTop = sheetTop,
                sheetBottom = sheetBottom,
                qrTop = qrTop,
                centreY = cy,
                ticker = ticker,
                identiconX = idX,
                identiconY = idY,
                name = namePlaced,
                address = address,
                edge = edge,
                iconX = brandX,
                iconY = brandCentre - C.ICON / 2,
                wordmark = wordmark,
            )
        }
    }
}

/** The text of `text`, one entry per code point — JavaScript's `Array.from`. */
private fun codePoints(text: String): List<String> = buildList {
    var i = 0
    while (i < text.length) {
        val n = Character.charCount(text.codePointAt(i))
        add(text.substring(i, i + n))
        i += n
    }
}

/**
 * The headline set to the card (the web's `fitHeadline`): one line at the
 * largest size from 32 down to 26 that fits, else two lines split where the
 * halves come out closest in width (at a space when there is one, anywhere
 * in CJK), shrunk until the longer half fits. [measure] is a width in dp.
 */
internal fun fitHeadline(text: String, measure: (String, Float) -> Float): Pair<Float, List<String>> {
    val top = C.HEADLINE_SIZE
    val whole = measure(text, top)
    if (whole <= C.TEXT_WIDTH) return top to listOf(text)
    val shrunk = floor(top * C.TEXT_WIDTH / whole)
    if (shrunk >= C.HEADLINE_MIN_SIZE) return shrunk to listOf(text)

    val chars = codePoints(text)
    val hasSpace = " " in chars
    var best: List<String>? = null
    var bestWidth = Float.POSITIVE_INFINITY
    for (i in 1 until chars.size) {
        if (hasSpace && chars[i] != " ") continue
        val first = chars.subList(0, i).joinToString("").trim()
        val second = chars.subList(i, chars.size).joinToString("").trim()
        if (first.isEmpty() || second.isEmpty()) continue
        val width = max(measure(first, top), measure(second, top))
        if (width < bestWidth) {
            best = listOf(first, second)
            bestWidth = width
        }
    }
    // Nowhere to break (one long word): the one line, as small as it must be.
    if (best == null) return shrunk to listOf(text)
    return min(top, floor(top * C.TEXT_WIDTH / bestWidth)) to best
}

/** `text` cut to `width` with an ellipsis, or whole when it fits (the web's `truncate`). */
internal fun truncate(text: String, width: Float, fits: (String) -> Float): String {
    if (fits(text) <= width) return text
    val chars = codePoints(text).toMutableList()
    while (chars.size > 1 && fits(chars.joinToString("") + "…") > width) chars.removeAt(chars.lastIndex)
    return chars.joinToString("").trimEnd() + "…"
}

/** Draws [placed] with its baseline where the layout put it. */
private fun DrawScope.drawPlaced(placed: Placed) {
    val topLeft = Offset(placed.x.dp.toPx(), placed.baseline.dp.toPx() - placed.text.firstBaseline)
    drawText(placed.text, topLeft = topLeft)
    placed.outline?.let { drawText(it, topLeft = topLeft) }
}

private fun DrawScope.drawCard(
    layout: ShareCardLayout,
    matrix: QrMatrix?,
    logo: ImageBitmap?,
    badge: Color,
    ink: Color,
    /** The scale the card is drawn at, so module edges land on whole DEVICE pixels. */
    fit: Float,
) {
    val paper = VelaOnAccent
    val w = C.WIDTH.dp.toPx()
    val cx = w / 2
    val cy = layout.centreY.dp.toPx()

    // White underneath, then the orange field with its downward curve.
    drawRect(paper, size = Size(w, layout.height.dp.toPx()))
    val edge = layout.edge.dp.toPx()
    val field = Path().apply {
        moveTo(0f, 0f)
        lineTo(w, 0f)
        lineTo(w, edge)
        quadraticTo(cx, edge + (C.CURVE_DEPTH * 2).dp.toPx(), 0f, edge)
        close()
    }
    drawPath(field, AppIconArt.plate)
    layout.headline.forEach { drawPlaced(it) }
    drawPlaced(layout.note)

    // The sheet.
    val sheetLeft = ((C.WIDTH - C.SHEET_WIDTH) / 2).dp.toPx()
    drawRoundRect(
        color = paper,
        topLeft = Offset(sheetLeft, layout.sheetTop.dp.toPx()),
        size = Size(C.SHEET_WIDTH.dp.toPx(), (layout.sheetBottom - layout.sheetTop).dp.toPx()),
        cornerRadius = CornerRadius(C.SHEET_RADIUS.dp.toPx()),
    )

    // The code, module edges snapped to whole pixels so it stays crisp (the
    // web's `shape-rendering="crispEdges"`); a dark run in a row is one rect.
    val cells = matrix?.width?.toInt() ?: QR_MODULES
    val qrLeft = ((C.WIDTH - C.QR) / 2).dp.toPx()
    val qrTop = layout.qrTop.dp.toPx()
    val module = C.QR.dp.toPx() / cells
    fun dark(r: Int, c: Int): Boolean = matrix?.modules?.getOrNull(r * cells + c) ?: qrCell(r, c)
    fun edgeX(c: Int): Float = ((qrLeft + c * module) * fit).roundToInt() / fit
    fun edgeY(r: Int): Float = ((qrTop + r * module) * fit).roundToInt() / fit
    for (r in 0 until cells) {
        var c = 0
        while (c < cells) {
            if (!dark(r, c)) { c++; continue }
            val start = c
            while (c < cells && dark(r, c)) c++
            drawRect(
                color = ink,
                topLeft = Offset(edgeX(start), edgeY(r)),
                size = Size(edgeX(c) - edgeX(start), edgeY(r + 1) - edgeY(r)),
            )
        }
    }

    // The network's logo on its plate, in the code's centre.
    val plate = C.PLATE.dp.toPx()
    drawRoundRect(
        color = paper,
        topLeft = Offset(cx - plate / 2, cy - plate / 2),
        size = Size(plate, plate),
        cornerRadius = CornerRadius(C.PLATE_RADIUS.dp.toPx()),
    )
    val r = (C.LOGO / 2).dp.toPx()
    if (logo == null) {
        drawCircle(color = badge, radius = r, center = Offset(cx, cy))
        drawPlaced(layout.ticker)
    } else {
        // Clipped to a circle and cropped to fill it (`xMidYMid slice`).
        val side = min(logo.width, logo.height)
        val circle = Path().apply { addOval(Rect(Offset(cx, cy), r)) }
        clipPath(circle) {
            drawImage(
                image = logo,
                srcOffset = IntOffset((logo.width - side) / 2, (logo.height - side) / 2),
                srcSize = IntSize(side, side),
                dstOffset = IntOffset((cx - r).roundToInt(), (cy - r).roundToInt()),
                dstSize = IntSize((2 * r).roundToInt(), (2 * r).roundToInt()),
                filterQuality = FilterQuality.High,
            )
        }
        drawCircle(
            color = ink.copy(alpha = 0.08f),
            radius = r - 0.5.dp.toPx(),
            center = Offset(cx, cy),
            style = Stroke(width = 1.dp.toPx()),
        )
    }

    // Name and address (the identicon beside them is a composable of its own).
    drawPlaced(layout.name)
    layout.address.forEach { drawPlaced(it) }

    // The brand line on the white foot: the app icon, then the wordmark.
    val icon = C.ICON.dp.toPx()
    val unit = icon / AppIconArt.VIEW_BOX
    translate(layout.iconX.dp.toPx(), layout.iconY.dp.toPx()) {
        scale(unit, pivot = Offset.Zero) {
            drawRoundRect(
                color = AppIconArt.plate,
                topLeft = Offset(AppIconArt.PLATE_XY, AppIconArt.PLATE_XY),
                size = Size(AppIconArt.PLATE_SIZE, AppIconArt.PLATE_SIZE),
                cornerRadius = CornerRadius(AppIconArt.PLATE_RX),
            )
            AppIconArt.paths.forEach { (path, fill) -> drawPath(path, fill) }
        }
    }
    drawPlaced(layout.wordmark)
}
