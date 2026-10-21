package app.getvela.wallet.feature.agent

import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import app.getvela.wallet.core.data.KeyValueStore
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * The AI agent's client token's one place (spec 104) — not with the other
 * `vela.*` values in the app's store, where it would sit in the clear. The
 * token lets whoever holds it connect as this wallet.
 */
interface AgentTokenVault {
    /** The token kept, or `null`. */
    suspend fun read(): String?

    /** Keep [token], replacing what was kept. `false` when the platform refused. */
    suspend fun write(token: String): Boolean

    /** Forget it — on an empty token, and on Erase This Device. */
    suspend fun delete()
}

/**
 * The Android Keystore's: an AES-GCM key generated inside the Keystore, which
 * never leaves it, seals the token; only the sealed bytes are stored
 * (`vela.agentToken`, swept with the rest on an erase, which deletes the key
 * too).
 */
class KeystoreTokenVault(private val store: KeyValueStore) : AgentTokenVault {

    override suspend fun read(): String? = runCatching {
        val text = store.read(KEY) ?: return null
        val (iv, sealed) = text.split(':').map { Base64.decode(it, Base64.NO_WRAP) }
        val cipher = Cipher.getInstance(CIPHER)
        cipher.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(TAG_BITS, iv))
        String(cipher.doFinal(sealed), Charsets.UTF_8)
    }.getOrNull()

    override suspend fun write(token: String): Boolean = runCatching {
        val cipher = Cipher.getInstance(CIPHER)
        cipher.init(Cipher.ENCRYPT_MODE, key())
        val sealed = cipher.doFinal(token.toByteArray(Charsets.UTF_8))
        store.write(KEY, Base64.encodeToString(cipher.iv, Base64.NO_WRAP) + ":" + Base64.encodeToString(sealed, Base64.NO_WRAP))
    }.getOrDefault(false)

    override suspend fun delete() {
        store.remove(KEY)
        runCatching { keyStore().deleteEntry(ALIAS) }
    }

    private fun keyStore(): KeyStore = KeyStore.getInstance(PROVIDER).apply { load(null) }

    private fun key(): SecretKey {
        (keyStore().getKey(ALIAS, null) as? SecretKey)?.let { return it }
        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, PROVIDER)
        generator.init(
            KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .build(),
        )
        return generator.generateKey()
    }

    companion object {
        /** The sealed token: `base64(iv):base64(ciphertext)`. */
        const val KEY = "vela.agentToken"
        private const val ALIAS = "vela.agent.token"
        private const val PROVIDER = "AndroidKeyStore"
        private const val CIPHER = "AES/GCM/NoPadding"
        private const val TAG_BITS = 128
    }
}

/** A vault in memory — for tests. */
class MemoryTokenVault(private var token: String? = null) : AgentTokenVault {
    override suspend fun read(): String? = token
    override suspend fun write(token: String): Boolean = true.also { this.token = token }
    override suspend fun delete() {
        token = null
    }
}
