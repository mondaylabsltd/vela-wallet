package app.getvela.wallet

import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.feature.contacts.ContactsFixtures
import app.getvela.wallet.feature.contacts.ContactsIcon
import app.getvela.wallet.feature.contacts.ContactsScreenState
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Spec 018 FR-012: the contacts fixture builder must reproduce the mock content
 * VERBATIM once merged with the zh corpus (the mocks are zh renderings), and
 * the canon addresses must stay byte-exact so identicon artwork matches across
 * the four clients (SC-003). Runs on the real engine via the host dylib, same
 * as WalletFixturesTest.
 */
class ContactsFixturesTest {

    private val repoRoot = File(
        System.getProperty("vela.repo.root")
            ?: error("vela.repo.root not set — run via Gradle (testOptions wires it)"),
    )

    private fun zhStrings(): I18nRuntime = I18nRuntime { tag ->
        File(repoRoot, "assets/i18n/$tag.json").readBytes()
    }.apply { initialize("zh") }

    @Test
    fun stateInventoryMatchesTheGalleryContract() {
        assertEquals(
            listOf("C1", "C1S", "C1F", "C2", "C2S", "C3", "C4", "C5", "C6", "C7", "C8", "C9"),
            ContactsScreenState.entries.map { it.name },
        )
    }

    @Test
    fun canonAddressesAreByteExact() {
        val strings = zhStrings()
        val contacts = ContactsFixtures
            .buildMobileState(ContactsScreenState.C1, strings)
            .sections
            .flatMap { it.contacts }

        assertEquals(8, contacts.size)
        assertEquals(
            listOf(
                "Alice",
                "阿豪",
                "Bartholomew Vanderbilt-Konstantinopoulos.eth",
                "Bob · 泵泵",
                "Charlie",
                "DAO 金库",
                "hold on",
                "妈妈",
            ),
            contacts.map { it.name },
        )
        assertEquals(
            listOf(
                "0x9F3c…21aE",
                "0x77Bd…4F02",
                "0x31c9…E77a",
                "0x44Aa…9C21",
                "0x5eF0…3a9C",
                "0xF00d…C0de",
                "0xCafe…F00d",
                "0x88Ce…12aB",
            ),
            contacts.map { it.addressDisplay },
        )
        assertEquals(
            listOf(
                "0x9F3cA71b04E82f5C55d9B21aE00734F8Dd8021aE",
                "0x77Bd59A302cC93D23dB0d0BA6a45C6830EF74F02",
                "0x31c9A100517d2436E9E1350D383A7d0aAeC1E77a",
                "0x44AaF19cE84f22101b5D6cbA918B92DcA5f19C21",
                "0x5eF0FF25a1A24E5cCb2a6D939B87F5DAb2003a9C",
                "0xF00dBaBe8712004343cD00926Ab004D6C042C0de",
                "0xCafe9078B1c2A04d33Ff21B0BC934eB8A812F00d",
                "0x88Ce02FdB0e50D9C21e33c0F9B58a3E38f7612aB",
            ),
            contacts.map { it.addressFull },
        )
        // Seeds go to the identicon exactly as pinned — no call-site lowercasing.
        assertTrue(contacts.all { it.addressFull == it.addressFull.trim() })
        assertTrue(contacts.any { it.addressFull != it.addressFull.lowercase() })

        // 表弟 exists only inside the 家人 group (the recorded C1-vs-DC1 delta).
        val cousin = ContactsFixtures.groupDetail(strings).members[1]
        assertEquals("表弟", cousin.name)
        assertEquals("0xA1c3…88dD", cousin.addressDisplay)
        assertEquals("0xA1c3D3d7085B90AF14E5d21C86e6dB49F30a88dD", cousin.addressFull)

        // Identicon board: the 9 canon seeds plus the placeholder probe.
        assertEquals(10, ContactsFixtures.IDENTICON_BOARD_SEEDS.size)
        assertEquals("", ContactsFixtures.IDENTICON_BOARD_SEEDS.last())
    }

