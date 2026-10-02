package app.getvela.wallet

import app.getvela.wallet.feature.documents.DocumentPorts
import app.getvela.wallet.feature.documents.PickedDocument
import app.getvela.wallet.feature.send.core.BatchExecutor
import app.getvela.wallet.feature.send.core.BatchFileContent
import app.getvela.wallet.feature.send.core.BatchOperation
import app.getvela.wallet.feature.send.core.BatchShellResult
import java.io.ByteArrayOutputStream
import java.util.zip.ZipEntry
import java.util.zip.ZipOutputStream
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class BatchExecutorTest {
    private class FakePorts(val picked: PickedDocument?, val creates: Boolean = true) : DocumentPorts {
        var created: Triple<String, String, String>? = null
        override suspend fun pick(mimes: List<String>): PickedDocument? = picked
        override suspend fun create(name: String, mime: String, bytes: ByteArray): Boolean {
            created = Triple(name, mime, bytes.decodeToString()); return creates
        }
        override suspend fun share(name: String, mime: String, bytes: ByteArray): Boolean = true
    }

    @Test
    fun `a csv arrives as its bytes, an xlsx as a matrix, a cancel as cancelled`() = runBlocking<Unit> {
        val csv = BatchExecutor({ null }, { FakePorts(PickedDocument("two-rows.csv", "name,address,amount\nA,0x11,1\n".toByteArray())) })
        val text = csv.perform(BatchOperation.PickFile) as BatchShellResult.FilePicked
        assertEquals("two-rows.csv", text.name)
        assertEquals(
            "name,address,amount\nA,0x11,1\n".toByteArray().map { it.toInt() },
            (text.content as BatchFileContent.Bytes).bytes,
        )

        // 087: never decoded here — a GBK name's bytes reach the core as they
        // are (0–255), for the core to refuse rather than garble.
        val gbk = byteArrayOf(0xD5.toByte(), 0xC5.toByte(), 0xC8.toByte(), 0xFD.toByte())
        val legacy = BatchExecutor({ null }, { FakePorts(PickedDocument("gbk.csv", gbk)) })
            .perform(BatchOperation.PickFile) as BatchShellResult.FilePicked
        assertEquals(listOf(0xD5, 0xC5, 0xC8, 0xFD), (legacy.content as BatchFileContent.Bytes).bytes)

        val out = ByteArrayOutputStream()
        ZipOutputStream(out).use { zip ->
            zip.putNextEntry(ZipEntry("xl/worksheets/sheet1.xml"))
            zip.write("""<sheetData><row r="1"><c r="A1"><v>1</v></c><c r="B1"><v>2</v></c></row></sheetData>""".toByteArray())
            zip.closeEntry()
        }
        val xlsx = BatchExecutor({ null }, { FakePorts(PickedDocument("pay.XLSX", out.toByteArray())) })
        val matrix = xlsx.perform(BatchOperation.PickFile) as BatchShellResult.FilePicked
        assertEquals(listOf(listOf("1", "2")), (matrix.content as BatchFileContent.Matrix).rows)

        val cancelled = BatchExecutor({ null }, { FakePorts(null) })
        assertEquals(BatchShellResult.FilePickCancelled, cancelled.perform(BatchOperation.PickFile))
        val unattached = BatchExecutor({ null }, { null })
        assertEquals(BatchShellResult.FilePickFailed, unattached.perform(BatchOperation.PickFile))
    }

    @Test
    fun `the rate is the wallet's waterfall and never a defaulted one`() = runBlocking<Unit> {
        val executor = BatchExecutor({ code -> if (code == "CNY") 7.25 else null }, { null })
        assertEquals(7.25, (executor.perform(BatchOperation.FetchUsdFiatRate("CNY")) as BatchShellResult.RateResolved).rate)
        assertNull((executor.perform(BatchOperation.FetchUsdFiatRate("XXX")) as BatchShellResult.RateResolved).rate)
        assertNull((BatchExecutor({ -1.0 }, { null }).perform(BatchOperation.FetchUsdFiatRate("CNY")) as BatchShellResult.RateResolved).rate)
    }

    @Test
    fun `the template goes through the creator with the core's name and mime`() = runBlocking<Unit> {
        val ports = FakePorts(null)
        val executor = BatchExecutor({ null }, { ports })
        assertEquals(BatchShellResult.TemplateSaved, executor.perform(BatchOperation.SaveTemplateFile("vela-batch-template.csv", "name,address,amount\n", "text/csv")))
        assertEquals(Triple("vela-batch-template.csv", "text/csv", "name,address,amount\n"), ports.created)
        assertEquals(BatchShellResult.TemplateSaveFailed, BatchExecutor({ null }, { FakePorts(null, creates = false) }).perform(BatchOperation.SaveTemplateFile("t.csv", "x", "text/csv")))
    }
}
