/**
 * Slide-to-confirm button for deliberate, premium confirmation of consequential
 * actions (sends to risky recipients, danger-level signing). Replaces a timed
 * press-and-hold with a drag: the user slides the thumb to the end to commit.
 * Same anti-fat-finger intent as a hold — a stray tap can't fire it — but it
 * reads as a modern, intentional gesture (Coinbase / Revolut style).
 *
 * Cross-platform via PanResponder — the same responder system AppModal's drag
 * uses — so it works with touch on iOS/Android AND mouse+touch on Expo web,
 * where react-native-gesture-handler's Pan is disabled. Drop-in API-compatible
 * with <HoldToConfirmButton>.
 */
import React, { useCallback, useMemo, useRef, useState } from 'react';
import {
  Animated,
  PanResponder,
  Platform,
  Text,
  View,
  ActivityIndicator,
  type ViewStyle,
} from 'react-native';
import { ArrowRight } from 'lucide-react-native';
import { hapticLight, hapticSuccess } from '@/services/platform';
import { color, text, inter, radius, space, createStyles } from '@/constants/theme';

const TRACK_H = 60;
const THUMB = 52;
const PAD = 4;
/** Fraction of the track the thumb must cross before release to commit. */
const COMPLETE = 0.88;

interface Props {
  /** Primary label shown on the track (e.g. "Confirm & Send"). */
  title: string;
  /** Secondary hint (e.g. "Slide to confirm"). Also the a11y hint. */
  hint: string;
  onConfirm: () => void;
  disabled?: boolean;
  loading?: boolean;
  /** 'accent' (orange, default) or 'danger' (red) for higher-stakes actions. */
  tone?: 'accent' | 'danger';
  style?: ViewStyle;
}

export function SlideToConfirmButton({ title, hint, onConfirm, disabled, loading, tone = 'accent', style }: Props) {
  const x = useRef(new Animated.Value(0)).current;
  const [trackW, setTrackW] = useState(0);
  const maxX = Math.max(0, trackW - THUMB - PAD * 2);
  // Mirror for the PanResponder closure (which is memoised and won't see state).
  const maxXRef = useRef(0);
  maxXRef.current = maxX;

  const armed = useRef(false); // crossed the commit threshold this drag
  const ticked = useRef(false); // mid-drag tick already fired
  const fired = useRef(false);
  const blocked = !!(disabled || loading);
  const blockedRef = useRef(blocked);
  blockedRef.current = blocked;

  const fire = useCallback(() => {
    if (fired.current || disabled || loading) return;
    fired.current = true;
    hapticSuccess();
    onConfirm();
  }, [onConfirm, disabled, loading]);

  const reset = useCallback(() => {
    armed.current = false;
    ticked.current = false;
    Animated.spring(x, { toValue: 0, useNativeDriver: true, bounciness: 0, speed: 18 }).start(() => {
      fired.current = false;
    });
  }, [x]);

  const panResponder = useMemo(
    () =>
      PanResponder.create({
        onStartShouldSetPanResponder: () => !blockedRef.current,
        onMoveShouldSetPanResponder: (_e, g) => !blockedRef.current && Math.abs(g.dx) > 2,
        onPanResponderGrant: () => { if (!blockedRef.current) hapticLight(); },
        onPanResponderMove: (_e, g) => {
          if (blockedRef.current) return;
          const m = maxXRef.current;
          const nx = Math.min(Math.max(0, g.dx), m);
          x.setValue(nx);
          if (!ticked.current && m > 0 && nx >= m * 0.5) { ticked.current = true; hapticLight(); }
          armed.current = m > 0 && nx >= m * COMPLETE;
        },
        onPanResponderRelease: () => {
          if (blockedRef.current) { reset(); return; }
          const m = maxXRef.current;
          if (armed.current && m > 0) {
            Animated.timing(x, { toValue: m, duration: 110, useNativeDriver: true }).start(() => fire());
          } else {
            reset();
          }
        },
        onPanResponderTerminate: () => reset(),
      }),
    [x, fire, reset],
  );

  const labelOpacity = x.interpolate({
    inputRange: [0, Math.max(1, maxX) * 0.55],
    outputRange: [1, 0],
    extrapolate: 'clamp',
  });

  const danger = tone === 'danger';

  return (
    <View
      style={[styles.track, danger && styles.trackDanger, blocked && styles.disabled, style]}
      onLayout={(e) => setTrackW(e.nativeEvent.layout.width)}
      accessible
      accessibilityRole="button"
      accessibilityLabel={title}
      accessibilityHint={hint}
      accessibilityState={{ disabled: blocked }}
      accessibilityActions={[{ name: 'activate' }]}
      onAccessibilityAction={(e) => { if (e.nativeEvent.actionName === 'activate') fire(); }}
    >
      {loading ? (
        <ActivityIndicator color={danger ? color.fg.inverse : color.accent.base} />
      ) : (
        <>
          <Animated.Text style={[styles.label, { opacity: labelOpacity }]} pointerEvents="none" numberOfLines={1}>
            {title}
          </Animated.Text>
          <Animated.View
            style={[styles.thumb, danger && styles.thumbDanger, { transform: [{ translateX: x }] }]}
            {...panResponder.panHandlers}
          >
            <ArrowRight size={22} color={color.fg.inverse} strokeWidth={2.6} />
          </Animated.View>
        </>
      )}
    </View>
  );
}

const styles = createStyles(() => ({
  track: {
    height: TRACK_H,
    borderRadius: TRACK_H / 2,
    backgroundColor: color.bg.sunken,
    borderWidth: 1,
    borderColor: color.border.base,
    justifyContent: 'center',
    overflow: 'hidden',
    // The slide gesture owns the drag — never let the browser select the label
    // or pop the iOS/Android touch callout, which would cancel the gesture.
    userSelect: 'none',
    ...(Platform.OS === 'web'
      ? ({ WebkitUserSelect: 'none', WebkitTouchCallout: 'none', touchAction: 'none' } as any)
      : null),
  },
  trackDanger: {
    backgroundColor: color.error.soft,
    borderColor: color.error.soft,
  },
  label: {
    position: 'absolute',
    left: THUMB,
    right: space.xl,
    textAlign: 'center',
    fontSize: text.lg,
    ...inter.semibold,
    color: color.fg.muted,
  },
  thumb: {
    position: 'absolute',
    left: PAD,
    top: PAD,
    width: THUMB,
    height: THUMB,
    borderRadius: THUMB / 2,
    backgroundColor: color.accent.base,
    alignItems: 'center',
    justifyContent: 'center',
    ...(Platform.OS === 'web' ? ({ cursor: 'grab' } as any) : null),
  },
  thumbDanger: {
    backgroundColor: color.error.base,
  },
  disabled: {
    opacity: 0.45,
  },
}));
