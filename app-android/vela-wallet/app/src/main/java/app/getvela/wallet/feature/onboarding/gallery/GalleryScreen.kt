package app.getvela.wallet.feature.onboarding.gallery

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaBorder
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaLetterSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.feature.onboarding.flow.DoneScreen
import app.getvela.wallet.feature.onboarding.flow.Fixture
import app.getvela.wallet.feature.onboarding.flow.FlowFixtures
import app.getvela.wallet.feature.onboarding.flow.FlowSheet
import app.getvela.wallet.feature.onboarding.flow.FlowShell
import app.getvela.wallet.feature.onboarding.flow.KeysScreen
import app.getvela.wallet.feature.onboarding.flow.NameScreen
import app.getvela.wallet.feature.onboarding.flow.ProgressScreen
import app.getvela.wallet.feature.onboarding.flow.RetryScreen
import app.getvela.wallet.feature.onboarding.flow.Screen
import app.getvela.wallet.feature.onboarding.flow.StateFixture
import app.getvela.wallet.feature.onboarding.flow.InsertKeySheet
import app.getvela.wallet.feature.onboarding.flow.UsbPinDialog
import app.getvela.wallet.feature.onboarding.flow.progressFor
import app.getvela.wallet.feature.onboarding.flow.screenFor
import app.getvela.wallet.feature.onboarding.flow.statusKeyToI18n
import app.getvela.wallet.feature.onboarding.flow.submitLabelToI18n

/**
 * Dev-only state gallery, rewritten to the v2 state set (spec 019 T115).
 *
 * The important property is not the list but the RENDERER: every entry goes
 * through [screenFor] and the same five screens production uses, so a fixture
 * cannot look right here and wrong in the app. The 014 gallery drove a separate
 * presentation type, which meant it could — and the whole reason those types are
 * gone.
 *
 * Reachable only via MainActivity's `--ez vela.gallery true` intent extra;
 * release builds never receive the extra.
 */
