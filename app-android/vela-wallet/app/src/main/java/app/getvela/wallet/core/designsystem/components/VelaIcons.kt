package app.getvela.wallet.core.designsystem.components

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathFillType
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.addPathNodes
import androidx.compose.ui.unit.dp

/**
 * Hand-built icon corpus (24×24 viewport) from specs/015-wallet-home-ui/contracts/icons.json.
 *
 * Every glyph is Lucide v1.11.0 (contract rev 2): stroke icons carry stroke 2 =
 * icon.stroke.base with round caps/joins and no fill; nav solid variants are
 * fills derived from the same lucide geometry (evenodd holes; the contacts
 * back-person arcs stay stroked), so selection swaps style without layout
 * shift (spec FR-007).
 *
 * Paths carry currentColor black; tint at the call site via Icon(tint = …).
 */
object VelaIcons {

    private fun strokeIcon(name: String, vararg paths: String): ImageVector =
        ImageVector.Builder(
            name = name,
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            for (d in paths) {
                addPath(
                    pathData = addPathNodes(d),
                    fill = null,
                    stroke = SolidColor(Color.Black),
                    strokeLineWidth = 2f,
                    strokeLineCap = StrokeCap.Round,
                    strokeLineJoin = StrokeJoin.Round,
                )
            }
        }.build()

    /**
     * A brand mark (simple-icons, CC0): a filled 24-unit glyph drawn at 20
     * inside the 24 frame — the same inset iOS uses — so it sits at the optical
     * size of the stroked row icons around it. Monochrome: tinted like them.
     */
    private fun brandIcon(name: String, path: String): ImageVector =
        ImageVector.Builder(
            name = name,
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            addGroup(scaleX = 0.8333f, scaleY = 0.8333f, translationX = 2f, translationY = 2f)
            addPath(pathData = addPathNodes(path), fill = SolidColor(Color.Black))
            clearGroup()
        }.build()

    /** Settings → Community (spec 078 round 3): X, Telegram, Discord. */
    val BrandX: ImageVector by lazy { brandIcon("VelaBrandX", "M18.901 1.153h3.68l-8.04 9.19L24 22.846h-7.406l-5.8-7.584-6.638 7.584H.474l8.6-9.83L0 1.154h7.594l5.243 6.932ZM17.61 20.644h2.039L6.486 3.24H4.298Z") }
    val BrandTelegram: ImageVector by lazy { brandIcon("VelaBrandTelegram", "M11.944 0A12 12 0 0 0 0 12a12 12 0 0 0 12 12 12 12 0 0 0 12-12A12 12 0 0 0 12 0a12 12 0 0 0-.056 0zm4.962 7.224c.1-.002.321.023.465.14a.506.506 0 0 1 .171.325c.016.093.036.306.02.472-.18 1.898-.962 6.502-1.36 8.627-.168.9-.499 1.201-.82 1.23-.696.065-1.225-.46-1.9-.902-1.056-.693-1.653-1.124-2.678-1.8-1.185-.78-.417-1.21.258-1.91.177-.184 3.247-2.977 3.307-3.23.007-.032.014-.15-.056-.212s-.174-.041-.249-.024c-.106.024-1.793 1.14-5.061 3.345-.48.33-.913.49-1.302.48-.428-.008-1.252-.241-1.865-.44-.752-.245-1.349-.374-1.297-.789.027-.216.325-.437.893-.663 3.498-1.524 5.83-2.529 6.998-3.014 3.332-1.386 4.025-1.627 4.476-1.635z") }
    val BrandDiscord: ImageVector by lazy { brandIcon("VelaBrandDiscord", "M20.317 4.3698a19.7913 19.7913 0 0 0 -4.8851 -1.5152 .0741 .0741 0 0 0 -.0785 .0371c-.211 .3753 -.4447 .8648 -.6083 1.2495 -1.8447 -.2762 -3.68 -.2762 -5.4868 0 -.1636 -.3933 -.4058 -.8742 -.6177 -1.2495a.077 .077 0 0 0 -.0785 -.037 19.7363 19.7363 0 0 0 -4.8852 1.515 .0699 .0699 0 0 0 -.0321 .0277C.5334 9.0458 -.319 13.5799 .0992 18.0578a.0824 .0824 0 0 0 .0312 .0561c2.0528 1.5076 4.0413 2.4228 5.9929 3.0294a.0777 .0777 0 0 0 .0842 -.0276c.4616 -.6304 .8731 -1.2952 1.226 -1.9942a.076 .076 0 0 0 -.0416 -.1057c-.6528 -.2476 -1.2743 -.5495 -1.8722 -.8923a.077 .077 0 0 1 -.0076 -.1277c.1258 -.0943 .2517 -.1923 .3718 -.2914a.0743 .0743 0 0 1 .0776 -.0105c3.9278 1.7933 8.18 1.7933 12.0614 0a.0739 .0739 0 0 1 .0785 .0095c.1202 .099 .246 .1981 .3728 .2924a.077 .077 0 0 1 -.0066 .1276 12.2986 12.2986 0 0 1 -1.873 .8914 .0766 .0766 0 0 0 -.0407 .1067c.3604 .698 .7719 1.3628 1.225 1.9932a.076 .076 0 0 0 .0842 .0286c1.961 -.6067 3.9495 -1.5219 6.0023 -3.0294a.077 .077 0 0 0 .0313 -.0552c.5004 -5.177 -.8382 -9.6739 -3.5485 -13.6604a.061 .061 0 0 0 -.0312 -.0286zM8.02 15.3312c-1.1825 0 -2.1569 -1.0857 -2.1569 -2.419 0 -1.3332 .9555 -2.4189 2.157 -2.4189 1.2108 0 2.1757 1.0952 2.1568 2.419 0 1.3332 -.9555 2.4189 -2.1569 2.4189zm7.9748 0c-1.1825 0 -2.1569 -1.0857 -2.1569 -2.419 0 -1.3332 .9554 -2.4189 2.1569 -2.4189 1.2108 0 2.1757 1.0952 2.1568 2.419 0 1.3332 -.946 2.4189 -2.1568 2.4189Z") }

