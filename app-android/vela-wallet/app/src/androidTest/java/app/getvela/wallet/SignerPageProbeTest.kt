package app.getvela.wallet

import android.webkit.JavascriptInterface
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.io.BufferedReader
import java.io.InputStreamReader
import java.net.InetAddress
import java.net.ServerSocket
import java.util.Collections
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference

/**
 * Spec 076, probe P1 — **a measurement, not a feature**.
 *
 * The design of 076 FR-004 rests on one assumption that nobody has checked on
 * this engine: that a page loaded by a real NAVIGATION can read back the very
 * bytes that navigation received, with
 *
 * ```js
 * fetch(location.href, { cache: 'only-if-cached', mode: 'same-origin' })
 * ```
 *
 * If that does not work in Android's WebView, the bytes can only be read by
 * intercepting the response — which costs the TLS/HTTP fingerprint that makes
 * the check indistinguishable from a real visit, and the whole "keep the
 * browser's fingerprint" position has to be re-argued (plan.md, risk 1).
 *
 * So this answers it with numbers, on the real engine:
 *
 * 1. does `only-if-cached` return the navigation's body, and is it the right
 *    length;
 * 2. **how many times the server was asked** — the point of `only-if-cached`
 *    is that it issues no second request, and only the server can say so;
 * 3. what happens under `Cache-Control: no-store`, which is exactly what a
 *    server defeating this check would send (FR-006 / SC-002);
 * 4. whether `crypto.subtle` is even available — it needs a secure context,
 *    and http on loopback is supposed to count as one.
 *
 * The server is in this process, so nothing depends on `adb reverse` or on the
 * host's networking, and the request count is ground truth rather than an
 * API's opinion.
 */
@RunWith(AndroidJUnit4::class)
class SignerPageProbeTest {

    /** A page big enough that a truncated read is obvious in the number. */
    private val body = buildString {
        append("<!doctype html><html><head><meta charset=\"utf-8\"><title>probe</title></head>")
        append("<body><p>")
        repeat(500) { append("vela clear signing probe ") }
        append("</p></body></html>")
    }

    private class Probe(val port: Int, val hits: MutableList<String>, val socket: ServerSocket)

    /** A server that counts what it was asked for, and says how to cache it. */
    private fun serve(cacheControl: String): Probe {
        val socket = ServerSocket(0, 8, InetAddress.getByName("127.0.0.1"))
        val hits = Collections.synchronizedList(mutableListOf<String>())
        val bytes = body.toByteArray(Charsets.UTF_8)
        Thread {
            while (!socket.isClosed) {
                try {
                    val client = socket.accept()
                    val reader = BufferedReader(InputStreamReader(client.getInputStream()))
                    val line = reader.readLine() ?: ""
                    hits.add(line)
                    val head = buildString {
                        append("HTTP/1.1 200 OK\r\n")
                        append("Content-Type: text/html; charset=utf-8\r\n")
                        append("Content-Length: ${bytes.size}\r\n")
                        append("Cache-Control: $cacheControl\r\n")
                        append("Connection: close\r\n\r\n")
                    }
                    client.getOutputStream().write(head.toByteArray(Charsets.US_ASCII))
                    client.getOutputStream().write(bytes)
                    client.getOutputStream().flush()
                    client.close()
                } catch (_: Exception) {
                    // the socket was closed, or the client went away
                }
            }
        }.also { it.isDaemon = true }.start()
        return Probe(socket.localPort, hits, socket)
    }

    private class Bridge(val latch: CountDownLatch, val said: AtomicReference<String>) {
        @JavascriptInterface
        fun report(text: String) {
            said.set(text)
            latch.countDown()
        }
    }

    /**
     * Load the page by navigation, then ask it — from inside its own JS
     * context — to read itself back out of the cache.
     */
    private fun readBack(cacheControl: String): Pair<String, List<String>> {
        val probe = serve(cacheControl)
        val loaded = CountDownLatch(1)
        val answered = CountDownLatch(1)
        val said = AtomicReference("(nothing)")
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        var view: WebView? = null

        instrumentation.runOnMainSync {
            val webView = WebView(instrumentation.targetContext)
            view = webView
            webView.settings.javaScriptEnabled = true
            webView.addJavascriptInterface(Bridge(answered, said), "Probe")
            webView.webViewClient = object : WebViewClient() {
                override fun onPageFinished(v: WebView?, url: String?) = loaded.countDown()
            }
            webView.loadUrl("http://127.0.0.1:${probe.port}/page.html")
        }
        assertTrue("the page never finished loading", loaded.await(30, TimeUnit.SECONDS))

        // Exactly what FR-004 says the verifier would run.
        val js = """
            (async () => {
              try {
                const secure = String(window.isSecureContext) + '/' +
                  String(typeof crypto !== 'undefined' && !!crypto.subtle);
                const r = await fetch(location.href, { cache: 'only-if-cached', mode: 'same-origin' });
                const buf = await r.arrayBuffer();
                let digest = 'no-subtle';
                if (typeof crypto !== 'undefined' && crypto.subtle) {
                  const h = await crypto.subtle.digest('SHA-256', buf);
                  digest = [...new Uint8Array(h)].map(b => b.toString(16).padStart(2, '0')).join('').slice(0, 16);
                }
                Probe.report('OK status=' + r.status + ' bytes=' + buf.byteLength +
                  ' secure/subtle=' + secure + ' sha256=' + digest);
              } catch (e) {
                Probe.report('THREW ' + (e && e.name) + ': ' + (e && e.message));
              }
            })()
        """.trimIndent()
        instrumentation.runOnMainSync { view?.evaluateJavascript(js, null) }
        assertTrue("the page never answered", answered.await(30, TimeUnit.SECONDS))

        instrumentation.runOnMainSync { view?.destroy() }
        // Give any second request time to arrive before counting.
        Thread.sleep(500)
        probe.socket.close()
        return said.get() to probe.hits.toList()
    }

    @Test
    fun only_if_cached_returns_the_navigations_own_bytes_without_asking_again() {
        val (answer, hits) = readBack("max-age=300")
        println("P1 · cacheable      → $answer")
        println("P1 · server was asked ${hits.size}×: $hits")

        assertTrue("expected the body back, got: $answer", answer.startsWith("OK"))
        val bytes = Regex("bytes=(\\d+)").find(answer)?.groupValues?.get(1)?.toInt() ?: -1
        assertEquals(
            "the read-back length must be the page's own length",
            body.toByteArray(Charsets.UTF_8).size,
            bytes
        )
        // The whole point: no second request for the DOCUMENT. Counted by path,
        // because a browser also asks for /favicon.ico on its own and that is
        // not the page being fetched twice. Anything above 1 here would mean a
        // server could tell a verification apart from a visit by counting.
        val forThePage = hits.filter { it.contains("/page.html") }
        assertEquals("only-if-cached must issue no second request: $forThePage", 1, forThePage.size)
    }

    @Test
    fun a_no_store_response_cannot_be_read_back_which_is_fail_closed() {
        // FR-006 / SC-002: a server that wants to defeat the check sends this.
        // Whatever the engine does here IS the behaviour 076 must fail closed on.
        val (answer, hits) = readBack("no-store")
        println("P1 · no-store       → $answer")
        println("P1 · server was asked ${hits.size}×: $hits")
        assertTrue("no-store produced neither a body nor an error: $answer", answer.isNotEmpty())
    }
}
