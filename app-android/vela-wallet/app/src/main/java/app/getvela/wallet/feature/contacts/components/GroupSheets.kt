package app.getvela.wallet.feature.contacts.components

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import app.getvela.wallet.core.designsystem.components.VelaAddressStrip
import app.getvela.wallet.core.designsystem.components.VelaPrimaryButton
import app.getvela.wallet.core.designsystem.components.VelaSecondaryButton
import app.getvela.wallet.core.designsystem.components.VelaTextField
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.feature.flows.components.QrCard

/**
 * Spec 048: the sheets the contacts machine had no doors to on the phone —
 * a group's name (new and rename), its ⋯ menu (rename, delete), the delete
 * confirmation, and a contact's own code (the detail's 二维码). The web's
 * GroupEditSheet / group menu / contact QR sheet, at phone width.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ContactsSheet(onDismiss: () -> Unit, content: @Composable () -> Unit) {
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = sheetState, containerColor = VelaTheme.colors.bgBase) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = VelaSpacing.xl)
                .padding(bottom = VelaSpacing.xl)
                .navigationBarsPadding(),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) { content() }
    }
}

@Composable
private fun SheetTitle(text: String) {
    Text(
        text = text,
        color = VelaTheme.colors.fgBase,
        fontFamily = VelaFontFamily,
        fontWeight = VelaFontWeight.semibold,
        fontSize = VelaTextSize.xl,
        modifier = Modifier.fillMaxWidth().padding(bottom = VelaSpacing.lg),
    )
}

@Composable
fun GroupEditSheet(
    title: String,
    initialName: String,
    nameLabel: String,
    saveLabel: String,
    cancelLabel: String,
    onSave: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    var name by remember(initialName) { mutableStateOf(initialName) }
    ContactsSheet(onDismiss) {
        SheetTitle(title)
        VelaTextField(value = name, onValueChange = { name = it }, label = nameLabel, placeholder = nameLabel, modifier = Modifier.fillMaxWidth())
        Spacer(modifier = Modifier.height(VelaSpacing.xl))
        VelaPrimaryButton(saveLabel, enabled = name.isNotBlank(), onClick = { onSave(name.trim()) }, modifier = Modifier.fillMaxWidth())
        Spacer(modifier = Modifier.height(VelaSpacing.lg))
        VelaSecondaryButton(cancelLabel, onClick = onDismiss, modifier = Modifier.fillMaxWidth())
    }
}

@Composable
fun GroupMenuSheet(
    title: String,
    renameLabel: String,
    deleteLabel: String,
    cancelLabel: String,
    onRename: () -> Unit,
    onDelete: () -> Unit,
    onDismiss: () -> Unit,
) {
    ContactsSheet(onDismiss) {
        SheetTitle(title)
        VelaSecondaryButton(renameLabel, onClick = onRename, modifier = Modifier.fillMaxWidth())
        Spacer(modifier = Modifier.height(VelaSpacing.lg))
        VelaSecondaryButton(deleteLabel, onClick = onDelete, modifier = Modifier.fillMaxWidth())
        Spacer(modifier = Modifier.height(VelaSpacing.lg))
        VelaSecondaryButton(cancelLabel, onClick = onDismiss, modifier = Modifier.fillMaxWidth())
    }
}

@Composable
fun GroupDeleteConfirmSheet(
    title: String,
    confirmLabel: String,
    cancelLabel: String,
    onConfirm: () -> Unit,
    onDismiss: () -> Unit,
) {
    ContactsSheet(onDismiss) {
        SheetTitle(title)
        VelaPrimaryButton(confirmLabel, onClick = onConfirm, modifier = Modifier.fillMaxWidth())
        Spacer(modifier = Modifier.height(VelaSpacing.lg))
        VelaSecondaryButton(cancelLabel, onClick = onDismiss, modifier = Modifier.fillMaxWidth())
    }
}

@Composable
fun ContactQrSheet(
    name: String,
    address: String,
    copyLabel: String,
    copiedLabel: String,
    closeLabel: String,
    onDismiss: () -> Unit,
) {
    ContactsSheet(onDismiss) {
        SheetTitle(name)
        QrCard(label = name, payload = address)
        Spacer(modifier = Modifier.height(VelaSpacing.xl))
        VelaAddressStrip(address = address, copyLabel = copyLabel, copiedLabel = copiedLabel, modifier = Modifier.fillMaxWidth())
        Spacer(modifier = Modifier.height(VelaSpacing.xl))
        VelaSecondaryButton(closeLabel, onClick = onDismiss, modifier = Modifier.fillMaxWidth())
    }
}