    private fun evenOddFillIcon(name: String, vararg paths: String): ImageVector =
        ImageVector.Builder(
            name = name,
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            for (d in paths) {
                addPath(
                    pathData = addPathNodes(d),
                    fill = SolidColor(Color.Black),
                    pathFillType = PathFillType.EvenOdd,
                )
            }
        }.build()

    private fun fillIcon(name: String, vararg paths: String): ImageVector =
        ImageVector.Builder(
            name = name,
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            for (d in paths) {
                addPath(
                    pathData = addPathNodes(d),
                    fill = SolidColor(Color.Black),
                )
            }
        }.build()

    /** lucide arrow-left (spec 009 placeholder screens). */
    val ArrowLeft: ImageVector by lazy {
        strokeIcon("VelaArrowLeft", "M19 12L5 12", "M12 19L5 12L12 5")
    }

    // --- Nav (lucide; stroke outline unselected / derived solid selected) ------

    val NavWalletOutline: ImageVector by lazy {
        strokeIcon(
            "VelaNavWalletOutline",
            "M19 7V4a1 1 0 0 0-1-1H5a2 2 0 0 0 0 4h15a1 1 0 0 1 1 1v4h-3a2 2 0 0 0 0 4h3a1 1 0 0 0 1-1v-2a1 1 0 0 0-1-1",
            "M3 5v14a2 2 0 0 0 2 2h15a1 1 0 0 0 1-1v-4",
        )
    }

    val NavWalletSolid: ImageVector by lazy {
        fillIcon(
            "VelaNavWalletSolid",
            "M18 3a1 1 0 0 1 1 1v3h1a1 1 0 0 1 1 1v3h-4a2 2 0 0 0 0 4h4v4a1 1 0 0 1-1 1H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h13z",
        )
    }

    val NavContactsOutline: ImageVector by lazy {
        strokeIcon(
            "VelaNavContactsOutline",
            "M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2",
            "M16 3.128a4 4 0 0 1 0 7.744",
            "M22 21v-2a4 4 0 0 0-3-3.87",
            "M13 7a4 4 0 1 1-8 0a4 4 0 1 1 8 0",
        )
    }

