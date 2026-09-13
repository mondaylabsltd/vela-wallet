package app.getvela.wallet.feature.wallet.components

import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.Dp
import app.getvela.wallet.core.identicon.IdenticonImage

/**
 * Wallet-identity avatar: the Nimiq identicon for a seed, circular crop
 * (spec vocabulary #1) — or, when the person chose 首字母 (spec 049, the
 * web's `avatarSvgForClient`), a letter of `name` on an accent disc. Spec
 * 015's "no initial-letter rendering" ruling predates the preference; the
 * preference wins, and the identicon stays the default.
 */
@Composable
fun IdenticonAvatar(
    seed: String,
    modifier: Modifier = Modifier,
    size: Dp = WalletMetrics.avatarSize,
    contentDescription: String? = null,
    tappable: Boolean = true,
    /** The name shown beside the artwork; the initials style takes its first letter. */
    name: String? = null,
) {
    IdenticonImage(
        seed = seed,
        size = size,
        modifier = modifier,
        contentDescription = contentDescription,
        tappable = tappable,
        name = name,
    )
}
