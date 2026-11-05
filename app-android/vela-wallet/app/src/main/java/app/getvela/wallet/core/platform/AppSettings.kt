package app.getvela.wallet.core.platform

import android.app.Activity
import android.content.Context
import android.content.ContextWrapper
import android.content.Intent
import android.net.Uri
import android.provider.Settings

/**
 * This app's page in the system Settings — where a permission the person
 * refused for good is given back (iOS opens `openSettingsURLString` for the
 * same reason). The only way out of a permanent denial: the system will not
 * show its dialog again, so asking again is a button that does nothing.
 */
object AppSettings {
    /** `Settings.ACTION_APPLICATION_DETAILS_SETTINGS` for [packageName]. */
    fun detailsIntent(packageName: String): Intent =
        Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.fromParts("package", packageName, null))
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)

    /** Opens this app's details page; `false` when no Settings app takes it. */
    fun open(context: Context): Boolean =
        runCatching { context.startActivity(detailsIntent(context.packageName)) }.isSuccess

    /** The activity behind a Compose [Context] (it is usually wrapped). */
    tailrec fun activityOf(context: Context): Activity? = when (context) {
        is Activity -> context
        is ContextWrapper -> activityOf(context.baseContext)
        else -> null
    }
}
