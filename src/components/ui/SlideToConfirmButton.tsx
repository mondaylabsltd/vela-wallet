/**
 * Slide-to-confirm button for deliberate, premium confirmation of consequential
 * actions (sends to risky recipients, danger-level signing). Replaces a timed
 * press-and-hold with a drag: the user slides the thumb to the end to commit.
 * Same anti-fat-finger intent as a hold — a stray tap can't fire it — but it
 * reads as a modern, intentional gesture (Coinbase / Revolut style).
 *
 * The premium feel comes from three things working together:
 *   - an accent progress fill that follows the thumb (the track "fills up"),
 *   - animated direction chevrons hinting which way to slide, and
 *   - a label parked clear of the thumb so it's never half-hidden at rest.
 *
 * Cross-platform via PanResponder — the same responder system AppModal's drag
 * uses — so it works with touch on iOS/Android AND mouse+touch on Expo web,
 * where react-native-gesture-handler's Pan is disabled. Drop-in API-compatible
 * with <HoldToConfirmButton>. All animation is JS-driven (non-native) so the
 * progress fill's width can track the same value as the thumb's translate.
 */
import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  Animated,
  PanResponder,
  Platform,
  View,
  ActivityIndicator,
  type ViewStyle,
} from 'react-native';
import { ArrowRight, ChevronRight } from 'lucide-react-native';
import { hapticLight, hapticSuccess } from '@/services/platform';
import { color, text, inter, space, shadow, createStyles } from '@/constants/theme';

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

  const danger = tone === 'danger';

  const fire = useCallback(() => {
    if (fired.current || disabled || loading) return;
    fired.current = true;
    hapticSuccess();
    onConfirm();
  }, [onConfirm, disabled, loading]);

  const reset = useCallback(() => {
    armed.current = false;
    ticked.current = false;
    Animated.spring(x, { toValue: 0, useNativeDriver: false, bounciness: 0, speed: 18 }).start(() => {
      fired.current = false;
    });
  }, [x]);

  const panResponder = useMemo(
    () =>
      PanResponder.create({
        onStartShouldSetPanResponder: () => !blockedRef.current,
        onMoveShouldSetPanResponder: (_e, g) => !blockedRef.current && Math.abs(g.dx) > 2,
        onPanResponderGrant: () => {
          if (blockedRef.current) return;
          // Self-initialize each drag so the button is never dependent on reset()
          // having run — a fresh grab always re-arms, even if a prior commit left
          // the latches set.
          fired.current = false;
          armed.current = false;
          ticked.current = false;
          hapticLight();
        },
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
            Animated.timing(x, { toValue: m, duration: 110, useNativeDriver: false }).start(() => fire());
          } else {
            reset();
          }
        },
        onPanResponderTerminate: () => reset(),
      }),
    [x, fire, reset],
  );

  // Looping shimmer that animates the direction chevrons. Paused while blocked.
  const pulse = useRef(new Animated.Value(0)).current;
  useEffect(() => {
    if (blocked) { pulse.setValue(0); return; }
    const anim = Animated.loop(
      Animated.timing(pulse, { toValue: 1, duration: 1500, useNativeDriver: false }),
    );
    anim.start();
    return () => anim.stop();
  }, [blocked, pulse]);

  // Re-arm after the action resolves. A committed slide latches `fired` and parks
  // the thumb at the end; `reset()` (the only thing that clears them) does NOT run
  // on the success path. Callers that unmount the button on commit (SendScreen) are
  // fine, but a persistently-mounted caller — the signing sheet across a cancelled
  // passkey prompt or an underfunded-gas retry — would otherwise be left with a
  // dead slider stuck at the far end. When the button leaves the blocked state,
  // spring back to start and clear the latches. Skips the initial mount.
  const wasBlocked = useRef(blocked);
  useEffect(() => {
    if (wasBlocked.current && !blocked) {
      fired.current = false;
      armed.current = false;
      ticked.current = false;
      Animated.spring(x, { toValue: 0, useNativeDriver: false, bounciness: 0, speed: 18 }).start();
    }
    wasBlocked.current = blocked;
  }, [blocked, x]);

  // Label + chevrons fade out as the thumb advances, so they never sit under it.
  const labelOpacity = x.interpolate({
    inputRange: [0, Math.max(1, maxX) * 0.5],
    outputRange: [1, 0],
    extrapolate: 'clamp',
  });
  // Label color rides from its at-rest tint (readable on the light track) to white
  // as the fill sweeps in behind it — otherwise grey-on-orange (or, in danger tone,
  // red-on-identical-red) goes unreadable in the overlap window.
  const labelColor = x.interpolate({
    inputRange: [0, Math.max(1, maxX) * 0.35],
    outputRange: [danger ? color.error.base : color.fg.muted, color.fg.inverse],
    extrapolate: 'clamp',
  });

  // Progress fill: left-anchored, grows to the thumb's right edge. Its rounded
  // right end is concentric with the thumb, so the thumb hides the seam.
  const fillWidth = Animated.add(x, THUMB);

  const chevTint = danger ? color.error.base : color.accent.base;

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
        <ActivityIndicator color={danger ? color.error.base : color.accent.base} />
      ) : (
        <>
          <Animated.View
            pointerEvents="none"
            style={[styles.fill, danger && styles.fillDanger, { width: fillWidth }]}
          />
          <Animated.View style={[styles.labelRow, { opacity: labelOpacity }]} pointerEvents="none">
            <Animated.Text style={[styles.label, { color: labelColor }]} numberOfLines={1}>
              {title}
            </Animated.Text>
            <View style={styles.chevrons}>
              {[0, 1, 2].map((i) => {
                const start = i * 0.18;
                return (
                  <Animated.View
                    key={i}
                    style={{
                      opacity: pulse.interpolate({
                        inputRange: [start, start + 0.18, start + 0.36, 1],
                        outputRange: [0.25, 1, 0.25, 0.25],
                        extrapolate: 'clamp',
                      }),
                    }}
                  >
                    <ChevronRight size={16} color={chevTint} strokeWidth={2.75} />
                  </Animated.View>
                );
              })}
            </View>
          </Animated.View>
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
    // No `overflow: hidden` here — it would clip the thumb's shadow. The fill is a
    // self-rounded pill inset by PAD, so it stays inside the track without clipping.
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
  fill: {
    position: 'absolute',
    left: PAD,
    top: PAD,
    bottom: PAD,
    borderRadius: THUMB / 2,
    backgroundColor: color.accent.base,
    // The thumb leads, an accent trail "paints" the track behind it.
    opacity: 0.9,
  },
  fillDanger: {
    backgroundColor: color.error.base,
  },
  labelRow: {
    position: 'absolute',
    left: TRACK_H,
    right: space.lg,
    top: 0,
    bottom: 0,
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    gap: space.sm,
  },
  label: {
    flexShrink: 1,
    textAlign: 'center',
    fontSize: text.lg,
    ...inter.semibold,
    // color is animated (see labelColor) — dark at rest, white under the fill.
  },
  chevrons: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 1,
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
    ...shadow.md,
    ...(Platform.OS === 'web' ? ({ cursor: 'grab' } as any) : null),
  },
  thumbDanger: {
    backgroundColor: color.error.base,
  },
  disabled: {
    opacity: 0.45,
  },
}));
