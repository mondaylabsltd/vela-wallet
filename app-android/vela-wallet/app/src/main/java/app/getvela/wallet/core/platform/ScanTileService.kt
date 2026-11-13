package app.getvela.wallet.core.platform

import android.app.PendingIntent
import android.content.Intent
import android.os.Build
import android.service.quicksettings.Tile
import android.service.quicksettings.TileService
import app.getvela.wallet.MainActivity
import app.getvela.wallet.VelaWalletApplication
import app.getvela.wallet.core.diagnostics.VelaLog

/**
 * The Quick Settings Scan tile (native parity Phase 3): one tap from any
 * screen to the wallet's scanner — the home dock's 扫码, which opens Send with
 * the scanner on top, where the core's `scan_resolved` rules on what it reads.
 *
 * Stateless (`STATE_INACTIVE` always): it is a door, not a switch. Its label
 * is the dock's own word in the app's language, set each time the shade shows
 * it (the manifest's label is only the tile picker's).
 */
class ScanTileService : TileService() {
    override fun onStartListening() {
        super.onStartListening()
        val tile = qsTile ?: return
        val runtime = (application as VelaWalletApplication).container.i18nRuntime
        if (runtime.state.value.ready) {
            tile.label = runtime.t(LauncherShortcuts.labelKey(LaunchEntry.Flow.Scan))
        }
        tile.state = Tile.STATE_INACTIVE
        tile.updateTile()
    }

    override fun onClick() {
        super.onClick()
        VelaLog.event("tile", "scan")
        val intent = Intent(this, MainActivity::class.java)
            .setAction(LaunchEntry.Flow.Scan.action)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP)
        // Unlocked first: a wallet's scanner is not for the lock screen.
        unlockAndRun { open(intent) }
    }

    private fun open(intent: Intent) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            startActivityAndCollapse(
                PendingIntent.getActivity(this, 0, intent, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT),
            )
        } else {
            @Suppress("DEPRECATION")
            startActivityAndCollapse(intent)
        }
    }
}
