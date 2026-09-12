package app.getvela.wallet.feature.contacts

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
import androidx.compose.ui.Modifier
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.feature.contacts.components.ActionMenuSheet
import app.getvela.wallet.feature.contacts.components.ContactRow
import app.getvela.wallet.feature.contacts.components.ContactsMetrics
import app.getvela.wallet.feature.contacts.components.ContactsNavHeader
import app.getvela.wallet.feature.contacts.components.GhostAddRow
import app.getvela.wallet.feature.contacts.components.Hairline
import app.getvela.wallet.feature.contacts.components.PinnedCtaBar

/**
 * Group detail (mocks C4 / C6): back + ⋯ header, group name with its member
 * count, member rows, the ghost 添加成员 row, and the bottom-pinned accent
 * 群发转账 CTA with its caption. The ⋯ button raises the group action sheet.
 */
@Composable
fun GroupDetailScreen(
    model: ContactsHomeModel,
    modifier: Modifier = Modifier,
    actions: ContactsActions = ContactsActions(),
) {
    val group = model.groupDetail ?: return
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
                .navigationBarsPadding(),
        ) {
            Column(
                modifier = Modifier
                    .weight(1f)
                    .verticalScroll(rememberScrollState())
                    .padding(horizontal = VelaSizing.screenPaddingX),
            ) {
                Spacer(modifier = Modifier.height(VelaSpacing.md))
                ContactsNavHeader(
                    backContentDescription = model.backLabel,
                    trailingIcon = VelaIcons.Ellipsis,
                    trailingContentDescription = group.manageLabel,
                    onBack = { actions.onAction("contacts.back") },
                    onTrailing = { actions.onAction("contacts.groupMenu") },
                )

                Spacer(modifier = Modifier.height(VelaSpacing.xl))
                Text(
                    text = group.name,
                    color = colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.bold,
                    fontSize = VelaTextSize.xl4,
                    maxLines = 1,
                )
                Spacer(modifier = Modifier.height(VelaSpacing.sm))
                Text(
                    text = group.membersLabel,
                    color = colors.fgMuted,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.regular,
                    fontSize = VelaTextSize.base,
                    maxLines = 1,
                )

                Spacer(modifier = Modifier.height(VelaSpacing.xl2))
                group.members.forEach { member ->
                    ContactRow(
                        contact = member,
                        avatarSize = ContactsMetrics.memberAvatar,
                        onClick = { actions.onContact(member) },
                    )
                    Hairline()
                }
                GhostAddRow(
                    label = group.addMemberLabel,
                    onClick = { actions.onAction("contacts.addMember") },
                )
                Spacer(modifier = Modifier.height(VelaSpacing.xl4))
            }
            PinnedCtaBar(
                label = group.batchSendLabel,
                caption = group.batchSendHint,
                enabled = group.members.isNotEmpty(),
                onClick = { actions.onAction("contacts.batchSend") },
                modifier = Modifier.padding(
                    start = VelaSizing.screenPaddingX,
                    end = VelaSizing.screenPaddingX,
                    bottom = VelaSpacing.xl3,
                ),
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
