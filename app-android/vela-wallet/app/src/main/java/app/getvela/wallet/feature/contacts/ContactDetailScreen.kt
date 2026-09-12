package app.getvela.wallet.feature.contacts

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaMonoFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.feature.contacts.components.AddressBlock
import app.getvela.wallet.feature.contacts.components.ContactsMetrics
import app.getvela.wallet.feature.contacts.components.ContactsNavHeader
import app.getvela.wallet.feature.contacts.components.DeleteConfirmSheet
import app.getvela.wallet.feature.contacts.components.DestructiveTextButton
import app.getvela.wallet.feature.contacts.components.EmptyStateCta
import app.getvela.wallet.feature.contacts.components.GroupChips
import app.getvela.wallet.feature.contacts.components.Hairline
import app.getvela.wallet.feature.contacts.components.glyphFor
import app.getvela.wallet.feature.wallet.components.ActionButtonItem
import app.getvela.wallet.feature.wallet.components.ActionButtonRow
import app.getvela.wallet.feature.wallet.components.ActivityRow
import app.getvela.wallet.feature.wallet.components.IdenticonAvatar
import app.getvela.wallet.feature.wallet.components.SectionHeader

/**
 * Contact detail (mocks C2 / C2s): back + pencil header, hero identicon, name,
 * short address, group chips, the reused three-card action dock, the mono
 * 地址 block, 最近往来 over spec-015 ActivityRows, and the centred destructive
 * 删除联系人 that raises the confirmation sheet.
 */
@Composable
fun ContactDetailScreen(
    model: ContactsHomeModel,
    modifier: Modifier = Modifier,
    actions: ContactsActions = ContactsActions(),
) {
    val detail = model.detail ?: return
    val colors = VelaTheme.colors
    Box(
        modifier = modifier
            .fillMaxSize()
            .background(colors.bgBase),
    ) {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .statusBarsPadding()
                .verticalScroll(rememberScrollState())
                .navigationBarsPadding()
                .padding(horizontal = VelaSizing.screenPaddingX),
        ) {
            Spacer(modifier = Modifier.height(VelaSpacing.md))
            ContactsNavHeader(
                backContentDescription = model.backLabel,
                trailingIcon = VelaIcons.Pencil,
                trailingContentDescription = detail.editLabel,
                onBack = { actions.onAction("contacts.back") },
                onTrailing = { actions.onAction("contacts.edit") },
            )

            Spacer(modifier = Modifier.height(VelaSpacing.xl3))
            Column(
                modifier = Modifier.align(Alignment.CenterHorizontally),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                IdenticonAvatar(
                    seed = detail.contact.addressFull,
                    size = ContactsMetrics.heroAvatar,
                    contentDescription = detail.contact.name,
                )
                Spacer(modifier = Modifier.height(VelaSpacing.xl))
                Text(
                    text = detail.contact.name,
                    color = colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.bold,
                    fontSize = VelaTextSize.xl3,
                    maxLines = 1,
                )
                Spacer(modifier = Modifier.height(VelaSpacing.md))
                Text(
                    text = detail.contact.addressDisplay,
                    color = colors.fgMuted,
                    fontFamily = VelaMonoFontFamily,
                    fontWeight = VelaFontWeight.regular,
                    fontSize = VelaTextSize.base,
                    maxLines = 1,
                )
                detail.inspection?.let { inspection ->
                    // Spec 045 US7: the core's word on this address — contract or
                    // wallet, and whether the wallet has paid it before.
                    Spacer(modifier = Modifier.height(VelaSpacing.md))
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        inspection.tag?.let { tag ->
                            Text(
                                text = tag,
                                color = colors.fgMuted,
                                fontFamily = VelaFontFamily,
                                fontSize = VelaTextSize.xs,
                                modifier = Modifier
                                    .background(colors.bgRaised, RoundedCornerShape(VelaRadius.full))
                                    .padding(horizontal = VelaSpacing.md, vertical = VelaSpacing.xs)
                                    .semantics { contentDescription = "contact-inspection-tag" },
                            )
                        }
                        inspection.firstTime?.let { first ->
                            if (inspection.tag != null) Spacer(modifier = Modifier.width(VelaSpacing.sm))
                            Text(
                                text = first,
                                color = colors.fgSubtle,
                                fontFamily = VelaFontFamily,
                                fontSize = VelaTextSize.xs,
                            )
                        }
                    }
                }
                detail.favourite?.let { favourite ->
                    // Spec 045 US5: the favourite star — 018's section word as its label.
                    Spacer(modifier = Modifier.height(VelaSpacing.md))
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        modifier = Modifier
                            .clip(RoundedCornerShape(VelaRadius.full))
                            .clickable { actions.onAction("contacts.favourite") }
                            .padding(horizontal = VelaSpacing.md, vertical = VelaSpacing.xs)
                            .semantics { contentDescription = if (favourite.on) "favourite-on" else "favourite-off" },
                    ) {
                        Icon(
                            imageVector = VelaIcons.Star,
                            contentDescription = null,
                            tint = if (favourite.on) colors.accentBase else colors.fgSubtle,
                            modifier = Modifier.size(VelaIconSize.sm),
                        )
                        Spacer(modifier = Modifier.width(VelaSpacing.xs))
                        Text(
                            text = favourite.label,
                            color = if (favourite.on) colors.accentBase else colors.fgMuted,
                            fontFamily = VelaFontFamily,
                            fontSize = VelaTextSize.sm,
                        )
                    }
                }
                Spacer(modifier = Modifier.height(VelaSpacing.lg))
                GroupChips(
                    model = detail.chips,
                    onGroup = { actions.onAction("contacts.sectionGroups") },
                    onAdd = { actions.onAction("contacts.moveGroup") },
                )
            }

            Spacer(modifier = Modifier.height(VelaSpacing.xl3))
            ActionButtonRow(
                items = detail.actions.map { action ->
                    ActionButtonItem(
                        icon = glyphFor(action.icon),
                        label = action.label,
                        onClick = { actions.onAction("contacts.action." + action.icon.name) },
                    )
                },
            )

            Spacer(modifier = Modifier.height(VelaSpacing.xl3))
            Hairline()
            Spacer(modifier = Modifier.height(VelaSpacing.xl3))
            AddressBlock(
                model = detail.address,
                onCopy = { actions.onAction("contacts.copyAddress") },
            )

            Spacer(modifier = Modifier.height(VelaSpacing.xl3))
            SectionHeader(
                title = detail.activity.title,
                action = detail.activity.action,
                onAction = { actions.onAction("history.filterAll") },
            )
            if (detail.activity.rows.isEmpty()) {
                detail.activity.empty?.let {
                    EmptyStateCta(model = it, icon = VelaIcons.Inbox)
                }
            } else {
                detail.activity.rows.forEach { row -> ActivityRow(model = row) }
            }

            Spacer(modifier = Modifier.height(VelaSpacing.xl4))
            DestructiveTextButton(
                label = detail.deleteLabel,
                onClick = { actions.onAction("contacts.deleteContact") },
            )
            Spacer(modifier = Modifier.height(VelaSpacing.xl4))
        }
    }

    model.deleteConfirm?.let { confirm ->
        DeleteConfirmSheet(
            model = confirm,
            onDismiss = actions.onDismissMenu,
            onConfirm = { actions.onAction("contacts.delete") },
        )
    }
}
