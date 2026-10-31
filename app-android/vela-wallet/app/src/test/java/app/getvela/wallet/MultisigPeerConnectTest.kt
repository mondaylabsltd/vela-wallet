package app.getvela.wallet

import app.getvela.wallet.feature.multisig.core.MultisigPeerConnect
import app.getvela.wallet.feature.multisig.core.PeerArrival
import app.getvela.wallet.feature.multisig.core.PeerNet
import app.getvela.wallet.feature.multisig.core.PeerSocket
import app.getvela.wallet.feature.multisig.core.PeerSocketListener
import app.getvela.wallet.feature.multisig.core.get
import app.getvela.wallet.feature.multisig.core.jo
import app.getvela.wallet.feature.multisig.core.kind
import app.getvela.wallet.feature.multisig.core.str
import java.util.concurrent.CopyOnWriteArrayList
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * M4: the PeerConnect rooms behind the multisig machines' `peer_*`
 * operations — PeerConnect's own peer (uniffi) over a socket this suite
 * hands in. [MemoryRelay] keeps the relay's side of SPEC §5 (HELLO, JOIN
 * seats, PRESENCE, forwarding peer frames, `ping`/`pong`), so two Vela
 * transports pair through it, Noise and all, exactly as through
 * `relay.peerconnect.dev` — and the public relay's token is asked for as
 * the core's `relay.*` rules say.
 */
class MultisigPeerConnectTest {

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    @After
    fun tearDown() = scope.cancel()

    /** A relay in memory: seats per room, frames forwarded byte for byte. */
    private class MemoryRelay : PeerNet {
        val urls = CopyOnWriteArrayList<String>()
        val posts = CopyOnWriteArrayList<Pair<String, String?>>()
        var answer: (String) -> Pair<Int?, String> = { null to "unreachable" }
        private val seats = mutableMapOf<String, MutableMap<Int, Conn>>()

        inner class Conn(val room: String, val listener: PeerSocketListener) : PeerSocket {
            var role: Int? = null
            var open = true

            override fun send(bytes: ByteArray) = synchronized(this@MemoryRelay) {
                if (!open) return@synchronized
                val type = bytes[0].toInt() and 0xff
                if (type == 0x81) {
                    val role = bytes[1].toInt()
                    this.role = role
                    val room = seats.getOrPut(room) { mutableMapOf() }
                    room.put(role, this)?.takeIf { it !== this }?.let { it.open = false; it.listener.closed(4001) }
                    val other = room[1 - role]
                    listener.binary(byteArrayOf(0x82.toByte(), if (other != null) 1 else 0))
                    other?.listener?.binary(byteArrayOf(0x82.toByte(), 1))
                } else if (type < 0x80) {
                    val r = role ?: return@synchronized
                    seats[room]?.get(1 - r)?.listener?.binary(bytes)
                }
            }

            override fun sendText(text: String) {
                if (text == "ping") listener.text("pong")
            }

            override fun close() = synchronized(this@MemoryRelay) {
                if (!open) return@synchronized
                open = false
                val r = role ?: return@synchronized
                val room = seats[room] ?: return@synchronized
                if (room[r] === this) {
                    room.remove(r)
                    room[1 - r]?.listener?.binary(byteArrayOf(0x82.toByte(), 0))
                }
            }
        }

        override fun connect(url: String, listener: PeerSocketListener): PeerSocket {
            urls += url
            val room = url.substringBefore('?')
            val conn = Conn(room, listener)
            Thread {
                listener.opened()
                listener.binary(byteArrayOf(0x80.toByte()) + ByteArray(32) { it.toByte() })
            }.start()
            return conn
        }

        override suspend fun post(url: String, body: String?): Pair<Int?, String> {
            posts += url to body
            return answer(url)
        }
    }

    private class Told {
        val events = CopyOnWriteArrayList<JsonObject>()
        fun of(type: String) = events.filter { it["type"].str == type }
    }

    private suspend fun until(what: String, ms: Long = 10_000, check: () -> Boolean) {
        withTimeout(ms) {
            while (!check()) delay(20)
        }
        assertTrue(what, check())
    }