    /** Mixed per the contract: filled body + head, the back-person arcs stay stroked. */
    val NavContactsSolid: ImageVector by lazy {
        ImageVector.Builder(
            name = "VelaNavContactsSolid",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            addPath(
                pathData = addPathNodes("M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2z"),
                fill = SolidColor(Color.Black),
            )
            addPath(
                pathData = addPathNodes("M13 7a4 4 0 1 1-8 0a4 4 0 1 1 8 0"),
                fill = SolidColor(Color.Black),
            )
            for (d in arrayOf("M16 3.128a4 4 0 0 1 0 7.744", "M22 21v-2a4 4 0 0 0-3-3.87")) {
                addPath(
                    pathData = addPathNodes(d),
                    fill = null,
                    stroke = SolidColor(Color.Black),
                    strokeLineWidth = 2f,
                    strokeLineCap = StrokeCap.Round,
                    strokeLineJoin = StrokeJoin.Round,
                )
            }
        }.build()
    }

    val NavExploreOutline: ImageVector by lazy {
        strokeIcon(
            "VelaNavExploreOutline",
            "M22 12a10 10 0 1 1-20 0a10 10 0 1 1 20 0",
            "m16.24 7.76-1.804 5.411a2 2 0 0 1-1.265 1.265L7.76 16.24l1.804-5.411a2 2 0 0 1 1.265-1.265z",
        )
    }

    val NavExploreSolid: ImageVector by lazy {
        evenOddFillIcon(
            "VelaNavExploreSolid",
            "M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20zM16.24 7.76l-1.804 5.411a2 2 0 0 1-1.265 1.265L7.76 16.24l1.804-5.411a2 2 0 0 1 1.265-1.265z",
        )
    }

    val NavSettingsOutline: ImageVector by lazy {
        strokeIcon(
            "VelaNavSettingsOutline",
            "M9.671 4.136a2.34 2.34 0 0 1 4.659 0 2.34 2.34 0 0 0 3.319 1.915 2.34 2.34 0 0 1 2.33 4.033 2.34 2.34 0 0 0 0 3.831 2.34 2.34 0 0 1-2.33 4.033 2.34 2.34 0 0 0-3.319 1.915 2.34 2.34 0 0 1-4.659 0 2.34 2.34 0 0 0-3.32-1.915 2.34 2.34 0 0 1-2.33-4.033 2.34 2.34 0 0 0 0-3.831A2.34 2.34 0 0 1 6.35 6.051a2.34 2.34 0 0 0 3.319-1.915",
            "M15 12a3 3 0 1 1-6 0a3 3 0 1 1 6 0",
        )
    }

    val NavSettingsSolid: ImageVector by lazy {
        evenOddFillIcon(
            "VelaNavSettingsSolid",
            "M9.671 4.136a2.34 2.34 0 0 1 4.659 0 2.34 2.34 0 0 0 3.319 1.915 2.34 2.34 0 0 1 2.33 4.033 2.34 2.34 0 0 0 0 3.831 2.34 2.34 0 0 1-2.33 4.033 2.34 2.34 0 0 0-3.319 1.915 2.34 2.34 0 0 1-4.659 0 2.34 2.34 0 0 0-3.32-1.915 2.34 2.34 0 0 1-2.33-4.033 2.34 2.34 0 0 0 0-3.831A2.34 2.34 0 0 1 6.35 6.051a2.34 2.34 0 0 0 3.319-1.915zM12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6z",
        )
    }

    // --- Utility (stroke style, Lucide) -----------------------------------------

    val ArrowDownLeft: ImageVector by lazy {
        strokeIcon("VelaArrowDownLeft", "M17 7 7 17", "M17 17H7V7")
    }

    val ArrowUpRight: ImageVector by lazy {
        strokeIcon("VelaArrowUpRight", "M7 7h10v10", "M7 17 17 7")
    }

    val ScanLine: ImageVector by lazy {
        strokeIcon(
            "VelaScanLine",
            "M3 7V5a2 2 0 0 1 2-2h2",
            "M17 3h2a2 2 0 0 1 2 2v2",
            "M21 17v2a2 2 0 0 1-2 2h-2",
            "M7 21H5a2 2 0 0 1-2-2v-2",
            "M7 12h10",
        )
    }

