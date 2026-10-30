package app.getvela.wallet.core.widget

import android.content.Context
import androidx.compose.runtime.Composable
import androidx.glance.GlanceId
import androidx.glance.GlanceModifier
import androidx.glance.action.clickable
import androidx.glance.appwidget.GlanceAppWidget
import androidx.glance.appwidget.GlanceAppWidgetManager
import androidx.glance.appwidget.GlanceAppWidgetReceiver
import androidx.glance.appwidget.SizeMode
import androidx.glance.action.actionStartActivity
import androidx.glance.appwidget.appWidgetBackground
import androidx.glance.appwidget.cornerRadius
import androidx.glance.appwidget.provideContent
import androidx.glance.appwidget.updateAll
import androidx.glance.background
import androidx.glance.color.ColorProvider
import androidx.glance.layout.Alignment
import androidx.glance.layout.Column
import androidx.glance.layout.Row
import androidx.glance.layout.Spacer
import androidx.glance.layout.fillMaxSize
import androidx.glance.layout.height
import androidx.glance.layout.padding
import androidx.glance.semantics.contentDescription
import androidx.glance.semantics.semantics
import androidx.glance.text.FontWeight
import androidx.glance.text.Text
// Named apart from Compose's: a widget draws in the launcher's system face,
// never the app's UI face whose contextual alternates turn "0x" into "0×"
// (LiteralFaceTest's rule is about that face, and Glance has no feature setting).
import androidx.glance.text.TextStyle as GlanceTextStyle
import app.getvela.wallet.MainActivity
import app.getvela.wallet.VelaWalletApplication
import app.getvela.wallet.core.designsystem.tokens.VelaColorsDark
import app.getvela.wallet.core.designsystem.tokens.VelaColorsLight
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.diagnostics.VelaLog
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.withTimeoutOrNull

/**
 * The home-screen balance widget (native parity Phase 3): the home's total as
 * last seen, its age, and whose it is — quiet and typographic, like the hero
 * it repeats (docs/DESIGN-LANGUAGE.md: open, no card inside the card, the
 * cents subordinate to the whole). A tap opens the wallet.
 *
 * NO network and no balance read here: [BalanceWidgetSnapshot] is what the
 * wallet route last drew, with the core's own time of that read. Redrawn when
 * the snapshot changes and every 30 min by the system (so "Updated 2h" ages).
 */
class VelaBalanceWidget : GlanceAppWidget() {
    override val sizeMode: SizeMode = SizeMode.Single

    override suspend fun provideGlance(context: Context, id: GlanceId) {
        val container = (context.applicationContext as VelaWalletApplication).container
        val runtime = container.i18nRuntime
        // The words are the corpus's: wait for the catalog (it loads at start).
        withTimeoutOrNull(I18N_WAIT_MS) { runtime.state.first { it.ready } }
        val model = if (runtime.state.value.ready) {
            BalanceWidgetModel.of(BalanceWidgetSnapshot.read(context), runtime, System.currentTimeMillis())
        } else {
            null
        }
        provideContent { Content(model) }
    }

    @Composable
    private fun Content(model: BalanceWidgetModel?) {
        val fg = ColorProvider(day = VelaColorsLight.fgBase, night = VelaColorsDark.fgBase)
        val muted = ColorProvider(day = VelaColorsLight.fgMuted, night = VelaColorsDark.fgMuted)
        val subtle = ColorProvider(day = VelaColorsLight.fgSubtle, night = VelaColorsDark.fgSubtle)
        val bg = ColorProvider(day = VelaColorsLight.bgBase, night = VelaColorsDark.bgBase)
        Column(
            modifier = GlanceModifier
                .fillMaxSize()
                .appWidgetBackground()
                .cornerRadius(VelaRadius.xl2)
                .background(bg)
                .padding(horizontal = VelaSpacing.xl2, vertical = VelaSpacing.xl)
                .clickable(actionStartActivity<MainActivity>()),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = model?.label.orEmpty(),
                style = GlanceTextStyle(color = subtle, fontSize = VelaTextSize.sm, fontWeight = FontWeight.Medium),
                maxLines = 1,
            )
            Spacer(GlanceModifier.height(VelaSpacing.sm))
            Row(
                verticalAlignment = Alignment.Bottom,
                modifier = GlanceModifier.semantics {
                    contentDescription = listOfNotNull(model?.label, model?.integer?.plus(model.decimals.orEmpty())).joinToString(" ")
                },
            ) {
                Text(
                    text = model?.integer ?: "—",
                    style = GlanceTextStyle(color = fg, fontSize = VelaTextSize.xl3, fontWeight = FontWeight.Medium),
                    maxLines = 1,
                )
                model?.decimals?.let {
                    Text(
                        text = it,
                        style = GlanceTextStyle(color = muted, fontSize = VelaTextSize.xl, fontWeight = FontWeight.Medium),
                        maxLines = 1,
                        modifier = GlanceModifier.padding(bottom = VelaSpacing.xs),
                    )
                }
            }
            model?.footnote?.let {
                Spacer(GlanceModifier.height(VelaSpacing.sm))
                Text(text = it, style = GlanceTextStyle(color = muted, fontSize = VelaTextSize.sm), maxLines = 1)
            }
        }
    }

    companion object {
        private const val I18N_WAIT_MS = 5_000L

        /** Redraw every placed widget (after the snapshot changed, or the language did). */
        suspend fun refresh(context: Context) {
            val placed = runCatching { GlanceAppWidgetManager(context).getGlanceIds(VelaBalanceWidget::class.java).size }.getOrDefault(0)
            if (placed == 0) return
            runCatching { VelaBalanceWidget().updateAll(context) }
                .onFailure { VelaLog.failure("widget", "redraw failed", it) }
            VelaLog.event("widget", "redrawn", "placed" to placed)
        }
    }
}

class VelaBalanceWidgetReceiver : GlanceAppWidgetReceiver() {
    override val glanceAppWidget: GlanceAppWidget = VelaBalanceWidget()
}
