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

export function useDisplayCurrency(): DisplayCurrency {
  const [code, setCode] = useState(getCurrencyCode());
  const [rate, setRate] = useState(1);

  useFocusEffect(useCallback(() => {
    let alive = true;
    loadCurrency().then((c) => {
      if (!alive) return;
      setCode(c);
      getRate(c).then((r) => { if (alive) setRate(r); });
    });
    return () => { alive = false; };
  }, []));

  const meta = currencyMeta(code);
  return {
    code,
    symbol: meta.symbol,
    rate,
    fmt: (usd: number) => formatFiat(usd * rate, code, meta.symbol),
  };
}
