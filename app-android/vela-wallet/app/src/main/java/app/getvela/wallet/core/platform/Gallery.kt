package app.getvela.wallet.core.platform

import android.content.ContentValues
import android.content.Context
import android.net.Uri
import android.os.Environment
import android.provider.MediaStore
import app.getvela.wallet.core.diagnostics.VelaLog

/**
 * "保存图片" the way a phone means it (spec 048): the image lands in the
 * system gallery under Pictures/Vela, no permission asked on API 29+, and the
 * URI comes back for a share. `null` means the gallery refused — the caller
 * says so and the share sheet stays the other way out.
 */
object Gallery {
    fun savePng(context: Context, name: String, bytes: ByteArray): Uri? {
        val resolver = context.contentResolver
        val values = ContentValues().apply {
            put(MediaStore.Images.Media.DISPLAY_NAME, name)
            put(MediaStore.Images.Media.MIME_TYPE, "image/png")
            put(MediaStore.Images.Media.RELATIVE_PATH, Environment.DIRECTORY_PICTURES + "/Vela")
            put(MediaStore.Images.Media.IS_PENDING, 1)
        }
        val uri = runCatching { resolver.insert(MediaStore.Images.Media.EXTERNAL_CONTENT_URI, values) }.getOrNull()
            ?: return null.also { VelaLog.event("gallery", "insert refused", "name" to name) }
        return runCatching {
            resolver.openOutputStream(uri)?.use { it.write(bytes) } ?: error("no output stream")
            values.clear()
            values.put(MediaStore.Images.Media.IS_PENDING, 0)
            resolver.update(uri, values, null, null)
            VelaLog.event("gallery", "saved", "name" to name, "bytes" to bytes.size)
            uri
        }.getOrElse { error ->
            VelaLog.failure("gallery", "save failed", error, "name" to name)
            runCatching { resolver.delete(uri, null, null) }
            null
        }
    }
}
