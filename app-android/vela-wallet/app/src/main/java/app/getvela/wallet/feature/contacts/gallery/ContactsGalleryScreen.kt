package app.getvela.wallet.feature.contacts.gallery

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.feature.contacts.ContactsActions
import app.getvela.wallet.feature.contacts.ContactsFixtures
import app.getvela.wallet.feature.contacts.ContactsRoute
import app.getvela.wallet.feature.contacts.ContactsScreenState

/**
 * Contacts preview gallery (spec 018 FR-004, research D1): every mobile C-state
 * plus the component board, each reachable in ≤2 interactions, driven by
 * fixtures alone and fully offline. Reached only via the
 * `vela.startDestination` intent extra — production navigation never links
 * here.
 *
 * Chip labels are state codes / component names (data, not translations); the
 * theme chip reuses the onboarding theme labels and the text-scale chip's
 * multiplier is a numeral, so nothing user-visible bypasses the corpus.
 */
private enum class ContactsGalleryEntry(val label: String) {
    C1("C1"),
    C1S("C1s"),
    C1F("C1f"),
    C2("C2"),
    C2S("C2s"),
    C3("C3"),
    C4("C4"),
    C5("C5"),
    C6("C6"),
    C7("C7"),
    C8("C8"),
    C9("C9"),
    Components("Components"),
    ;

    val screenState: ContactsScreenState?
        get() = when (this) {
            C1 -> ContactsScreenState.C1
            C1S -> ContactsScreenState.C1S
            C1F -> ContactsScreenState.C1F
            C2 -> ContactsScreenState.C2
            C2S -> ContactsScreenState.C2S
            C3 -> ContactsScreenState.C3
            C4 -> ContactsScreenState.C4
            C5 -> ContactsScreenState.C5
            C6 -> ContactsScreenState.C6
            C7 -> ContactsScreenState.C7
            C8 -> ContactsScreenState.C8
            C9 -> ContactsScreenState.C9
            Components -> null
        }
}

/** The gallery's 1.35× accessibility-text probe (mirrors wallet H7x). */
private const val TEXT_SCALE_LABEL = "1.35×"
private const val TEXT_SCALE = 1.35f

@Composable
fun ContactsGalleryScreen(systemDarkTheme: Boolean) {
    var dark by rememberSaveable { mutableStateOf(systemDarkTheme) }
    var scaled by rememberSaveable { mutableStateOf(false) }
    var entry by rememberSaveable { mutableStateOf(ContactsGalleryEntry.C1) }

    VelaTheme(darkTheme = dark) {
        val colors = VelaTheme.colors
        val strings = LocalVelaStrings.current
        Column(
            modifier = Modifier
                .fillMaxSize()
                .background(colors.bgBase)
                .statusBarsPadding(),
        ) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .horizontalScroll(rememberScrollState())
                    .padding(horizontal = VelaSpacing.xl, vertical = VelaSpacing.md),
                horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                ContactsGalleryEntry.entries.forEach { candidate ->
                    GalleryChip(
                        label = candidate.label,
                        selected = candidate == entry,
                        onClick = { entry = candidate },
                    )
                }
                GalleryChip(
                    label = strings.t(
                        if (dark) I18nKeys.Settings.THEME_LIGHT else I18nKeys.Settings.THEME_DARK,
                    ),
                    selected = false,
                    onClick = { dark = !dark },
                )
                GalleryChip(
                    label = TEXT_SCALE_LABEL,
                    selected = scaled,
                    onClick = { scaled = !scaled },
                )
            }
            Box(modifier = Modifier.weight(1f)) {
                when (entry) {
                    ContactsGalleryEntry.Components -> ContactsComponentBoard()
                    else -> {
                        val state = entry.screenState ?: ContactsScreenState.C1
                        val model = remember(strings, state, scaled) {
                            ContactsFixtures.buildMobileState(state, strings)
                                .copy(textScale = if (scaled) TEXT_SCALE else 1f)
                        }
                        ContactsRoute(
                            model = model,
                            actions = ContactsActions(
                                // Sinks: the gallery only swaps fixture states.
                                onContact = { entry = ContactsGalleryEntry.C2 },
                                onGroup = { entry = ContactsGalleryEntry.C4 },
                                onDismissMenu = {
                                    entry = when (entry) {
                                        ContactsGalleryEntry.C5 -> ContactsGalleryEntry.C1
                                        ContactsGalleryEntry.C6 -> ContactsGalleryEntry.C4
                                        ContactsGalleryEntry.C2S -> ContactsGalleryEntry.C2
                                        else -> entry
                                    }
                                },
                                onAction = { id ->
                                    entry = when (id) {
                                        "contacts.addContact" -> ContactsGalleryEntry.C5
                                        "contacts.groupMenu" -> ContactsGalleryEntry.C6
                                        "contacts.deleteContact",
                                        "contacts.swipeDelete",
                                        -> ContactsGalleryEntry.C2S
                                        // Confirming is visual only — the
                                        // gallery returns to the fixture state.
                                        "contacts.delete" -> ContactsGalleryEntry.C2
                                        "contacts.back" -> ContactsGalleryEntry.C1
                                        else -> entry
                                    }
                                },
                            ),
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun GalleryChip(label: String, selected: Boolean, onClick: () -> Unit) {
    val colors = VelaTheme.colors
    Box(
        modifier = Modifier
            .clip(RoundedCornerShape(VelaRadius.full))
            .background(if (selected) colors.accentSoft else colors.bgRaised)
            .clickable(onClick = onClick)
            .padding(horizontal = VelaSpacing.lg, vertical = VelaSpacing.md),
    ) {
        Text(
            text = label,
            color = if (selected) colors.accentBase else colors.fgBase,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.medium,
            fontSize = VelaTextSize.base,
            maxLines = 1,
        )
    }
}
