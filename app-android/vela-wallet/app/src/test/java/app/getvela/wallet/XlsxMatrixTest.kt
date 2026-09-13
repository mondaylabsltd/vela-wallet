package app.getvela.wallet

import app.getvela.wallet.feature.send.core.XlsxMatrix
import java.io.ByteArrayOutputStream
import java.util.zip.ZipEntry
import java.util.zip.ZipOutputStream
import org.junit.Assert.assertEquals
import org.junit.Test

class XlsxMatrixTest {
    private fun workbook(shared: String, sheet: String): ByteArray {
        val out = ByteArrayOutputStream()
        ZipOutputStream(out).use { zip ->
            zip.putNextEntry(ZipEntry("[Content_Types].xml")); zip.write("<Types/>".toByteArray()); zip.closeEntry()
            zip.putNextEntry(ZipEntry("xl/sharedStrings.xml")); zip.write(shared.toByteArray()); zip.closeEntry()
            zip.putNextEntry(ZipEntry("xl/worksheets/sheet1.xml")); zip.write(sheet.toByteArray()); zip.closeEntry()
        }
        return out.toByteArray()
    }

    @Test
    fun `shared strings, inline strings, numbers and gaps come out as one matrix`() {
        val shared = """<sst><si><t>name</t></si><si><t>Alice</t></si><si><r><t>Bob &amp; </t></r><r><t>Co</t></r></si></sst>"""
        val sheet = """<worksheet><sheetData>
            <row r="1"><c r="A1" t="s"><v>0</v></c><c r="B1" t="inlineStr"><is><t>address</t></is></c><c r="C1" t="inlineStr"><is><t>amount</t></is></c></row>
            <row r="2"><c r="A2" t="s"><v>1</v></c><c r="B2" t="inlineStr"><is><t>0x1111111111111111111111111111111111111111</t></is></c><c r="C2"><v>5000</v></c></row>
            <row r="3"><c r="A3" t="s"><v>2</v></c><c r="C3"><v>8000.5</v></c></row>
        </sheetData></worksheet>"""
        val rows = XlsxMatrix.read(workbook(shared, sheet))
        assertEquals(listOf("name", "address", "amount"), rows[0])
        assertEquals(listOf("Alice", "0x1111111111111111111111111111111111111111", "5000"), rows[1])
        assertEquals(listOf("Bob & Co", "", "8000.5"), rows[2])
    }

    @Test
    fun `column letters index as a spreadsheet does`() {
        assertEquals(0, XlsxMatrix.columnIndex("A"))
        assertEquals(25, XlsxMatrix.columnIndex("Z"))
        assertEquals(26, XlsxMatrix.columnIndex("AA"))
        assertEquals(27, XlsxMatrix.columnIndex("AB"))
    }
}
