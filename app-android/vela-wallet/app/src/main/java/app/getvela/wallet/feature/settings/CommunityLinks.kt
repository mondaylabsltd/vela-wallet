package app.getvela.wallet.feature.settings

/**
 * Vela's official social accounts (founder, 2026-09-27: 「设置里面再加一下，我们的
 * 官方社交账号链接」) — defined ONCE here, exactly as getvela.app's footer
 * publishes them, and pinned by a test so the four shells cannot drift.
 * Titles and handles are brand names and are never translated.
 */
object CommunityLinks {

    /** One row: its id, the brand's name, the handle shown under it, and where it goes. */
    data class Link(val id: String, val title: String, val handle: String, val url: String, val icon: SettingsIcon)

    val X = Link("community-x", "X (Twitter)", "@realvelawallet", "https://x.com/realvelawallet", SettingsIcon.BrandX)
    val TELEGRAM = Link("community-telegram", "Telegram", "@velawallet", "https://t.me/velawallet", SettingsIcon.BrandTelegram)
    val DISCORD = Link("community-discord", "Discord", "discord.gg/23gWrtaYSa", "https://discord.gg/23gWrtaYSa", SettingsIcon.BrandDiscord)

    val ALL: List<Link> = listOf(X, TELEGRAM, DISCORD)

    /** The URL a row id opens, or null when the id is not a community row. */
    fun urlFor(rowId: String): String? = ALL.firstOrNull { it.id == rowId }?.url
}
