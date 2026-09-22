package app.getvela.wallet.feature.settings.core

import app.getvela.wallet.core.data.KeyValueStore

/**
 * The only place the `sign_pref` core touches the outside world (spec 071):
 * two keys in the store `vela.feeTier` lives in, and for the same reason they
 * survive sign-out — how a person signs belongs to them and the device.
 */
class SignPrefExecutor(private val store: KeyValueStore) {

    suspend fun perform(operation: SignPrefOperation): SignPrefShellResult = when (operation) {
        is SignPrefOperation.ReadStored -> SignPrefShellResult.Stored(
            method = store.read(KeyValueStore.Keys.SIGN_METHOD),
            signer_url = store.read(KeyValueStore.Keys.CLEAR_SIGNER_URL),
            relay_url = store.read(KeyValueStore.Keys.CLEAR_SIGNER_RELAY),
        )

        // Best effort: the committed choice stays on screen either way.
        is SignPrefOperation.WriteMethod -> {
            store.write(KeyValueStore.Keys.SIGN_METHOD, operation.method)
            SignPrefShellResult.Written
        }

        is SignPrefOperation.WriteSignerUrl -> {
            val url = operation.url
            if (url == null) store.remove(KeyValueStore.Keys.CLEAR_SIGNER_URL)
            else store.write(KeyValueStore.Keys.CLEAR_SIGNER_URL, url)
            SignPrefShellResult.Written
        }

        // Spec 075: the relay, the same way — a removed key is the official one.
        is SignPrefOperation.WriteRelayUrl -> {
            val url = operation.url
            if (url == null) store.remove(KeyValueStore.Keys.CLEAR_SIGNER_RELAY)
            else store.write(KeyValueStore.Keys.CLEAR_SIGNER_RELAY, url)
            SignPrefShellResult.Written
        }
    }

    fun neutralAnswer(operation: SignPrefOperation): SignPrefShellResult = when (operation) {
        is SignPrefOperation.ReadStored -> SignPrefShellResult.Stored()
        else -> SignPrefShellResult.Written
    }
}
