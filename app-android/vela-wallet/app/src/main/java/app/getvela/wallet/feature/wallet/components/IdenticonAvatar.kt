package app.getvela.wallet.feature.wallet.components

import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.Dp
import app.getvela.wallet.core.identicon.IdenticonImage

/**
 * Wallet-identity avatar: the Nimiq identicon for a seed, circular crop
 * (spec vocabulary #1). No initial-letter rendering anywhere (FR-006) — the
 * mocks' lettermark avatar is superseded by the identicon per the spec input.
 */
@Composable
fun IdenticonAvatar(
    seed: String,
    modifier: Modifier = Modifier,
    size: Dp = WalletMetrics.avatarSize,
    contentDescription: String? = null,
    tappable: Boolean = true,
) {
    IdenticonImage(
        seed = seed,
        size = size,
        modifier = modifier,
        contentDescription = contentDescription,
        tappable = tappable,
    )
}