    val Eye: ImageVector by lazy {
        strokeIcon(
            "VelaEye",
            "M2.062 12.348a1 1 0 0 1 0-.696 10.75 10.75 0 0 1 19.876 0 1 1 0 0 1 0 .696 10.75 10.75 0 0 1-19.876 0",
            // circle cx=12 cy=12 r=3
            "M9 12a3 3 0 1 0 6 0a3 3 0 1 0-6 0",
        )
    }

    val EyeOff: ImageVector by lazy {
        strokeIcon(
            "VelaEyeOff",
            "M10.733 5.076a10.744 10.744 0 0 1 11.205 6.575 1 1 0 0 1 0 .696 10.747 10.747 0 0 1-1.444 2.49",
            "M14.084 14.158a3 3 0 0 1-4.242-4.242",
            "M17.479 17.499a10.75 10.75 0 0 1-15.417-5.151 1 1 0 0 1 0-.696 10.75 10.75 0 0 1 4.446-5.143",
            "m2 2 20 20",
        )
    }

    val Search: ImageVector by lazy {
        strokeIcon(
            "VelaSearch",
            "m21 21-4.34-4.34",
            // circle cx=11 cy=11 r=8
            "M3 11a8 8 0 1 0 16 0a8 8 0 1 0-16 0",
        )
    }

    val Close: ImageVector by lazy {
        strokeIcon("VelaClose", "M18 6 6 18", "m6 6 12 12")
    }

    /** Lucide `image-plus`: the report's add-screenshots target (spec 078 round 3). */
    val ImagePlus: ImageVector by lazy {
        strokeIcon(
            "VelaImagePlus",
            "M16 5h6",
            "M19 2v6",
            "M21 11.5V19a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h7.5",
            "m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21",
            // circle cx=9 cy=9 r=2
            "M7 9a2 2 0 1 0 4 0a2 2 0 1 0-4 0",
        )
    }

    val Copy: ImageVector by lazy {
        strokeIcon(
            "VelaCopy",
            // rect x=8 y=8 w=14 h=14 rx=2
            "M10 8h10a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H10a2 2 0 0 1-2-2V10a2 2 0 0 1 2-2z",
            "M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2",
        )
    }

    val ChevronRight: ImageVector by lazy {
        strokeIcon("VelaChevronRight", "m9 18 6-6-6-6")
    }

    val ChevronDown: ImageVector by lazy {
        strokeIcon("VelaChevronDown", "m6 9 6 6 6-6")
    }

    val TriangleAlert: ImageVector by lazy {
        strokeIcon(
            "VelaTriangleAlert",
            "m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3",
            "M12 9v4",
            "M12 17h.01",
        )
    }

    val RefreshCw: ImageVector by lazy {
        strokeIcon(
            "VelaRefreshCw",
            "M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8",
            "M21 3v5h-5",
            "M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16",
            "M8 16H3v5",
        )
    }

    val Check: ImageVector by lazy {
        strokeIcon("VelaCheck", "M20 6 9 17l-5-5")
    }

    val Inbox: ImageVector by lazy {
        strokeIcon(
            "VelaInbox",
            // polyline 22 12 16 12 14 15 10 15 8 12 2 12
            "M22 12L16 12L14 15L10 15L8 12L2 12",
            "M5.45 5.11 2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z",
        )
    }

    val Wallet: ImageVector by lazy {
        strokeIcon(
            "VelaWallet",
            "M19 7V4a1 1 0 0 0-1-1H5a2 2 0 0 0 0 4h15a1 1 0 0 1 1 1v4h-3a2 2 0 0 0 0 4h3a1 1 0 0 0 1-1v-2a1 1 0 0 0-1-1",
            "M3 5v14a2 2 0 0 0 2 2h15a1 1 0 0 0 1-1v-4",
        )
    }

    val Link2: ImageVector by lazy {
        strokeIcon(
            "VelaLink2",
            "M9 17H7A5 5 0 0 1 7 7h2",
            "M15 7h2a5 5 0 1 1 0 10h-2",
            // line x1=8 y1=12 x2=16 y2=12
            "M8 12L16 12",
        )
    }