    @Test
    fun twoTransportsPairThroughTheRelayAndCarryJson() = runBlocking {
        val relay = MemoryRelay()
        val vela = MultisigPeerConnect(scope, FakeStore(), relay)
        val wallet = MultisigPeerConnect(scope, FakeStore(), relay)
        val velaTold = Told().also { t -> vela.tell = { t.events += it } }
        val walletTold = Told().also { t -> wallet.tell = { t.events += it } }

        val created = vela.open("ws://relay.test", null, null)
        assertEquals("room-1", created.room)
        val invite = created.invite!!
        val joined = wallet.open(null, invite, null)
        assertNull("a joined room hands out no invite", joined.invite)

        until("both sides connected") {
            velaTold.of("peer_state").any { it["state"].str == "connected" } &&
                walletTold.of("peer_state").any { it["state"].str == "connected" }
        }
        until("pairing told with the record that reopens the room") { velaTold.of("paired").isNotEmpty() }
        assertEquals("room-1", velaTold.of("paired").first()["room"].str)
        assertTrue(velaTold.of("paired").first()["pairing"].str!!.isNotEmpty())
        // On the relay: said up, as the core reads the state.
        assertTrue(velaTold.of("relay").any { it["status"].kind == "up" })
        // A local relay asks no token.
        assertTrue(relay.posts.isEmpty())
        assertTrue(relay.urls.all { it.startsWith("ws://relay.test/v1/") })

        vela.send(created.room, jo("method" to "walletpair_connect", "id" to 1))
        val arrived = withTimeout(10_000) { wallet.receive(joined.room) }
        assertEquals("walletpair_connect", ((arrived as PeerArrival.Message).message)["method"].str)
        wallet.send(joined.room, JsonPrimitive("plain words"))
        val back = withTimeout(10_000) { vela.receive(created.room) }
        assertEquals(JsonPrimitive("plain words"), (back as PeerArrival.Message).message)

        // Ending the room ends it for both; a receive then says how.
        vela.close(created.room, cancel = false, end = true)
        assertEquals(PeerArrival.Closed("closed"), vela.receive(created.room))
        val ended = withTimeout(10_000) { wallet.receive(joined.room) }
        assertEquals(PeerArrival.Closed("closed-by-peer"), ended)
    }

    @Test
    fun aKeptPairingReopensWithoutAnInviteAndLeavingKeepsIt() = runBlocking {
        val relay = MemoryRelay()
        val vela = MultisigPeerConnect(scope, FakeStore(), relay)
        val created = vela.open("ws://relay.test", null, null)
        val again = vela.open(null, null, created.pairing)
        assertNull(again.invite)
        assertEquals(created.pairing, again.pairing)
        vela.leaveAll()
        assertEquals(PeerArrival.Closed("closed"), vela.receive(created.room))
        // Unknown rooms read as closed, never hang.
        assertEquals(PeerArrival.Closed("closed"), vela.receive("room-99"))
    }

    @Test
    fun thePublicRelayWaitsForASiteTokenThenConnects() = runBlocking {
        val relay = MemoryRelay()
        relay.answer = { url ->
            if (url.contains("getvela.app")) 200 to """{"token":"site-token","expires_at":${System.currentTimeMillis() / 1000 + 86_400}}"""
            else 500 to "no"
        }
        val store = FakeStore()
        val vela = MultisigPeerConnect(scope, store, relay)
        vela.open(null, null, null)
        until("connected after the token") { relay.urls.isNotEmpty() }
        assertTrue(relay.urls.first().startsWith(MultisigPeerConnect.DEFAULT_RELAY + "/v1/"))
        val (url, body) = relay.posts.first()
        assertEquals(MultisigPeerConnect.TOKEN_ENDPOINT, url)
        assertTrue("asked under this device's ID", body!!.contains("\"device\""))
        assertNotNull(store.values[MultisigPeerConnect.DEVICE_KEY])
        val kept = store.values[MultisigPeerConnect.TOKEN_KEY]!!
        assertTrue(kept, kept.contains("site-token") && kept.contains("\"source\":\"site\""))

        // A second room uses the kept token: no new request.
        vela.open(null, null, null)
        until("second room connects") { relay.urls.size >= 2 }
        assertEquals(1, relay.posts.size)
    }

    @Test
    fun noSiteTokenFallsBackToTheTrialsAndNoneIsSaidHonestly() = runBlocking {
        val relay = MemoryRelay()
        relay.answer = { url ->
            if (url.contains("trial")) 200 to """{"token":"trial-token","expires_at":${System.currentTimeMillis() / 1000 + 3_600}}"""
            else 429 to "slow down"
        }
        val store = FakeStore()
        val vela = MultisigPeerConnect(scope, store, relay)
        vela.open(null, null, null)
        until("connected on the trial's token") { relay.urls.isNotEmpty() }
        assertTrue(store.values[MultisigPeerConnect.TOKEN_KEY]!!.contains("\"source\":\"trial\""))

        // Nowhere answers: the relay status says why, retrying.
        val none = MemoryRelay()
        val told = Told()
        val lonely = MultisigPeerConnect(scope, FakeStore(), none).also { t -> t.tell = { told.events += it } }
        lonely.open(null, null, null)
        until("said retrying for want of a token") {
            told.of("relay").any { it["status"].kind == "retrying" && it["status"]["why"].str == "noToken" }
        }
        assertTrue(none.urls.isEmpty())
        lonely.leaveAll()
    }
}