@Composable
fun GalleryScreen(initialDarkTheme: Boolean) {
    var darkTheme by rememberSaveable { mutableStateOf(initialDarkTheme) }
    var selectedCode by rememberSaveable { mutableStateOf<String?>(null) }

    VelaTheme(darkTheme = darkTheme) {
        val strings = LocalVelaStrings.current
        val colors = VelaTheme.colors
        val selected = selectedCode?.let(FlowFixtures::byCode)

        // Device-verified 2026-08-25: without this, system back left the APP
        // from an open fixture rather than returning to the list. The flow
        // screens' own back control is an in-screen affordance, and on Android
        // the gesture is what people actually reach for.
        BackHandler(enabled = selected != null) { selectedCode = null }

        LazyColumn(
            modifier = Modifier
                .fillMaxSize()
                .background(colors.bgBase)
                .safeDrawingPadding(),
        ) {
            item(key = "themeToggle") {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .heightIn(min = VelaSizing.hitTarget)
                        .padding(horizontal = VelaSizing.screenPaddingX),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(
                        text = strings.t(I18nKeys.Settings.THEME_DARK),
                        color = colors.fgBase,
                        fontFamily = VelaFontFamily,
                        fontWeight = VelaFontWeight.medium,
                        fontSize = VelaTextSize.lg,
                        modifier = Modifier.weight(1f),
                    )
                    Switch(checked = darkTheme, onCheckedChange = { darkTheme = it })
                }
                HorizontalDivider(color = colors.borderBase, thickness = VelaBorder.hairline)
            }

            FlowFixtures.all.groupBy { it.group }.forEach { (group, fixtures) ->
                item(key = "header-$group") { GalleryGroupHeader(group) }
                items(fixtures, key = { "$group-${it.code}" }) { fixture ->
                    GalleryFixtureRow(fixture) { selectedCode = fixture.code }
                }
            }
        }

        when (val fixture = selected?.fixture) {
            null -> Unit

            is Fixture.Sheet -> FlowSheet(
                kind = fixture.kind,
                confirmable = fixture.confirmable,
                onAnswer = { selectedCode = null },
            )

            is Fixture.UsbPin -> UsbPinDialog(
                product = "YubiKey 5C NFC",
                retries = fixture.retries,
                isRetry = fixture.isRetry,
                onSubmit = { selectedCode = null },
            )

            is Fixture.InsertKey -> InsertKeySheet(
                otgLooksOff = fixture.otgLooksOff,
                onCancel = { selectedCode = null },
            )

            // A flow step covers the list entirely, as it does in production —
            // it IS a full screen there, and showing it in a card would be a
            // picture of a layout the app never draws.
            is Fixture.Flow -> Box(modifier = Modifier.fillMaxSize().background(colors.bgBase)) {
                val view = fixture.view
                val screen = screenFor(view)
                val statusText = view.status
                    ?.takeIf { progressFor(it) == null }
                    ?.let { strings.t(statusKeyToI18n(it)) }

                FlowShell(
                    backLabel = strings.t(I18nKeys.Flow.BACK),
                    canGoBack = true,
                    // The only interactive control in the gallery: everything
                    // else is a no-op, because a fixture has no core behind it
                    // and a button that appeared to work would be lying.
                    onBack = { selectedCode = null },
                ) {
                    when (screen) {
                        Screen.Loading -> Unit
                        Screen.Name -> NameScreen(
                            name = view.name,
                            nameEditable = view.nameEditable,
                            nameTooLong = view.nameTooLong,
                            acks = view.acks,
                            canSubmit = view.canSubmit,
                            busy = view.busy,
                            submitLabel = strings.t(submitLabelToI18n(view.submitLabel)),
                            statusText = statusText,
                            showStartOver = view.showStartOver,
                            onName = {},
                            onToggleAck = {},
                            onSubmit = {},
                            onStartOver = {},
                            onOpenPrivacy = {},
                            onOpenTerms = {},
                        )
                        Screen.Keys -> KeysScreen(
                            keys = view.keys,
                            canAddKey = view.canAddKey,
                            canFinish = view.canFinish,
                            needsSecondKey = view.needsSecondKey,
                            busy = view.busy,
                            onAddKey = {},
                            onConfirmKey = {},
                            onRemoveKey = {},
                            onFinish = {},
                        )
                        Screen.Progress -> ProgressScreen(
                            position = progressFor(view.status)!!,
                            keyCount = view.keys.size,
                        )
                        Screen.Retry -> RetryScreen(
                            detail = view.syncErrorDetail,
                            busy = view.busy,
                            onRetry = {},
                            onStartOver = {},
                            onEditEndpoint = {},
                        )
                        Screen.Done -> DoneScreen(
                            address = view.address.orEmpty(),
                            walletName = view.keys.firstOrNull()?.name ?: view.name,
                            keys = view.keys,
                            entering = view.busy,
                            onEnter = { selectedCode = null },
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun GalleryGroupHeader(label: String) {
    val colors = VelaTheme.colors
    Text(
        text = label,
        color = colors.fgSubtle,
        fontFamily = VelaFontFamily,
        fontWeight = VelaFontWeight.semibold,
        fontSize = VelaTextSize.sm,
        letterSpacing = VelaLetterSpacing.sectionLabel,
        modifier = Modifier
            .fillMaxWidth()
            .padding(
                start = VelaSizing.screenPaddingX,
                end = VelaSizing.screenPaddingX,
                top = VelaSpacing.xl3,
                bottom = VelaSpacing.md,
            ),
    )
}

@Composable
private fun GalleryFixtureRow(fixture: StateFixture, onClick: () -> Unit) {
    val colors = VelaTheme.colors
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = VelaSizing.hitTarget)
            .clickable(onClick = onClick)
            .padding(horizontal = VelaSizing.screenPaddingX),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        // The fixture's own name is data, not UI copy — it never translates.
        Text(
            text = fixture.code,
            color = colors.fgBase,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.medium,
            fontSize = VelaTextSize.lg,
        )
    }
}