    /** lucide clock — timeout badge glyph (spec 014 E3). */
    val Clock: ImageVector by lazy {
        strokeIcon(
            "VelaClock",
            "M12 2a10 10 0 1 0 0 20 10 10 0 1 0 0-20",
            "M12 6v6l4 2",
        )
    }

    /** Bare exclamation — warning/neutral/info badge glyph (spec 014); the
     *  no-Text-literals rule forbids rendering it as a "!" string. */
    val Exclamation: ImageVector by lazy {
        strokeIcon(
            "VelaExclamation",
            "M12 6v7",
            "M12 17h.01",
        )
    }

    // --- Spec 018 contacts utility glyphs -------------------------------------
    // Bodies copied verbatim from specs/018-contacts-ui/contracts/icons.json
    // (lucide v1.11.0). <circle>/<rect> elements are expressed as the equivalent
    // arc/rounded-rect path data — Compose has no primitive-shape node.

    /** lucide user-round-plus — header add-contact button · 新建联系人 row. */
    val UserRoundPlus: ImageVector by lazy {
        strokeIcon(
            "VelaUserRoundPlus",
            "M2 21a8 8 0 0 1 13.292-6",
            // circle cx=10 cy=8 r=5
            "M5 8a5 5 0 1 0 10 0a5 5 0 1 0-10 0",
            "M19 16v6",
            "M22 19h-6",
        )
    }

    /** lucide users-round — group tile (GroupRow) · desktop rail rows. */
    val UsersRound: ImageVector by lazy {
        strokeIcon(
            "VelaUsersRound",
            "M18 21a8 8 0 0 0-16 0",
            // circle cx=10 cy=8 r=5
            "M5 8a5 5 0 1 0 10 0a5 5 0 1 0-10 0",
            "M22 20c0-3.37-2-6.5-4-8a5 5 0 0 0-.45-8.3",
        )
    }

    /** lucide folder-plus — 新建分组. */
    val FolderPlus: ImageVector by lazy {
        strokeIcon(
            "VelaFolderPlus",
            "M12 10v6",
            "M9 13h6",
            "M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 " +
                "7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z",
        )
    }

    /** lucide download — 从文件导入 / 导入通讯录 / 导入到本组. */
    val Download: ImageVector by lazy {
        strokeIcon(
            "VelaDownload",
            "M12 15V3",
            "M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4",
            "m7 10 5 5 5-5",
        )
    }

    /** lucide upload — 导出通讯录 / 导出全部通讯录 / 导出本组. */
    val Upload: ImageVector by lazy {
        strokeIcon(
            "VelaUpload",
            "M12 3v12",
            "m17 8-5-5-5 5",
            "M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4",
        )
    }

    /** lucide pencil — C2 header edit · 编辑分组 / 重命名分组 / 编辑. */
    val Pencil: ImageVector by lazy {
        strokeIcon(
            "VelaPencil",
            "M21.174 6.812a1 1 0 0 0-3.986-3.987L3.842 16.174a2 2 0 0 0-.5.83l-1.321 4.352a.5.5 " +
                "0 0 0 .623.622l4.353-1.32a2 2 0 0 0 .83-.497z",
            "m15 5 4 4",
        )
    }

    /** lucide trash-2 — destructive delete rows. */
    val Trash2: ImageVector by lazy {
        strokeIcon(
            "VelaTrash2",
            "M10 11v6",
            "M14 11v6",
            "M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6",
            "M3 6h18",
            "M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2",
        )
    }

    /** lucide ellipsis — page-header ⋯ button. */
    val Ellipsis: ImageVector by lazy {
        strokeIcon(
            "VelaEllipsis",
            // circle cx=12 cy=12 r=1
            "M11 12a1 1 0 1 0 2 0a1 1 0 1 0-2 0",
            // circle cx=19 cy=12 r=1
            "M18 12a1 1 0 1 0 2 0a1 1 0 1 0-2 0",
            // circle cx=5 cy=12 r=1
            "M4 12a1 1 0 1 0 2 0a1 1 0 1 0-2 0",
        )
    }

