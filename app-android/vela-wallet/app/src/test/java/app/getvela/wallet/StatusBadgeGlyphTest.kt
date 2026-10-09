package app.getvela.wallet

import app.getvela.wallet.core.designsystem.components.BadgeVariant
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.components.badgeGlyph
import org.junit.Assert.assertNotSame
import org.junit.Assert.assertSame
import org.junit.Test

/**
 * Issue #460: the error badge drew the close glyph in a disc at the sheet's
 * top-left, where a close button goes — and it closed nothing. An outcome
 * says ! (only its colour tells error from warning); no badge wears ×.
 */
class StatusBadgeGlyphTest {
    @Test
    fun `the error badge says !, and no badge wears the close glyph`() {
        assertSame(VelaIcons.Exclamation, badgeGlyph(BadgeVariant.Error))
        for (variant in BadgeVariant.entries) {
            assertNotSame("$variant wears the close glyph", VelaIcons.Close, badgeGlyph(variant))
        }
        assertSame(VelaIcons.Check, badgeGlyph(BadgeVariant.Success))
        assertSame(VelaIcons.Clock, badgeGlyph(BadgeVariant.Timeout))
    }
}
