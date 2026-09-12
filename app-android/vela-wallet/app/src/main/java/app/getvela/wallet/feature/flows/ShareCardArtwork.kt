package app.getvela.wallet.feature.flows

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
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
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.graphics.drawable.toBitmap
import app.getvela.wallet.core.designsystem.components.VelaLogo
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaBorder
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaMonoFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaOnAccent
import app.getvela.wallet.core.identicon.IdenticonImage
import app.getvela.wallet.core.marks.RemoteLogo
import app.getvela.wallet.feature.flows.components.QrCard

/**
 * R4 — what "Save image" produces (spec 021), redrawn in spec 048 after the
 * web's `share-image.ts` so the two cards are one card: the accent field, the
 * headline, a white sheet holding the code (a REAL code — the founder found
 * the placeholder pattern in a saved image), the identicon in its centre, the
 * name, the address in two mono lines, the network pill wearing the chain's
 * own logo, and the white foot with its curved edge carrying the app icon and
 * the wordmark in ink. 480 × 700 at whatever density the capture provides.
 *
 * Not a screen. It ends up in someone's photo library and then in a chat, so
 * every colour is fixed rather than themed: away from the app, the card has
 * to say what it is on its own.
 */
@Composable
fun ShareCardArtwork(model: ShareCardModel, modifier: Modifier = Modifier) {
    val colors = VelaTheme.colors
    val ink = colors.fixed.shadowInk
    val context = LocalContext.current
    val appIcon = remember(context) {
        runCatching { context.packageManager.getApplicationIcon(context.packageName).toBitmap(176, 176).asImageBitmap() }.getOrNull()
    }
    Box(
        modifier = modifier
            .width(CARD_W)
            .height(CARD_H)
            .background(colors.accentBase)
            .drawBehind {
                // The foot: white, its top edge one curve across the card — the
                // reference's hill, not a straight rule (the web's `foot` path).
                val edge = FOOT_EDGE.toPx()
                val apex = FOOT_APEX.toPx()
                val path = Path().apply {
                    moveTo(0f, size.height)
                    lineTo(0f, edge)
                    quadraticBezierTo(size.width / 2f, apex * 2 - edge, size.width, edge)
                    lineTo(size.width, size.height)
                    close()
                }
                drawPath(path, VelaOnAccent)
            },
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = PAD),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Spacer(modifier = Modifier.height(32.dp))
            Text(
                text = model.headline,
                color = VelaOnAccent,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.bold,
                fontSize = 26.sp,
                textAlign = TextAlign.Center,
            )
            Spacer(modifier = Modifier.height(20.dp))
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .background(VelaOnAccent, RoundedCornerShape(20.dp))
                    .padding(PAD),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                QrCard(label = model.headline, payload = model.code.ifBlank { null }) {
                    IdenticonImage(tappable = false, seed = model.identiconSeed, size = 40.dp)
                }
                Spacer(modifier = Modifier.height(16.dp))
                Text(
                    text = model.name,
                    color = ink,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.bold,
                    fontSize = 15.sp,
                )
                Spacer(modifier = Modifier.height(6.dp))
                listOf(model.lines.first, model.lines.second)
                    .filter { it.isNotEmpty() }
                    .forEach { line ->
                        Text(
                            text = line,
                            color = ink.copy(alpha = 0.4f),
                            fontFamily = VelaMonoFontFamily,
                            fontSize = 11.sp,
                        )
                    }
                Spacer(modifier = Modifier.height(16.dp))
                Row(
                    modifier = Modifier
                        .border(VelaBorder.hairline, colors.borderBase, CircleShape)
                        .padding(start = 4.dp, end = 12.dp, top = 4.dp, bottom = 4.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    // The chain's own logo when the chain-data endpoint has one;
                    // the lettered disc is the fallback, as on the web.
                    RemoteLogo(urls = listOfNotNull(model.chainLogoUrl), size = MARK) {
                        Box(
                            modifier = Modifier
                                .size(MARK)
                                .background(model.networkMark.badgeColor, CircleShape),
                            contentAlignment = Alignment.Center,
                        ) {
                            Text(
                                text = model.networkMark.ticker,
                                color = VelaOnAccent,
                                fontFamily = VelaFontFamily,
                                fontWeight = VelaFontWeight.bold,
                                fontSize = 10.sp,
                                maxLines = 1,
                            )
                        }
                    }
                    Spacer(modifier = Modifier.width(6.dp))
                    Text(
                        text = model.networkNote,
                        color = ink,
                        fontFamily = VelaFontFamily,
                        fontSize = 11.sp,
                    )
                }
            }
        }
        Row(
            modifier = Modifier
                .align(Alignment.BottomCenter)
                .height(CARD_H - FOOT_EDGE),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            if (appIcon != null) {
                Image(
                    bitmap = appIcon,
                    contentDescription = null,
                    modifier = Modifier.size(ICON).clip(RoundedCornerShape(10.dp)),
                )
            } else {
                VelaLogo(darkTheme = false, modifier = Modifier.size(ICON))
            }
            Spacer(modifier = Modifier.width(12.dp))
            Text(
                text = model.wordmark,
                color = ink,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.bold,
                fontSize = 26.sp,
            )
        }
    }
}

/** The drawn card's geometry — the web's `CARD_W/H`, `PAD`, `MARK`, `ICON`, `FOOT_EDGE/APEX`. */
private val CARD_W = 480.dp
private val CARD_H = 700.dp
private val PAD = 20.dp
private val MARK = 26.dp
private val ICON = 44.dp
private val FOOT_EDGE = 630.dp
private val FOOT_APEX = 600.dp