    /** lucide qr-code — 二维码 contact action. */
    val QrCode: ImageVector by lazy {
        strokeIcon(
            "VelaQrCode",
            // rect x=3 y=3 w=5 h=5 rx=1
            "M4 3h3a1 1 0 0 1 1 1v3a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1z",
            // rect x=16 y=3 w=5 h=5 rx=1
            "M17 3h3a1 1 0 0 1 1 1v3a1 1 0 0 1-1 1h-3a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1z",
            // rect x=3 y=16 w=5 h=5 rx=1
            "M4 16h3a1 1 0 0 1 1 1v3a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1v-3a1 1 0 0 1 1-1z",
            "M21 16h-3a2 2 0 0 0-2 2v3",
            "M21 21v.01",
            "M12 7v3a2 2 0 0 1-2 2H7",
            "M3 12h.01",
            "M12 3h.01",
            "M12 16v.01",
            "M16 12h1",
            "M21 12v.01",
            "M12 21v-1",
        )
    }

    /** lucide plus — 添加成员 ghost row · + 分组 chip. */
    val Plus: ImageVector by lazy {
        strokeIcon("VelaPlus", "M5 12h14", "M12 5v14")
    }

    // --- spec 021 additions (docs/design/wallet-2) ----------------------------
    /**
     * The single-person glyph. `UsersRound` is the group; SD2's recipient
     * field opens a picker for exactly one person, and two heads there read
     * as "add several".
     */
    val UserRound: ImageVector by lazy {
        strokeIcon("UserRound", "M12 3a5 5 0 1 1 0 10 5 5 0 0 1 0-10z", "M20 21a8 8 0 0 0-16 0")
    }

    /** SD2's denomination toggle: token or display currency. */
    val ChevronsUpDown: ImageVector by lazy {
        strokeIcon("ChevronsUpDown", "m7 15 5 5 5-5", "m7 9 5-5 5 5")
    }

    /** T4's empty-assets mark. */
    val CreditCard: ImageVector by lazy {
        strokeIcon(
            "CreditCard",
            "M4 5h16a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2z",
            "M2 10h20",
        )
    }

    /** SD2c's "import a file". */
    val FileText: ImageVector by lazy {
        strokeIcon(
            "FileText",
            "M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z",
            "M14 2v4a2 2 0 0 0 2 2h4",
            "M10 9H8",
            "M16 13H8",
            "M16 17H8",
        )
    }

    /** S1's photo-library tool. */
    val Image: ImageVector by lazy {
        strokeIcon(
            "Image",
            "M5 3h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z",
            "M9 7a2 2 0 1 1 0 4 2 2 0 0 1 0-4z",
            "m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21",
        )
    }

    /** S1's torch. */
    val Zap: ImageVector by lazy {
        strokeIcon(
            "Zap",
            "M4 14a1 1 0 0 1-.78-1.63l9.9-10.2a.5.5 0 0 1 .86.46l-1.92 6.02A1 1 0 0 0 13 10h7a1 1 0 0 1 .78 1.63l-9.9 10.2a.5.5 0 0 1-.86-.46l1.92-6.02A1 1 0 0 0 11 14z",
        )
    }

    /** S1's camera flip. */
    val RotateCcw: ImageVector by lazy {
        strokeIcon("RotateCcw", "M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8", "M3 3v5h5")
    }

    /** lucide chevron-left — mobile back affordance (C2/C4). */
    val ChevronLeft: ImageVector by lazy {
        strokeIcon("VelaChevronLeft", "m15 18-6-6 6-6")
    }

    // -- spec 023: the settings rows' leading glyphs and their chrome --------

    /** lucide globe — 语言 row. */
    val Globe: ImageVector by lazy {
        strokeIcon(
            "VelaGlobe",
            "M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20z",
            "M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20",
            "M2 12h20",
        )
    }

    /** lucide sun — 浅色 segment · 外观 nav. */
    val Sun: ImageVector by lazy {
        strokeIcon(
            "VelaSun",
            "M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8z",
            "M12 2v2",
            "M12 20v2",
            "m4.93 4.93 1.41 1.41",
            "m17.66 17.66 1.41 1.41",
            "M2 12h2",
            "M20 12h2",
            "m6.34 17.66-1.41 1.41",
            "m19.07 4.93-1.41 1.41",
        )
    }

