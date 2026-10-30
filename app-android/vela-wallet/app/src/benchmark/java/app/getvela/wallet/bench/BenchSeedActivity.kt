package app.getvela.wallet.bench

import android.app.Activity
import android.os.Bundle
import android.widget.TextView
import app.getvela.wallet.core.data.VelaStore
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * Native parity Phase 4, benchmark build types only: puts the fixture wallet
 * ([BenchFixture]) in the store, then says so on screen — the journey waits
 * for [DONE] and kills the process, so the measured launch is a cold one that
 * reads the store as a returning person's would.
 *
 *   adb shell am start -n app.getvela.wallet/app.getvela.wallet.bench.BenchSeedActivity --ei vela.bench.rows 200
 *
 * A plain [Activity], not Compose: nothing here may warm the code the
 * measured launch is about.
 */
class BenchSeedActivity : Activity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val label = TextView(this).apply { text = "seeding" }
        setContentView(label)
        val rows = intent?.getIntExtra(EXTRA_ROWS, BenchFixture.DEFAULT_ROWS) ?: BenchFixture.DEFAULT_ROWS
        CoroutineScope(Dispatchers.IO).launch {
            val written = BenchFixture.write(VelaStore(applicationContext), rows, System.currentTimeMillis())
            withContext(Dispatchers.Main) { label.text = if (written) DONE else FAILED }
        }
    }

    companion object {
        const val EXTRA_ROWS = "vela.bench.rows"
        const val DONE = "vela-bench-seeded"
        const val FAILED = "vela-bench-seed-failed"
    }
}
