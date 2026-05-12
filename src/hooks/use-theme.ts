/**
 * Legacy theme hook — returns dynamic colors based on current color scheme.
 * Reads from the mutable `color` tokens so it always reflects the active mode.
 */

import { getThemeColors } from '@/constants/theme';
import { useColorSchemePreference } from '@/constants/color-scheme';

export function useTheme() {
  // Subscribe to color scheme changes so consumers re-render
  useColorSchemePreference();
  return getThemeColors();
}
