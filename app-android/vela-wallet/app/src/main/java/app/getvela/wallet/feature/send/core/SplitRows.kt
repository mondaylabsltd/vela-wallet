package app.getvela.wallet.feature.send.core

/**
 * The three edits a split's rows can take, each rebuilt as the ONE whole-list
 * `RecipientsChanged` the core accepts (spec 045 D1, desktop 033 phase 3):
 * untouched rows are the same instances, ids and names ride along, and a
 * new row carries an EMPTY id so the core mints its own (`rcpt_{n}`).
 */
object SplitRows {
    fun amountEdited(rows: List<SendRecipientDraft>, id: String, amount: String): List<SendRecipientDraft> =
        rows.map { row -> if (row.id == id && row.amount != amount) row.copy(amount = amount) else row }

    fun addressEdited(rows: List<SendRecipientDraft>, id: String, address: String): List<SendRecipientDraft> =
        rows.map { row -> if (row.id == id && row.address != address) row.copy(address = address, name = null) else row }

    fun removed(rows: List<SendRecipientDraft>, id: String): List<SendRecipientDraft> =
        rows.filterNot { it.id == id }

    /**
     * One amount into every row that has none (the web's `fillEmptyAmounts`):
     * a bulk edit of the drafts, like adding a row. Rows with a figure keep it.
     */
    fun emptyFilled(rows: List<SendRecipientDraft>, amount: String): List<SendRecipientDraft> =
        rows.map { row -> if (row.amount.isBlank()) row.copy(amount = amount) else row }

    fun appended(rows: List<SendRecipientDraft>): List<SendRecipientDraft> =
        rows + SendRecipientDraft(id = "", address = "", amount = "")
}
