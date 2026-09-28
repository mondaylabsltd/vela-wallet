package app.getvela.wallet.core.designsystem.components

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.statusBars
import androidx.compose.material3.BottomSheetDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.ModalBottomSheetProperties
import androidx.compose.material3.SheetState
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.nestedscroll.NestedScrollConnection
import androidx.compose.ui.input.nestedscroll.NestedScrollSource
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.compose.ui.unit.Velocity
import androidx.compose.ui.unit.dp

/**
 * THE bottom sheet — every modal sheet in the app goes through here (spec 078
 * round 3, device-found: a quick fling inside a sheet set it jittering up and
 * down without end, on the founder's phone).
 *
 * Why it jittered: Material3's sheet pads its content by the status-bar inset
 * MINUS the sheet's current offset (`consumeWindowInsets(top = offset)` +
 * `safeDrawing` top in `contentWindowInsets`). A tall sheet flung past its top
 * anchor dips into the status-bar zone, so its content grows, so the Expanded
 * anchor (window height − sheet height) moves, so the settle re-targets and
 * overshoots again — a layout value fed by the offset it changes, for ever.
 *
 * The two rules this host holds for every sheet:
 *
 * 1. **Nothing in the sheet's size depends on its offset.** The content takes
 *    only the BOTTOM inset (navigation bar), and the sheet is capped below the
 *    status bar instead — a stable height, so the anchors are stable.
 * 2. **Upward overscroll never reaches the sheet.** A list flung to its end
 *    keeps the leftover; only a DOWNWARD drag or fling that begins with the
 *    content at its top moves the sheet (to dismiss, or to spring back once).
 *
 * [followAppTextSize]: the sheet is its own window, whose root provides its
 * own density — the app's text size (`VelaTheme`'s font scale) stops at the
 * sheet's edge unless the sheet carries it in. The settings sheets do (078
 * round 3); the rest keep their current rendering until each is checked at the
 * largest size.
 *
 * [dismissible] `false`: the sheet closes only when its content says so — no
 * drag, no tap on the scrim, no system Back, and no drag handle suggesting
 * otherwise. The signing and consent sheets (owner, spec 079: "除非用户明确关掉，
 * 不应该很容易误操作，比如下滑就关掉了" — a stray swipe threw away the dApp's
 * request, and the page had to ask again). Such a sheet MUST draw its own close.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun VelaModalSheet(
    onDismissRequest: () -> Unit,
    modifier: Modifier = Modifier,
    sheetState: SheetState = rememberModalBottomSheetState(),
    containerColor: Color = BottomSheetDefaults.ContainerColor,
    dragHandle: @Composable (() -> Unit)? = { BottomSheetDefaults.DragHandle() },
    followAppTextSize: Boolean = false,
    dismissible: Boolean = true,
    content: @Composable ColumnScope.() -> Unit,
) {
    val appDensity = LocalDensity.current
    ModalBottomSheet(
        onDismissRequest = onDismissRequest,
        modifier = modifier,
        sheetState = sheetState,
        sheetGesturesEnabled = dismissible,
        containerColor = containerColor,
        dragHandle = if (dismissible) dragHandle else null,
        // Bottom only: the top inset is the offset-dependent one (rule 1).
        contentWindowInsets = { WindowInsets.safeDrawing.only(WindowInsetsSides.Bottom) },
        properties = ModalBottomSheetProperties(
            shouldDismissOnBackPress = dismissible,
            shouldDismissOnClickOutside = dismissible,
        ),
    ) {
        val sheetDensity = LocalDensity.current
        // Below the status bar, whatever the content: the window's height less
        // the status bar and the handle's room — measured, never the offset.
        val statusBar = WindowInsets.statusBars.getTop(sheetDensity)
        val window = LocalWindowInfo.current.containerSize.height
        val cap = with(sheetDensity) { (window - statusBar).coerceAtLeast(0).toDp() } - SHEET_CHROME
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .then(if (window > 0) Modifier.heightIn(max = cap) else Modifier)
                // No handle on an undismissible sheet: its room stays, so the
                // content sits where it always did.
                .then(if (dismissible) Modifier else Modifier.padding(top = SHEET_CHROME / 2))
                .nestedScroll(SheetOverscrollGuard),
        ) {
            val column = this
            if (followAppTextSize) {
                CompositionLocalProvider(LocalDensity provides appDensity) { column.content() }
            } else {
                column.content()
            }
        }
    }
}

/** The drag handle's room above the content (M3's handle: 22 + 4 + 22). */
private val SHEET_CHROME = 48.dp

/**
 * Rule 2: whatever an inner scroll leaves over UPWARD (negative y — the finger
 * travelling up, a list at its end) is kept here, so the sheet's own
 * nested-scroll connection never turns it into a drag or a settle past its top
 * anchor. Downward leftovers pass: that is how a sheet is pulled closed.
 */
internal object SheetOverscrollGuard : NestedScrollConnection {
    override fun onPostScroll(consumed: Offset, available: Offset, source: NestedScrollSource): Offset =
        if (available.y < 0f) Offset(0f, available.y) else Offset.Zero

    override suspend fun onPostFling(consumed: Velocity, available: Velocity): Velocity =
        if (available.y < 0f) Velocity(0f, available.y) else Velocity.Zero
}
