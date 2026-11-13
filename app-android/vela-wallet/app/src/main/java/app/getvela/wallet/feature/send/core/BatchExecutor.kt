package app.getvela.wallet.feature.send.core

import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.feature.documents.DocumentPorts

/**
 * The `batch_import` machine's three shell duties (spec 045 D3): the fiat
 * rate through the wallet's own waterfall (never a defaulted 1 — the core
 * refuses to price on `null`), the document picker (every file as its bytes —
 * the core reads a workbook and decodes text itself; this app's own XLSX
 * reader is gone, so the same payroll reads the same everywhere), and the
 * document creator for the template.
 */
class BatchExecutor(
    private val fiatRate: suspend (String) -> Double?,
    private val documents: () -> DocumentPorts?,
) {
    suspend fun perform(operation: BatchOperation): BatchShellResult = when (operation) {
        is BatchOperation.FetchUsdFiatRate -> BatchShellResult.RateResolved(
            operation.code,
            rate = fiatRate(operation.code)?.takeIf { it.isFinite() && it > 0.0 },
        )
        BatchOperation.PickFile -> {
            val ports = documents()
            if (ports == null) {
                VelaLog.event("send.batch", "no document ports attached")
                BatchShellResult.FilePickFailed
            } else {
                val picked = ports.pick(PICK_MIMES) ?: return BatchShellResult.FilePickCancelled
                BatchShellResult.FilePicked(picked.name, BatchFileContent.Bytes.of(picked.bytes))
            }
        }
        is BatchOperation.SaveTemplateFile -> {
            val saved = documents()?.create(operation.name, operation.mime, operation.contents.toByteArray()) ?: false
            if (saved) BatchShellResult.TemplateSaved else BatchShellResult.TemplateSaveFailed
        }
    }

    /** What to answer when the shell itself threw — the machine must not wait forever. */
    fun neutralAnswer(operation: BatchOperation): BatchShellResult = when (operation) {
        is BatchOperation.FetchUsdFiatRate -> BatchShellResult.RateResolved(operation.code, null)
        BatchOperation.PickFile -> BatchShellResult.FilePickFailed
        is BatchOperation.SaveTemplateFile -> BatchShellResult.TemplateSaveFailed
    }

    companion object {
        val PICK_MIMES = listOf(
            "text/csv",
            "text/comma-separated-values",
            "text/plain",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            "application/vnd.ms-excel",
        )
    }
}
