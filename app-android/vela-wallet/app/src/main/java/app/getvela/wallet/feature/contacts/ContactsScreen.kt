package app.getvela.wallet.feature.contacts

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Density
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.feature.contacts.components.ActionMenuSheet
import app.getvela.wallet.feature.contacts.components.ContactFormSheet
import app.getvela.wallet.feature.contacts.components.ContactNoticeSheet
import app.getvela.wallet.feature.contacts.components.AlphaIndexRail
import app.getvela.wallet.feature.contacts.components.AlphaSectionHeader
import app.getvela.wallet.feature.contacts.components.ContactRow
import app.getvela.wallet.feature.contacts.components.ContactsSearchField
import app.getvela.wallet.feature.contacts.components.ContactsTitleHeader
import app.getvela.wallet.feature.contacts.components.EmptyStateCta
import app.getvela.wallet.feature.contacts.components.GroupRow
import app.getvela.wallet.feature.contacts.components.Hairline
import app.getvela.wallet.feature.wallet.components.SectionHeader
import app.getvela.wallet.feature.wallet.components.VelaTab
import app.getvela.wallet.feature.wallet.components.VelaTabBar
import kotlinx.coroutines.launch

/**
 * Action sinks for the contacts screens: every tap reports an id and nothing
 * navigates (spec Assumptions — no destinations exist yet). The gallery passes
 * its own handlers to switch fixture states.
 */
@androidx.compose.runtime.Immutable
data class ContactsActions(
    val onAction: (String) -> Unit = {},
    val onContact: (ContactModel) -> Unit = {},
    val onGroup: (GroupRowModel) -> Unit = {},
    val onDismissMenu: () -> Unit = {},
    val onTab: (VelaTab) -> Unit = {},
    /**
     * The search box, keystroke by keystroke.
     *
     * `null` leaves the box display-only, which is what the gallery wants: a
     * fixture state pins its own query and must not be editable out of it.
     */
    val onQueryChange: ((String) -> Unit)? = null,
    /** Spec 045 US5: the form's two fields (typed text, not an action id). */
    val onFormName: (String) -> Unit = {},
    val onFormAddress: (String) -> Unit = {},
)

/**
 * Mobile contacts entry point (spec 018 FR-002): renders any C-state from a
 * fixture model alone. Detail states delegate to their own screens so each
 * mock has exactly one assembly. The 1.35× text-scale state applies through
 * LocalDensity, the same escape WalletScreen uses for H7x (FR-011).
 */
@Composable
fun ContactsRoute(
    model: ContactsHomeModel,
    modifier: Modifier = Modifier,
    actions: ContactsActions = ContactsActions(),
) {
    if (model.textScale != 1f) {
        val density = LocalDensity.current
        CompositionLocalProvider(
            LocalDensity provides Density(density.density, density.fontScale * model.textScale),
        ) {
            ContactsRouteContent(model, modifier, actions)
        }
    } else {
        ContactsRouteContent(model, modifier, actions)
    }
}

@Composable
private fun ContactsRouteContent(
    model: ContactsHomeModel,
    modifier: Modifier,
    actions: ContactsActions,
) {
    when {
        model.detail != null -> ContactDetailScreen(model, modifier, actions)
        model.groupDetail != null -> GroupDetailScreen(model, modifier, actions)
        else -> ContactsScreen(model, modifier, actions)
    }
    // Spec 045: the form and the import notice sit over whichever page is showing.
    model.notice?.let { notice ->
        ContactNoticeSheet(model = notice, onDismiss = { actions.onAction("contacts.notice.close") })
    }
    model.form?.let { form ->
        ContactFormSheet(
            model = form,
            onDismiss = { actions.onAction("contacts.form.cancel") },
            onName = actions.onFormName,
            onAddress = actions.onFormAddress,
            onSave = { actions.onAction("contacts.form.save") },
        )
    }
}

/**
 * Contacts home (mocks C1 / C1s / C1f / C3 / C5): title + person-add, search
 * field, 分组 section, the A–Z sectioned list with its index rail, and the
 * spec-015 tab bar with 通讯录 selected.
 */
