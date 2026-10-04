package app.getvela.wallet.feature.contacts

import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.tooling.preview.Preview
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.core.i18n.VelaStrings

/**
 * Preview-only translation fake (never shipped): the tooling process cannot
 * load the native engine (same pattern as WalletPreviews' PreviewStrings).
 * Sample copy mirrors locales/zh — the corpus stays the single source of truth
 * for the real app, and `ContactsFixturesTest` pins the real resolution.
 */
private object ContactsPreviewStrings : VelaStrings {
    private val sample = mapOf(
        I18nKeys.Contacts.TITLE to "通讯录",
        I18nKeys.Contacts.SECTION_GROUPS to "分组",
        I18nKeys.Contacts.SECTION_CONTACTS to "联系人",
        I18nKeys.Contacts.MANAGE to "管理",
        I18nKeys.Contacts.COUNT_PEOPLE to "{{count}} 位",
        I18nKeys.Contacts.GROUP_MEMBERS to "{{count}} 人",
        I18nKeys.Contacts.MEMBERS_COUNT to "{{count}} 位成员",
        I18nKeys.Contacts.ALL_CONTACTS to "全部联系人",
        I18nKeys.Contacts.SEARCH_PLACEHOLDER to "搜索名字、ENS 或地址",
        I18nKeys.Contacts.NO_RESULTS to "没有匹配「{{query}}」的结果",
        I18nKeys.Contacts.EMPTY to "还没有联系人",
        I18nKeys.Contacts.EMPTY_HINT to "添加常用地址，转账时不再反复粘贴。也可以从文件导入现有通讯录。",
        I18nKeys.Contacts.ADD_CONTACT to "添加联系人",
        I18nKeys.Contacts.ADDRESS_LABEL to "地址",
        I18nKeys.Contacts.RECENT_ACTIVITY to "最近往来",
        I18nKeys.Contacts.VIEW_ALL_ACTIVITY to "查看全部往来",
        I18nKeys.Contacts.DELETE_CONTACT to "删除联系人",
        I18nKeys.Contacts.ACTION_QR to "二维码",
        I18nKeys.Contacts.EDIT to "编辑",
        I18nKeys.Contacts.MOVE_GROUP to "移入分组",
        I18nKeys.Contacts.ADD_MEMBER to "添加成员",
        I18nKeys.Contacts.BATCH_SEND to "群发转账",
        I18nKeys.Contacts.BATCH_SEND_HINT to "向本组 {{count}} 人转账，金额可分别设置。",
        I18nKeys.Contacts.BATCH_SEND_HINT_TITLED to
            "群发转账：向本组 {{count}} 人转账，金额可分别设置。",
        I18nKeys.Contacts.GROUP_NEW to "新建分组",
        I18nKeys.Contacts.GROUP_EDIT to "编辑分组",
        I18nKeys.Contacts.GROUP_RENAME to "重命名分组",
        I18nKeys.Contacts.GROUP_DELETE to "删除分组",
        I18nKeys.Contacts.ADD_TITLE to "新建联系人",
        I18nKeys.Contacts.IMPORT_FILE to "从文件导入",
        I18nKeys.Contacts.IMPORT_ALL to "导入通讯录",
        I18nKeys.Contacts.EXPORT_TITLE to "导出通讯录",
        I18nKeys.Contacts.EXPORT_ALL to "导出全部通讯录",
        I18nKeys.Contacts.IMPORT_GROUP to "导入到本组",
        I18nKeys.Contacts.EXPORT_GROUP to "导出本组",
        I18nKeys.Contacts.DELETE_TITLE to "删除联系人？",
        I18nKeys.Contacts.DELETE_BODY to "{{name}} 将从通讯录中移除。",
        I18nKeys.Contacts.DELETE to "删除",
        I18nKeys.Contacts.CANCEL to "取消",
        I18nKeys.Contacts.ACTION_SEND to "转账",
        I18nKeys.Contacts.ACTION_RECEIVE to "收款",
        I18nKeys.Contacts.COPY_ADDRESS to "复制地址",
        I18nKeys.Contacts.LABEL_SENT to "已发送",
        I18nKeys.Contacts.LABEL_RECEIVED to "已收到",
        I18nKeys.Contacts.FILTER_ALL to "全部",
        I18nKeys.Contacts.NAV_WALLET to "钱包",
        I18nKeys.Contacts.NAV_CONTACTS to "通讯录",
        I18nKeys.Contacts.NAV_EXPLORE to "探索",
        I18nKeys.Contacts.NAV_SETTINGS to "设置",
    )

    override fun t(key: String): String = sample[key] ?: key.substringAfterLast('.')

    override fun t(key: String, vars: Map<String, String>): String =
        vars.entries.fold(t(key)) { acc, (name, value) -> acc.replace("{{$name}}", value) }

    /** Previews carry one sample per key, so a plural key is its own sample. */
    override fun t(key: String, count: Int): String = t(key, mapOf("count" to count.toString()))
}

@Composable
private fun ContactsPreviewContent(darkTheme: Boolean, state: ContactsScreenState) {
    VelaTheme(darkTheme = darkTheme) {
        CompositionLocalProvider(LocalVelaStrings provides ContactsPreviewStrings) {
            ContactsRoute(
                model = ContactsFixtures.buildMobileState(state, ContactsPreviewStrings),
            )
        }
    }
}

@Preview(name = "Contacts home — dark (C1)")
@Composable
private fun ContactsHomePreviewDark() {
    ContactsPreviewContent(darkTheme = true, state = ContactsScreenState.C1)
}

@Preview(name = "Contacts home — light (C1, token-derived)")
@Composable
private fun ContactsHomePreviewLight() {
    ContactsPreviewContent(darkTheme = false, state = ContactsScreenState.C1)
}

@Preview(name = "Contact detail — dark (C2)")
@Composable
private fun ContactDetailPreviewDark() {
    ContactsPreviewContent(darkTheme = true, state = ContactsScreenState.C2)
}

@Preview(name = "Group detail — dark (C4)")
@Composable
private fun GroupDetailPreviewDark() {
    ContactsPreviewContent(darkTheme = true, state = ContactsScreenState.C4)
}

@Preview(name = "Contacts empty — light (C3)")
@Composable
private fun ContactsEmptyPreviewLight() {
    ContactsPreviewContent(darkTheme = false, state = ContactsScreenState.C3)
}
