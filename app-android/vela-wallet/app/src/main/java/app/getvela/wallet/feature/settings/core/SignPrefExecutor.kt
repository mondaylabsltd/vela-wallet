package app.getvela.wallet.feature.settings.core

import app.getvela.wallet.core.data.KeyValueStore

/**
 * The only place the `sign_pref` core touches the outside world (spec 071):
 * one key in the store `vela.feeTier` lives in, and for the same reason it
 * survives sign-out — which page a person trusts belongs to them and the device.
 */
class SignPrefExecutor(private val store: KeyValueStore) {

    suspend fun perform(operation: SignPrefOperation): SignPrefShellResult = when (operation) {
        is SignPrefOperation.ReadStored -> SignPrefShellResult.Stored(
            signer_url = store.read(KeyValueStore.Keys.TRUSTED_SIGNER_URL),
        )

        is SignPrefOperation.WriteSignerUrl -> {
            val url = operation.url
            if (url == null) store.remove(KeyValueStore.Keys.TRUSTED_SIGNER_URL)
            else store.write(KeyValueStore.Keys.TRUSTED_SIGNER_URL, url)
            SignPrefShellResult.Written
        }
    }

    fun neutralAnswer(operation: SignPrefOperation): SignPrefShellResult = when (operation) {
        is SignPrefOperation.ReadStored -> SignPrefShellResult.Stored()
        is SignPrefOperation.WriteSignerUrl -> SignPrefShellResult.Written
    }
}