@Composable
fun ContactsScreen(
    model: ContactsHomeModel,
    modifier: Modifier = Modifier,
    actions: ContactsActions = ContactsActions(),
) {
    val colors = VelaTheme.colors
    val listState = rememberLazyListState()
    val scope = rememberCoroutineScope()

    // Row index of each letter's section header inside the LazyColumn — built
    // while emitting so the index rail can jump straight to it, and so a letter
    // with no section resolves to the nearest existing one (data-model rule).
    val letterAnchors = LinkedHashMap<String, Int>()

    Box(
        modifier = modifier
            .fillMaxSize()
            .background(colors.bgBase),
    ) {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .statusBarsPadding(),
        ) {
            Box(modifier = Modifier.weight(1f)) {
                var rowIndex = 0
                LazyColumn(
                    state = listState,
                    modifier = Modifier
                        .fillMaxSize()
                        .padding(horizontal = VelaSizing.screenPaddingX),
                ) {
                    item {
                        Spacer(modifier = Modifier.height(VelaSpacing.xl))
                        ContactsTitleHeader(
                            title = model.title,
                            addContentDescription = model.addContactLabel,
                            onAdd = { actions.onAction("contacts.addContact") },
                        )
                        Spacer(modifier = Modifier.height(VelaSpacing.xl))
                        ContactsSearchField(
                            model = model.search,
                            onClick = { actions.onAction("contacts.search") },
                            onClear = { actions.onAction("contacts.searchClear") },
                            onQueryChange = actions.onQueryChange,
                        )
                        Spacer(modifier = Modifier.height(VelaSpacing.xl3))
                    }
                    rowIndex = 1

                    if (model.empty != null) {
                        item {
                            Spacer(modifier = Modifier.height(VelaSpacing.xl5))
                            EmptyStateCta(
                                model = model.empty,
                                onPrimary = { actions.onAction("contacts.addContact") },
                                onSecondary = { actions.onAction("contacts.importFile") },
                            )
                        }
                        rowIndex += 1
                    }

                    // The header shows whenever there is somebody to GROUP, not
                    // only when a group already exists: drawing it on
                    // `groups.isNotEmpty()` put the door to the first group
                    // behind the first group, so a person with contacts and no
                    // groups could not make one (the founder, 2026-09-15; iOS
                    // had the identical condition and 058 fixed both). An empty
                    // book still draws its own invitation instead — grouping
                    // nobody is not a thing to offer.
                    if (model.empty == null) {
                        item {
                            SectionHeader(
                                title = model.groupsSectionTitle,
                                action = model.groupsAction,
                                onAction = { actions.onAction("contacts.manage") },
                            )
                            Spacer(modifier = Modifier.height(VelaSpacing.md))
                        }
                        rowIndex += 1
                        model.groups.forEach { group ->
                            item {
                                Hairline()
                                GroupRow(model = group, onClick = { actions.onGroup(group) })
                            }
                            rowIndex += 1
                        }
                        item { Spacer(modifier = Modifier.height(VelaSpacing.xl3)) }
                        rowIndex += 1
                    }

                    if (model.sections.isNotEmpty()) {
                        item {
                            SectionHeader(
                                title = model.contactsSectionTitle,
                                action = model.totalLabel,
                                showChevron = false,
                            )
                        }
                        rowIndex += 1

                        var flat = 0
                        model.sections.forEach { section ->
                            letterAnchors[section.letter] = rowIndex
                            item {
                                AlphaSectionHeader(letter = section.letter)
                                Hairline()
                            }
                            rowIndex += 1
                            section.contacts.forEach { contact ->
                                val flatIndex = flat++
                                item {
                                    ContactRow(
                                        contact = contact,
                                        revealed = model.revealedIndex == flatIndex,
                                        swipeSendLabel = model.swipeSendLabel,
                                        swipeDeleteLabel = model.swipeDeleteLabel,
                                        onClick = { actions.onContact(contact) },
                                        onSwipeSend = { actions.onAction("contacts.swipeSend:" + contact.addressFull) },
                                        // Swipe-delete always raises the
                                        // destructive confirmation (FR-008).
                                        onSwipeDelete = { actions.onAction("contacts.swipeDelete:" + contact.addressFull) },
                                    )
                                    Hairline()
                                }
                                rowIndex += 1
                            }
                        }
                    }

                    item { Spacer(modifier = Modifier.height(VelaSpacing.xl3)) }
                }

                if (model.sections.isNotEmpty()) {
                    AlphaIndexRail(
                        letters = model.indexLetters,
                        modifier = Modifier
                            .align(Alignment.CenterEnd)
                            .padding(vertical = VelaSpacing.xl4),
                        onLetter = { letter ->
                            val target = nearestAnchor(letterAnchors, model.indexLetters, letter)
                            if (target != null) {
                                // Direct positioning, never a smooth scroll (SPEC sheet).
                                scope.launch { listState.scrollToItem(target) }
                            }
                        },
                    )
                }
            }
            VelaTabBar(
                tabs = model.tabs,
                selected = VelaTab.Contacts,
                onSelect = actions.onTab,
                modifier = Modifier
                    .fillMaxWidth()
                    .navigationBarsPadding(),
            )
        }
    }

    model.menu?.let { menu ->
        ActionMenuSheet(
            model = menu,
            onDismiss = actions.onDismissMenu,
            onItem = { actions.onAction(it.id) },
        )
    }
}

/**
 * Resolve a rail letter to a list anchor: the letter's own section when it has
 * one, otherwise the nearest existing section by alphabet position (the shared
 * cross-platform rule from data-model.md).
 */
internal fun nearestAnchor(
    anchors: Map<String, Int>,
    alphabet: List<String>,
    letter: String,
): Int? {
    anchors[letter]?.let { return it }
    if (anchors.isEmpty()) return null
    val target = alphabet.indexOf(letter)
    if (target < 0) return anchors.values.first()
    return anchors.entries.minByOrNull { entry ->
        val position = alphabet.indexOf(entry.key)
        if (position < 0) Int.MAX_VALUE else kotlin.math.abs(position - target)
    }?.value
}
