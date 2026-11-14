package app.getvela.wallet

import androidx.compose.animation.core.FastOutSlowInEasing
import app.getvela.wallet.core.designsystem.components.VelaLaunchTimeline
import app.getvela.wallet.core.designsystem.tokens.VelaLaunchScene
import java.io.File
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The launch animation's timeline, held to the shared samples
 * (`docs/design/onboarding/launch/launch-timeline.vectors.json`, generated with
 * the scene). The web's rendering of the same timeline was compared pixel for
 * pixel with the animation files it replaced; matching these samples is how the
 * Compose drawing inherits that check.
 */
class LaunchTimelineTest {
    private val vectors = JSONObject(
        File(
            System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle"),
            "docs/design/onboarding/launch/launch-timeline.vectors.json",
        ).readText(),
    )

    @Test
    fun `every sample, mark and letters`() {
        val cases = vectors.getJSONArray("cases")
        assertTrue("too few samples to mean anything: ${cases.length()}", cases.length() >= 30)
        for (i in 0 until cases.length()) {
            val case = cases.getJSONObject(i)
            val ms = case.getDouble("ms").toFloat()
            val state = VelaLaunchTimeline.state(ms)
            val mark = case.getJSONObject("mark")
            assertEquals("mark x @ $ms ms", mark.getDouble("x").toFloat(), state.markX, 0.05f)
            assertEquals("mark y @ $ms ms", mark.getDouble("y").toFloat(), state.markY, 0.05f)
            val letters = case.getJSONArray("letters")
            assertEquals("letter count @ $ms ms", letters.length(), state.letterAlpha.size)
            for (j in 0 until letters.length()) {
                assertEquals(
                    "letter $j (${VelaLaunchScene.letters[j].glyph}) @ $ms ms",
                    letters.getDouble(j).toFloat(),
                    state.letterAlpha[j],
                    0.005f,
                )
            }
        }
    }

    @Test
    fun `the samples' easing is the scene's, and Compose's FastOutSlowIn`() {
        val easing = vectors.getJSONArray("easing")
        for (i in 0 until 4) {
            assertEquals(easing.getDouble(i).toFloat(), VelaLaunchScene.easing[i], 0f)
        }
        for (step in 0..20) {
            val t = step / 20f
            assertEquals("t = $t", FastOutSlowInEasing.transform(t), VelaLaunchTimeline.easing.transform(t), 1e-6f)
        }
    }

    @Test
    fun `the authored length leaves the finished lockup still`() {
        val settled = VelaLaunchTimeline.state(VelaLaunchScene.settledMs)
        val end = VelaLaunchTimeline.state(VelaLaunchScene.durationMs)
        assertEquals(VelaLaunchScene.markToX, settled.markX, 0f)
        assertEquals(VelaLaunchScene.markToX, end.markX, 0f)
        assertTrue(settled.letterAlpha.all { it == 1f })
        assertTrue(end.letterAlpha.all { it == 1f })
        assertTrue(VelaLaunchScene.settledMs < VelaLaunchScene.durationMs)
    }
}
