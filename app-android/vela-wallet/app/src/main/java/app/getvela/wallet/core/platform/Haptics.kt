package app.getvela.wallet.core.platform

import android.content.Context
import android.os.Build
import android.os.VibrationEffect
import android.os.Vibrator
import android.os.VibratorManager

/**
 * The one buzz this app performs outside a button press: money arriving.
 *
 * The core decides WHEN — `activity_feed` fires its haptic operation only for
 * a genuinely new incoming record, bound to the read that earned it. This just
 * performs it.
 *
 * **Success, not alert.** `EFFECT_DOUBLE_CLICK` is the closest stock effect to
 * the two-beat "done" pattern; a long buzz reads as a warning, which is the
 * opposite of what has happened.
 *
 * Silent when the device has no vibrator, when the person has haptics off, or
 * on any platform refusal — a wallet that crashes because a phone would not
 * buzz has its priorities wrong.
 */
object Haptics {

    fun moneyIn(context: Context) {
        val vibrator = vibrator(context) ?: return
        if (!vibrator.hasVibrator()) return
        runCatching {
            vibrator.vibrate(VibrationEffect.createPredefined(VibrationEffect.EFFECT_DOUBLE_CLICK))
        }
    }

    private fun vibrator(context: Context): Vibrator? = runCatching {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            val manager = context.getSystemService(VibratorManager::class.java)
            manager?.defaultVibrator
        } else {
            @Suppress("DEPRECATION")
            context.getSystemService(Vibrator::class.java)
        }
    }.getOrNull()
}
