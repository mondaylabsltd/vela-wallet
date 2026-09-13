package app.getvela.wallet.feature.send.core

import java.io.ByteArrayInputStream
import java.util.zip.ZipInputStream

/**
 * A workbook's first sheet as a cell matrix — the shell's half of the batch
 * file path (spec 045 D3): the core interprets the table, this only reads
 * it. Shared strings, inline strings and plain values; gaps are empty cells.
 */
object XlsxMatrix {
    private val SI = Regex("<si>(.*?)</si>", RegexOption.DOT_MATCHES_ALL)
    private val T = Regex("<t[^>]*>(.*?)</t>", RegexOption.DOT_MATCHES_ALL)
    private val ROW = Regex("<row\\b[^>]*>(.*?)</row>", RegexOption.DOT_MATCHES_ALL)
    private val CELL = Regex("<c\\b([^>]*?)(?:/>|>(.*?)</c>)", RegexOption.DOT_MATCHES_ALL)
    private val V = Regex("<v>(.*?)</v>", RegexOption.DOT_MATCHES_ALL)
    private val ATTR_R = Regex("\\br=\"([A-Z]+)\\d*\"")
    private val ATTR_T = Regex("\\bt=\"([^\"]+)\"")

    fun read(bytes: ByteArray): List<List<String>> {
        var shared: List<String> = emptyList()
        var sheet: String? = null
        ZipInputStream(ByteArrayInputStream(bytes)).use { zip ->
            var entry = zip.nextEntry
            while (entry != null) {
                when (entry.name) {
                    "xl/sharedStrings.xml" -> shared = SI.findAll(zip.readBytes().decodeToString())
                        .map { si -> T.findAll(si.groupValues[1]).joinToString("") { unescape(it.groupValues[1]) } }
                        .toList()
                    "xl/worksheets/sheet1.xml" -> sheet = zip.readBytes().decodeToString()
                }
                entry = zip.nextEntry
            }
        }
        val xml = sheet ?: return emptyList()
        return ROW.findAll(xml).map { row -> cells(row.groupValues[1], shared) }.toList()
    }

    private fun cells(row: String, shared: List<String>): List<String> {
        val out = ArrayList<String>()
        CELL.findAll(row).forEach { cell ->
            val attrs = cell.groupValues[1]
            val body = cell.groupValues[2]
            val column = ATTR_R.find(attrs)?.groupValues?.get(1)?.let(::columnIndex) ?: out.size
            while (out.size < column) out.add("")
            val value = when (ATTR_T.find(attrs)?.groupValues?.get(1)) {
                "s" -> V.find(body)?.groupValues?.get(1)?.trim()?.toIntOrNull()?.let { shared.getOrNull(it) } ?: ""
                "inlineStr" -> T.findAll(body).joinToString("") { unescape(it.groupValues[1]) }
                else -> V.find(body)?.groupValues?.get(1)?.let(::unescape) ?: ""
            }
            if (out.size == column) out.add(value) else out[column] = value
        }
        return out
    }

    /** `A` → 0, `Z` → 25, `AA` → 26. */
    internal fun columnIndex(letters: String): Int =
        letters.fold(0) { acc, c -> acc * 26 + (c - 'A' + 1) } - 1

    private fun unescape(text: String): String = text
        .replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}
