/**
 * useDisplayCurrency — the chosen display currency + its USD→fiat rate, for any
 * screen that shows fiat values. Refreshes on focus so changing the currency on
 * one screen reflects everywhere. `fmt(usd)` converts a USD amount into the
 * selected currency, honouring the localized number format + decimal rules.
 */
import { useCallback, useState } from 'react';
import { useFocusEffect } from '@react-navigation/native';
import { currencyMeta, formatFiat, getCurrencyCode, getRate, loadCurrency } from '@/services/currency';

export interface DisplayCurrency {
  code: string;
  symbol: string;
  /** USD → display-currency multiplier. */
  rate: number;
  /** Format a USD value into the selected currency, e.g. 1.0 → "¥155". */
  fmt: (usd: number) => string;
}

// Last committed code+rate PAIR, shared across hook instances. A fresh mount
// (e.g. a tab pane) must never pair the stored code with the rate-1 default —
// ¥12 instead of ¥1,860 — so until some instance has committed a real pair,
// everyone renders the consistent USD/1.
let _committed: { code: string; rate: number } | null = null;

export function useDisplayCurrency(): DisplayCurrency {
  const [pair, setPair] = useState(() => _committed ?? { code: 'USD', rate: 1 });

  useFocusEffect(useCallback(() => {
    let alive = true;
    // Commit code + rate together: flipping the code while the old rate is still
    // applied would render a wrong-magnitude value for a frame (huge for IDR/KRW).
    loadCurrency().then(async (c) => {
      const r = await getRate(c);
      if (!alive) return;
      _committed = { code: c, rate: r };
      setPair(_committed);
    });
    return () => { alive = false; };
  }, []));

  const { code, rate } = pair;
  const meta = currencyMeta(code);
  return {
    code,
    symbol: meta.symbol,
    rate,
    fmt: (usd: number) => formatFiat(usd * rate, code, meta.symbol),
  };
}
