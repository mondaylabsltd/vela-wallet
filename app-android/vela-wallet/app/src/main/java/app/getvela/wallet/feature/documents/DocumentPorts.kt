package app.getvela.wallet.feature.documents

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.provider.OpenableColumns
import androidx.activity.ComponentActivity
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.content.FileProvider
import java.io.File
import java.io.IOException
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * The platform's documents, as three shell duties (spec 045): pick a file,
 * create one, hand one to the share sheet. The core never sees a URI — it
 * gets a name and bytes, and gives a name, a mime and content back.
 */
interface DocumentPorts {
    /** The system picker; `null` when the person backed out. */
    suspend fun pick(mimes: List<String>): PickedDocument?

    /** The system creator ("save as"); `false` when backed out or unwritable. */
    suspend fun create(name: String, mime: String, bytes: ByteArray): Boolean

    /** The share sheet with the file attached; `true` once the sheet is up. */
    suspend fun share(name: String, mime: String, bytes: ByteArray): Boolean
}

class PickedDocument(val name: String, val bytes: ByteArray)

/**
 * Activity-bound: the two result launchers must be registered before the
 * activity is STARTED, so this is built in `MainActivity.onCreate` and
 * attached to the container, exactly like the security-key ceremony.
 */
class ActivityDocumentPorts(private val activity: ComponentActivity) : DocumentPorts {
    private var pickAnswer: CompletableDeferred<Uri?>? = null
    private var createAnswer: CompletableDeferred<Uri?>? = null

    private val pickLauncher = activity.registerForActivityResult(ActivityResultContracts.StartActivityForResult()) { result ->
        pickAnswer?.complete(if (result.resultCode == Activity.RESULT_OK) result.data?.data else null)
        pickAnswer = null
    }

    private val createLauncher = activity.registerForActivityResult(ActivityResultContracts.StartActivityForResult()) { result ->
        createAnswer?.complete(if (result.resultCode == Activity.RESULT_OK) result.data?.data else null)
        createAnswer = null
    }

    override suspend fun pick(mimes: List<String>): PickedDocument? {
        val answer = CompletableDeferred<Uri?>()
        pickAnswer = answer
        val intent = Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
            addCategory(Intent.CATEGORY_OPENABLE)
            type = "*/*"
            putExtra(Intent.EXTRA_MIME_TYPES, mimes.toTypedArray())
        }
        withContext(Dispatchers.Main) { pickLauncher.launch(intent) }
        val uri = answer.await() ?: return null
        return withContext(Dispatchers.IO) {
            val bytes = activity.contentResolver.openInputStream(uri)?.use { it.readBytes() }
                ?: throw IOException("unreadable document")
            PickedDocument(displayName(uri) ?: uri.lastPathSegment ?: "file", bytes)
        }
    }

    override suspend fun create(name: String, mime: String, bytes: ByteArray): Boolean {
        val answer = CompletableDeferred<Uri?>()
        createAnswer = answer
        val intent = Intent(Intent.ACTION_CREATE_DOCUMENT).apply {
            addCategory(Intent.CATEGORY_OPENABLE)
            type = mime
            putExtra(Intent.EXTRA_TITLE, name)
        }
        withContext(Dispatchers.Main) { createLauncher.launch(intent) }
        val uri = answer.await() ?: return false
        return withContext(Dispatchers.IO) {
            val out = activity.contentResolver.openOutputStream(uri, "wt") ?: return@withContext false
            out.use { it.write(bytes) }
            true
        }
    }

    override suspend fun share(name: String, mime: String, bytes: ByteArray): Boolean {
        val file = withContext(Dispatchers.IO) {
            val dir = File(activity.cacheDir, "shared").apply { mkdirs() }
            File(dir, name).apply { writeBytes(bytes) }
        }
        val uri = FileProvider.getUriForFile(activity, "${activity.packageName}.files", file)
        val send = Intent(Intent.ACTION_SEND).apply {
            type = mime
            putExtra(Intent.EXTRA_STREAM, uri)
            putExtra(Intent.EXTRA_SUBJECT, name)
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        }
        withContext(Dispatchers.Main) { activity.startActivity(Intent.createChooser(send, name)) }
        return true
    }

    private fun displayName(uri: Uri): String? =
        activity.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { cursor ->
            if (cursor.moveToFirst()) cursor.getString(0) else null
        }
}
