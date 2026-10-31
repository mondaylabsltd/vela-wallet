package app.getvela.wallet.core.diagnostics

import app.getvela.wallet.core.designsystem.components.VelaModalSheet
import app.getvela.wallet.core.designsystem.components.sheetScrolls
import android.content.Intent
import android.net.Uri
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextAlign
import app.getvela.wallet.core.designsystem.components.VelaPrimaryButton
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.VelaStrings

/**
 * Spec 047 D10 (038's panic sheet): a crash the last run wrote — or a core
 * fault — raised once on the next launch as the ordinary failure sheet:
 * what went wrong, the first lines, Report (the prefilled issue) and Close.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CrashSheet(strings: VelaStrings, version: String) {
    val context = LocalContext.current
    var report by remember { mutableStateOf(CrashReport.read(context)) }
    val record = report ?: return
    val colors = VelaTheme.colors
    androidx.compose.runtime.LaunchedEffect(record) { VelaLog.event("crash.sheet", "raised", "message" to record.message.take(60)) }
    val dismiss = {
        CrashReport.clear(context)
        report = null
    }
    VelaModalSheet(onDismissRequest = dismiss, containerColor = colors.bgRaised) {
        Column(modifier = Modifier.fillMaxWidth().sheetScrolls().padding(horizontal = VelaSpacing.xl).navigationBarsPadding()) {
            Text(
                text = strings.t(I18nKeys.Flow.UNKNOWN_TITLE),
                color = colors.fgBase, fontFamily = VelaFontFamily, fontWeight = VelaFontWeight.bold, fontSize = VelaTextSize.xl,
            )
            Spacer(modifier = Modifier.height(VelaSpacing.md))
            // r8-3: calm words, never a raw Java stack trace. The exception and
            // its first lines go into the report the person chooses to send
            // (Report, below) — the screen itself says what happened in plain
            // language and that the way back is one tap.
            Text(text = crashBody(strings), color = colors.fgMuted, fontFamily = VelaFontFamily, fontSize = VelaTextSize.base)
            Spacer(modifier = Modifier.height(VelaSpacing.xl))
            VelaPrimaryButton(
                strings.t(I18nKeys.Flow.REPORT_ERROR),
                onClick = {
                    VelaLog.event("crash.sheet", "report tapped")
                    // Spec 081 FR-016: by FIELD ID, never `body`. With
                    // `template=bug.yml` GitHub drops `body` silently, so this
                    // button used to open an empty form with the stack trace
                    // thrown away — the one moment a report is worth most.
                    val url = BugReportUrl.build(
                        what = record.message,
                        steps = record.stack.lines().take(12).joinToString("\n"),
                        environment = "App version: v$version\nPlatform: Android ${android.os.Build.VERSION.RELEASE}\nThread: ${record.thread}",
                        titlePrefix = "[android crash] ",
                    )
                    runCatching { context.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url))) }
                    dismiss()
                },
                modifier = Modifier.fillMaxWidth(),
            )
            Spacer(modifier = Modifier.height(VelaSpacing.sm))
            Text(
                text = strings.t(I18nKeys.Flow.CLOSE),
                color = colors.fgMuted, fontFamily = VelaFontFamily, fontSize = VelaTextSize.base, textAlign = TextAlign.Center,
                modifier = Modifier.fillMaxWidth().clickable(onClick = dismiss).padding(vertical = VelaSpacing.md),
            )
            Spacer(modifier = Modifier.height(VelaSpacing.lg))
        }
    }
}

/**
 * What the crash sheet says (r8-3): the plain "something went wrong inside
 * Vela" line — no exception class, no message, no stack. Pure, so a JVM test
 * holds the screen to it.
 */
internal fun crashBody(strings: VelaStrings): String = strings.t(I18nKeys.Flows.FEE_REASON_INTERNAL)