    @Test
    fun c1MatchesTheMockVerbatim() {
        val model = ContactsFixtures.buildMobileState(ContactsScreenState.C1, zhStrings())

        assertEquals("通讯录", model.title)
        assertEquals("添加联系人", model.addContactLabel)
        assertEquals("返回", model.backLabel)
        assertEquals("搜索名字、ENS 或地址", model.search.placeholder)
        assertTrue(!model.search.filtering)
        assertEquals("分组", model.groupsSectionTitle)
        assertEquals("管理", model.groupsAction)
        assertEquals(
            listOf("家人" to "3 人", "工作" to "5 人", "交易所" to "2 人"),
            model.groups.map { it.name to it.countLabel },
        )
        assertEquals("联系人", model.contactsSectionTitle)
        assertEquals("8 位", model.totalLabel)
        assertEquals(listOf("A", "B", "C", "D", "H", "M"), model.sections.map { it.letter })
        // The rail always renders the full alphabet plus "#".
        assertEquals(27, model.indexLetters.size)
        assertEquals("A", model.indexLetters.first())
        assertEquals("#", model.indexLetters.last())
        assertEquals("通讯录", model.tabs.contacts)
        assertEquals("钱包", model.tabs.wallet)
        assertNull(model.revealedIndex)
        assertEquals("转账", model.swipeSendLabel)
        assertEquals("删除", model.swipeDeleteLabel)
        assertEquals(1f, model.textScale, 0f)
    }

    @Test
    fun c1sRevealsTheSecondRowAndC1fFiltersToAlice() {
        val strings = zhStrings()
        val swiped = ContactsFixtures.buildMobileState(ContactsScreenState.C1S, strings)
        assertEquals(1, swiped.revealedIndex)
        assertEquals(8, swiped.sections.sumOf { it.contacts.size })

        val filtered = ContactsFixtures.buildMobileState(ContactsScreenState.C1F, strings)
        assertEquals("Ali", filtered.search.query)
        assertTrue(filtered.search.filtering)
        assertEquals(listOf("A"), filtered.sections.map { it.letter })
        assertEquals(listOf("Alice"), filtered.sections.flatMap { it.contacts }.map { it.name })
        assertEquals("1 位", filtered.totalLabel)
    }

    @Test
    fun c2CarriesAliceDetailVerbatim() {
        val model = ContactsFixtures.buildMobileState(ContactsScreenState.C2, zhStrings())
        val detail = model.detail ?: error("C2 must carry the contact detail")

        assertEquals("Alice", detail.contact.name)
        assertEquals("0x9F3c…21aE", detail.contact.addressDisplay)
        assertEquals(listOf("家人"), detail.chips.groups)
        assertEquals("分组", detail.chips.addLabel)
        assertEquals(
            listOf("转账", "收款", "二维码"),
            detail.actions.map { it.label },
        )
        assertEquals(
            listOf(ContactsIcon.Send, ContactsIcon.Receive, ContactsIcon.Qr),
            detail.actions.map { it.icon },
        )

        assertEquals("地址", detail.address.label)
        assertEquals(
            listOf("0x9F3cA71b04E82f5C55d9", "B21aE00734F8Dd8021aE"),
            detail.address.lines,
        )
        // The two mono lines rejoin to exactly the canon full address.
        assertEquals(detail.contact.addressFull, detail.address.lines.joinToString(""))
        assertEquals("复制地址", detail.address.copyLabel)

        assertEquals("最近往来", detail.activity.title)
        assertEquals("全部", detail.activity.action)
        assertEquals(2, detail.activity.rows.size)
        val received = detail.activity.rows[0]
        assertEquals("已收到", received.title)
        assertEquals("昨天 20:15 · Ethereum", received.subtitle)
        assertEquals("+50", received.amount)
        assertEquals("USDC", received.unit)
        assertTrue(received.positive)
        val sent = detail.activity.rows[1]
        assertEquals("已发送", sent.title)
        assertEquals("8 月 5 日 · Arbitrum", sent.subtitle)
        assertEquals("−0.2", sent.amount)
        assertEquals("ETH", sent.unit)
        assertTrue(!sent.positive)

        assertEquals("编辑", detail.editLabel)
        assertEquals("删除联系人", detail.deleteLabel)
        assertNull(model.deleteConfirm)
    }