    /** lucide moon — 深色 segment. */
    val Moon: ImageVector by lazy {
        strokeIcon(
            "VelaMoon",
            "M20.985 12.486a9 9 0 1 1-9.473-9.472c.405-.022.617.46.402.803a6 6 0 0 0 8.268 8.268c.344-.215.825-.004.803.401",
        )
    }

    /** lucide monitor — 跟随系统 segment. */
    val Monitor: ImageVector by lazy {
        strokeIcon(
            "VelaMonitor",
            "M4 3h16a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z",
            "M8 21h8",
            "M12 17v4",
        )
    }

    /** lucide coins — 货币 row · 区域格式 nav. */
    val Coins: ImageVector by lazy {
        strokeIcon(
            "VelaCoins",
            "M13.744 17.736a6 6 0 1 1-7.48-7.48",
            "M15 6h1v4",
            "m6.134 14.768.866-.5 2 3.464",
            "M16 2a6 6 0 1 0 0 12 6 6 0 0 0 0-12z",
        )
    }

    /** lucide hash — 数字格式 row. */
    val Hash: ImageVector by lazy {
        strokeIcon("VelaHash", "M4 9h16", "M4 15h16", "M10 3 8 21", "M16 3l-2 18")
    }

    /** lucide calendar — 日期格式 row. */
    val Calendar: ImageVector by lazy {
        strokeIcon(
            "VelaCalendar",
            "M8 2v4",
            "M16 2v4",
            "M5 4h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2z",
            "M3 10h18",
        )
    }

    /** lucide network — 网络 row. */
    val Network: ImageVector by lazy {
        strokeIcon(
            "VelaNetwork",
            "M17 16h4a1 1 0 0 1 1 1v4a1 1 0 0 1-1 1h-4a1 1 0 0 1-1-1v-4a1 1 0 0 1 1-1z",
            "M3 16h4a1 1 0 0 1 1 1v4a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1v-4a1 1 0 0 1 1-1z",
            "M10 2h4a1 1 0 0 1 1 1v4a1 1 0 0 1-1 1h-4a1 1 0 0 1-1-1V3a1 1 0 0 1 1-1z",
            "M5 16v-3a1 1 0 0 1 1-1h12a1 1 0 0 1 1 1v3",
            "M12 12V8",
        )
    }

    /** lucide server — RPC 供应商 row. */
    val Server: ImageVector by lazy {
        strokeIcon(
            "VelaServer",
            "M4 2h16a2 2 0 0 1 2 2v4a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2z",
            "M4 14h16a2 2 0 0 1 2 2v4a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2v-4a2 2 0 0 1 2-2z",
            "M6 6h.01",
            "M6 18h.01",
        )
    }

    /** lucide hard-drive — 设备存储 row. */
    val HardDrive: ImageVector by lazy {
        strokeIcon(
            "VelaHardDrive",
            "M10 16h.01",
            "M2.212 11.577a2 2 0 0 0-.212.896V18a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-5.527a2 2 0 0 0-.212-.896L18.55 5.11A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z",
            "M21.946 12.013H2.054",
            "M6 16h.01",
        )
    }

    /** lucide info — 关于 row. */
    val Info: ImageVector by lazy {
        strokeIcon(
            "VelaInfo",
            "M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20z",
            "M12 16v-4",
            "M12 8h.01",
        )
    }

    /** lucide log-out — 退出登录. */
    val LogOut: ImageVector by lazy {
        strokeIcon(
            "VelaLogOut",
            "m16 17 5-5-5-5",
            "M21 12H9",
            "M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4",
        )
    }

    /** lucide message-square-text — 反馈 row. */
    val MessageSquareText: ImageVector by lazy {
        strokeIcon(
            "VelaMessageSquareText",
            "M22 17a2 2 0 0 1-2 2H6.828a2 2 0 0 0-1.414.586l-2.202 2.202A.71.71 0 0 1 2 21.286V5a2 2 0 0 1 2-2h16a2 2 0 0 1 2 2z",
            "M7 11h10",
            "M7 15h6",
            "M7 7h8",
        )
    }

