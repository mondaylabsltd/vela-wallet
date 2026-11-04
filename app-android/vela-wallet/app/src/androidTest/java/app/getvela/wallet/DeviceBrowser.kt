package app.getvela.wallet

import android.app.UiAutomation
import android.os.ParcelFileDescriptor
import androidx.test.platform.app.InstrumentationRegistry

/**
 * The device's browser, made ready for a test that opens a real Custom Tab —
 * and put away afterwards.
 *
 * A browser on a fresh emulator (CI's, every night; anybody's new AVD) has
 * never been opened, so the first tab opens Chrome's **first-run screen**
 * instead of the page: the page is never fetched, the probe waits out its
 * 45 s, and the first-run activity stays on top of every test that runs after
 * it. One un-onboarded Chrome failed 45 Compose tests that had nothing to do
 * with it (`signing-confirm`, `signing-sheet` … "could not find any node"):
 * their activity was behind Chrome's.
 *
 * [prepare] gives Chrome the switches every browser-driving harness uses to
 * skip that screen (read at process start from the command-line file, which a
 * userdebug image or a debug app honours) and stops it so the next start
 * reads them. [putAway] stops it again — whatever the test concluded — so no
 * tab outlives the test that opened it, and takes the switches back.
 *
 * Shell commands go through [UiAutomation], which runs them as the shell user:
 * nothing here needs root.
 */
object DeviceBrowser {
    const val CHROME = "com.android.chrome"
    private const val COMMAND_LINE = "/data/local/tmp/chrome-command-line"

    private val automation: UiAutomation get() = InstrumentationRegistry.getInstrumentation().uiAutomation

    private fun installed(pkg: String): Boolean = runCatching {
        InstrumentationRegistry.getInstrumentation().targetContext.packageManager.getPackageInfo(pkg, 0)
    }.isSuccess

    /** Runs [command] as the shell user and waits for it to finish; answers its output. */
    fun shell(command: String): String =
        ParcelFileDescriptor.AutoCloseInputStream(automation.executeShellCommand(command)).use {
            it.readBytes().toString(Charsets.UTF_8)
        }

    /** Chrome, ready to open a tab on its page rather than on its first-run screen. */
    fun prepare() {
        if (!installed(CHROME)) return
        // The first word is the program name Chrome skips over.
        val switches = "_ --disable-fre --no-default-browser-check --no-first-run\n"
        val (read, write) = automation.executeShellCommandRw("tee $COMMAND_LINE")
        ParcelFileDescriptor.AutoCloseOutputStream(write).use { it.write(switches.toByteArray()) }
        ParcelFileDescriptor.AutoCloseInputStream(read).use { it.readBytes() }
        // A release Chrome reads that file only as the debug app (a userdebug
        // image's does anyway). Not persistent: a reboot forgets it.
        shell("am set-debug-app $CHROME")
        shell("am force-stop $CHROME")
    }

    /** No tab left over the next test, and the device's browser as it was. */
    fun putAway() {
        if (!installed(CHROME)) return
        shell("am force-stop $CHROME")
        shell("am clear-debug-app")
        shell("rm -f $COMMAND_LINE")
    }
}