    @Test
    fun c2sRaisesTheDestructiveConfirmationNamingAlice() {
        val model = ContactsFixtures.buildMobileState(ContactsScreenState.C2S, zhStrings())
        val confirm = model.deleteConfirm ?: error("C2s must carry the delete confirmation")
        assertNotNull(model.detail)
        assertEquals("删除联系人？", confirm.title)
        assertEquals("Alice 将从通讯录中移除。", confirm.body)
        assertEquals("删除", confirm.confirm)
        assertEquals("取消", confirm.cancel)
    }

    @Test
    fun c3IsEmptyWithBothCtas() {
        val model = ContactsFixtures.buildMobileState(ContactsScreenState.C3, zhStrings())
        val empty = model.empty ?: error("C3 must carry the empty state")
        assertEquals("还没有联系人", empty.title)
        assertEquals(
            "添加常用地址，转账时不再反复粘贴。也可以从文件导入现有通讯录。",
            empty.caption,
        )
        assertEquals("添加联系人", empty.primaryCta)
        assertEquals("从文件导入", empty.secondaryCta)
        assertTrue(model.sections.isEmpty())
        assertTrue(model.groups.isEmpty())
    }

    @Test
    fun c4CarriesTheFamilyGroupVerbatim() {
        val model = ContactsFixtures.buildMobileState(ContactsScreenState.C4, zhStrings())
        val group = model.groupDetail ?: error("C4 must carry the group detail")
        assertEquals("家人", group.name)
        assertEquals("3 位成员", group.membersLabel)
        assertEquals(listOf("妈妈", "表弟", "Alice"), group.members.map { it.name })
        assertEquals("添加成员", group.addMemberLabel)
        assertEquals("群发转账", group.batchSendLabel)
        assertEquals("向本组 3 人转账，金额可分别设置。", group.batchSendHint)
        assertEquals("管理", group.manageLabel)
    }

    @Test
    fun c5AndC6MenusMatchTheMocks() {
        val strings = zhStrings()

        val c5 = ContactsFixtures.buildMobileState(ContactsScreenState.C5, strings)
        val addMenu = c5.menu ?: error("C5 must carry the add menu")
        assertEquals(
            listOf("新建联系人", "从文件导入", "导出通讯录"),
            addMenu.items.map { it.label },
        )
        assertEquals(
            listOf(ContactsIcon.AddContact, ContactsIcon.Import, ContactsIcon.Export),
            addMenu.items.map { it.icon },
        )
        assertTrue(addMenu.items.none { it.destructive })
        assertEquals("取消", addMenu.cancel)

        val c6 = ContactsFixtures.buildMobileState(ContactsScreenState.C6, strings)
        val groupMenu = c6.menu ?: error("C6 must carry the group menu")
        assertNotNull(c6.groupDetail)
        assertEquals(
            listOf("编辑分组", "导入到本组", "导出本组", "删除分组"),
            groupMenu.items.map { it.label },
        )
        assertEquals(
            listOf(ContactsIcon.Edit, ContactsIcon.Import, ContactsIcon.Export, ContactsIcon.Delete),
            groupMenu.items.map { it.icon },
        )
        // Divider sits before the destructive row only (M2 anatomy).
        assertEquals(listOf(false, false, false, true), groupMenu.items.map { it.dividerBefore })
        assertEquals(listOf(false, false, false, true), groupMenu.items.map { it.destructive })
        assertEquals("取消", groupMenu.cancel)
    }

    @Test
    fun unmockedVariantsReuseTheExistingTreatments() {
        val strings = zhStrings()

        // Search-empty reuses EmptyState with contacts.noResults (no CTAs).
        val searchEmpty = ContactsFixtures.searchEmptyState(strings)
        assertEquals("没有匹配「zzz」的结果", searchEmpty.title)
        assertNull(searchEmpty.primaryCta)

        // Empty group: 0 members, caption re-counted, CTA disabled by the screen.
        val emptyGroup = ContactsFixtures.emptyGroupDetail(strings)
        assertTrue(emptyGroup.members.isEmpty())
        assertEquals("0 位成员", emptyGroup.membersLabel)
        assertEquals("向本组 0 人转账，金额可分别设置。", emptyGroup.batchSendHint)

        // Contact with no activity falls back to the reused empty treatment.
        val noActivity = ContactsFixtures.contactDetailNoActivity(strings)
        assertTrue(noActivity.activity.rows.isEmpty())
        assertEquals("还没有联系人", noActivity.activity.empty?.title)
    }
}