    /** lucide external-link — the 反馈 row's trailing mark and About's links. */
    // --- spec 022 additions (design/explore + the signing sheet) ----------
    // Spec 022's Android screens were written against these eight names and the
    // registry never got them, so `explore` and `signing` did not compile. Path
    // data is the web's `icons.ts`, which is where spec 022 authored the same
    // glyphs — one vocabulary, four clients. `Star` is a computed five-point
    // path, not a remembered lucide one: a mis-recalled star draws a shape
    // nobody can name.

    /** lucide arrow-right — the browser's Forward. */
    val ArrowRight: ImageVector by lazy {
        strokeIcon("VelaArrowRight", "M5 12h14", "m12 5 7 7-7 7")
    }

    /** lucide arrow-down — the signing sheet's "what comes back" leg. */
    val ArrowDown: ImageVector by lazy {
        strokeIcon("VelaArrowDown", "M12 5v14", "m19 12-7 7-7-7")
    }

    /** The site menu's add-to-favourites row. */
    val Star: ImageVector by lazy {
        strokeIcon(
            "VelaStar",
            "M12.00 2.70 L14.35 8.76 L20.84 9.13 L15.80 13.24 L17.47 19.52 " +
                "L12.00 16.00 L6.53 19.52 L8.20 13.24 L3.16 9.13 L9.65 8.76 Z",
        )
    }

    /** The same star filled (the web's `star-filled`): the page in front already is a favourite. */
    val StarFilled: ImageVector by lazy {
        fillIcon(
            "VelaStarFilled",
            "M12.00 2.70 L14.35 8.76 L20.84 9.13 L15.80 13.24 L17.47 19.52 " +
                "L12.00 16.00 L6.53 19.52 L8.20 13.24 L3.16 9.13 L9.65 8.76 Z",
        )
    }

    /** lucide lock — the address pill's https mark and the site sheets'. */
    val Lock: ImageVector by lazy {
        strokeIcon(
            "VelaLock",
            // rect x=3 y=11 w=18 h=11 rx=2
            "M5 11h14a2 2 0 0 1 2 2v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-7a2 2 0 0 1 2-2z",
            "M7 11V7a5 5 0 0 1 10 0v4",
        )
    }

    /**
     * lucide `shield-check` — spec 102: a signing page whose build was checked
     * (the integrity line's "matches", and every trusted page's row).
     */
    val ShieldCheck: ImageVector by lazy {
        strokeIcon(
            "VelaShieldCheck",
            "M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z",
            "m9 12 2 2 4-4",
        )
    }

    /** lucide `lock-open` — a page NOT on https (spec 079: the lock alone says it, no words). */
    val LockOpen: ImageVector by lazy {
        strokeIcon(
            "VelaLockOpen",
            // rect x=3 y=11 w=18 h=11 rx=2
            "M5 11h14a2 2 0 0 1 2 2v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-7a2 2 0 0 1 2-2z",
            "M7 11V7a5 5 0 0 1 9.9-1",
        )
    }

    /** lucide share-2 — the site menu's share row. */
    val Share2: ImageVector by lazy {
        strokeIcon(
            "VelaShare2",
            // circles cx/cy 18,5 · 6,12 · 18,19, r=3
            "M18 2a3 3 0 1 1 0 6 3 3 0 0 1 0-6z",
            "M6 9a3 3 0 1 1 0 6 3 3 0 0 1 0-6z",
            "M18 16a3 3 0 1 1 0 6 3 3 0 0 1 0-6z",
            "M8.59 13.51 15.42 17.49",
            "M15.41 6.51 8.59 10.49",
        )
    }

    /** lucide power — the site menu's disconnect row. */
    val Power: ImageVector by lazy {
        strokeIcon("VelaPower", "M12 2v10", "M18.4 6.6a9 9 0 1 1-12.77.04")
    }

    /** lucide external-link — open in the system browser, and the explorer link. */
    val ExternalLink: ImageVector by lazy {
        strokeIcon(
            "VelaExternalLink",
            "M15 3h6v6",
            "M10 14 21 3",
            "M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6",
        )
    }
}
