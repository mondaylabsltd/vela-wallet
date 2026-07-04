/**
 * Slide-to-confirm button for deliberate confirmation of consequential actions
 * (sends, danger-level signing). Replaces a timed press-and-hold with a drag:
 * the user slides the knob to the end to commit. Same anti-fat-finger intent
 * as a hold — a stray tap can't fire it.
 *
 * Look — matches the getvela.app landing mockup (the founder-approved design):
 * a QUIET raised track with a hairline border, an accent knob with a white
 * arrow, and a muted centered label. On commit the track settles into soft
 * success green. Never a red track — recipient risk is signaled by the tags
 * and copy above the control, not by making the commit surface scary.
 *
 * Feel: the drag runs on the UI thread — react-native-gesture-handler Pan +
 * Reanimated shared values (the same stack as Settings' text-scale slider), so
 * the knob tracks the finger even while JS is busy estimating gas. Grab scales
 * the knob, a tick fires at 60%, overdrag rubber-bands, a fast flick past
 * mid-track commits, and an under-threshold release springs back. The knob
 * "peeks" right a few times at rest to teach the gesture, and stops forever on
 * first grab. `activeOffsetX`/`failOffsetY` yield cleanly to vertical scroll.
 *
 * iPhone app-switcher note: the SYSTEM horizontal-swipe gesture owns the home-
 * indicator band, so this control must never rest against the screen's bottom
 * edge — call sites keep ≥ ~48pt of clearance below it (see SendScreen's
 * confirmBtn margin; the signing sheet parks a Reject button beneath it).
 */
import React, { useCallback, useEffect, useRef, useState } from 'react';
import { ActivityIndicator, Platform, type ViewStyle } from 'react-native';
import { Gesture, GestureDetector } from 'react-native-gesture-handler';
import Animated, {
  cancelAnimation,
  interpolateColor,
  runOnJS,
  useAnimatedStyle,
  useSharedValue,
  withDelay,
  withRepeat,
  withSequence,
  withSpring,
  withTiming,
} from 'react-native-reanimated';
import { ArrowRight } from 'lucide-react-native';
import { hapticLight, hapticSuccess } from '@/services/platform';
import { color, text, inter, shadow, createStyles } from '@/constants/theme';

const TRACK_H = 60;
const THUMB = 52;
const PAD = 4;
/** Fraction of the track the thumb must cross before release to commit. */
const COMPLETE = 0.8;
/** A fast rightward flick past this fraction also commits (feels premium). */
const FLICK_MIN = 0.45;
const FLICK_VELOCITY = 900;

interface Props {
  /** Primary label shown on the track (e.g. "Confirm & Send"). */
  title: string;
  /** Secondary hint (e.g. "Slide to confirm"). Also the a11y hint. */
  hint: string;
  onConfirm: () => void;
  disabled?: boolean;
  loading?: boolean;
  style?: ViewStyle;
}

export function SlideToConfirmButton({ title, hint, onConfirm, disabled, loading, style }: Props) {
  const x = useSharedValue(0);
  const startX = useSharedValue(0);
  const maxX = useSharedValue(0);
  const grabbed = useSharedValue(0); // 1 while the finger is down (knob scale)
  const nudge = useSharedValue(0); // idle "peek" offset — teaches the gesture
  const done = useSharedValue(0); // 1 after commit — track settles into success
  const ticked = useSharedValue(false); // mid-drag haptic latch
  const [trackW, setTrackW] = useState(0);

  const fired = useRef(false);
  const blocked = !!(disabled || loading);
  const trackRef = useRef<any>(null);

  useEffect(() => {
    maxX.value = Math.max(0, trackW - THUMB - PAD * 2);
  }, [trackW, maxX]);

  const fire = useCallback(() => {
    if (fired.current || disabled || loading) return;
    fired.current = true;
    hapticSuccess();
    onConfirm();
  }, [onConfirm, disabled, loading]);

  // Idle nudge: the knob peeks right a few times, then rests. Killed for good
  // on the first grab — once the user has the gesture, motion is just noise.
  const nudgeKilled = useRef(false);
  useEffect(() => {
    if (blocked || nudgeKilled.current) { cancelAnimation(nudge); nudge.value = 0; return; }
    nudge.value = withRepeat(
      withSequence(
        withDelay(2200, withTiming(9, { duration: 240 })),
        withSpring(0, { damping: 13, stiffness: 240 }),
      ),
      3,
      false,
    );
    return () => cancelAnimation(nudge);
  }, [blocked, nudge]);

  const killNudge = useCallback(() => { nudgeKilled.current = true; }, []);

  // Web keyboard access: the commit is a pointer-drag, so on web the whole
  // (idle) send/sign flow would be unreachable by keyboard & switch users — this
  // is the ONLY commit control. Expose the track as a focusable button that fires
  // on Enter/Space (explicit activation is an acceptable a11y substitute for the
  // anti-fat-finger drag). Native keeps its onAccessibilityAction path below.
  const blockedRef = useRef(blocked);
  blockedRef.current = blocked;
  useEffect(() => {
    if (Platform.OS !== 'web') return;
    const el = trackRef.current as HTMLElement | null;
    if (!el) return;
    el.setAttribute('tabindex', blocked ? '-1' : '0');
    const onKeyDown = (e: KeyboardEvent) => {
      if (blockedRef.current) return;
      if (e.key === 'Enter' || e.key === ' ' || e.key === 'Spacebar') {
        e.preventDefault();
        fire();
      }
    };
    el.addEventListener('keydown', onKeyDown);
    return () => el.removeEventListener('keydown', onKeyDown);
  }, [blocked, fire]);

  // Re-arm after the action resolves. A committed slide latches `fired` and
  // parks the thumb at the end. Callers that unmount the button on commit
  // (SendScreen) are fine, but a persistently-mounted caller — the signing
  // sheet across a cancelled passkey prompt or an underfunded-gas retry —
  // would otherwise be left with a dead slider stuck at the far end. When the
  // button leaves the blocked state, spring back and clear the latch.
  const wasBlocked = useRef(blocked);
  useEffect(() => {
    if (wasBlocked.current && !blocked) {
      fired.current = false;
      ticked.value = false;
      done.value = withTiming(0, { duration: 200 });
      x.value = withSpring(0, { damping: 18, stiffness: 260 });
    }
    wasBlocked.current = blocked;
  }, [blocked, x, ticked, done]);

  // While loading, park the thumb at the end (it hosts the spinner) — covers a
  // caller that mounts the button already-loading.
  useEffect(() => {
    if (loading && maxX.value > 0) x.value = withTiming(maxX.value, { duration: 160 });
  }, [loading, maxX, x]);

  const pan = Gesture.Pan()
    .enabled(!blocked)
    .activeOffsetX([-6, 6])
    .failOffsetY([-14, 14])
    .onStart(() => {
      startX.value = x.value;
      grabbed.value = withSpring(1, { damping: 16, stiffness: 320 });
      cancelAnimation(nudge);
      nudge.value = withTiming(0, { duration: 80 });
      ticked.value = false;
      runOnJS(killNudge)();
      runOnJS(hapticLight)();
    })
    .onUpdate((e) => {
      const m = maxX.value;
      if (m <= 0) return;
      const raw = startX.value + e.translationX;
      // Rubber-band past both ends instead of a hard stop.
      x.value = raw < 0
        ? raw * 0.12
        : raw > m
          ? m + Math.min((raw - m) * 0.12, 10)
          : raw;
      if (!ticked.value && x.value >= m * 0.6) {
        ticked.value = true;
        runOnJS(hapticLight)();
      }
    })
    .onEnd((e) => {
      grabbed.value = withSpring(0, { damping: 16, stiffness: 320 });
      const m = maxX.value;
      if (m <= 0) return;
      const commit = x.value >= m * COMPLETE ||
        (e.velocityX > FLICK_VELOCITY && x.value >= m * FLICK_MIN);
      if (commit) {
        done.value = withTiming(1, { duration: 220 });
        x.value = withTiming(m, { duration: 110 }, (finished) => {
          if (finished) runOnJS(fire)();
        });
      } else {
        x.value = withSpring(0, { damping: 18, stiffness: 260 });
      }
    })
    .onFinalize((_e, success) => {
      if (!success) {
        grabbed.value = withSpring(0, { damping: 16, stiffness: 320 });
        x.value = withSpring(0, { damping: 18, stiffness: 260 });
      }
    });

  // Colors resolved in render scope (theme-aware tokens), captured by worklets
  // as plain strings.
  const trackBgRest = color.bg.raised;
  const trackBgDone = color.success.soft;
  const borderRest = color.border.base;
  const borderDone = 'rgba(45, 142, 95, 0.3)';

  const trackStyle = useAnimatedStyle(() => ({
    backgroundColor: interpolateColor(done.value, [0, 1], [trackBgRest, trackBgDone]),
    borderColor: interpolateColor(done.value, [0, 1], [borderRest, borderDone]),
  }));

  const thumbStyle = useAnimatedStyle(() => ({
    transform: [
      { translateX: x.value + nudge.value },
      { scale: 1 + grabbed.value * 0.06 },
    ],
  }));

  // Label fades and drifts as the knob approaches it.
  const labelStyle = useAnimatedStyle(() => {
    const m = Math.max(1, maxX.value);
    const p = Math.min(1, Math.max(0, x.value / (m * 0.55)));
    return {
      opacity: 1 - p,
      transform: [{ translateX: p * 14 }],
    };
  });

  return (
    <Animated.View
      ref={trackRef}
      style={[styles.track, trackStyle, blocked && styles.disabled, style]}
      onLayout={(e) => setTrackW(e.nativeEvent.layout.width)}
      accessible
      accessibilityRole="button"
      accessibilityLabel={title}
      accessibilityHint={hint}
      accessibilityState={{ disabled: blocked }}
      accessibilityActions={[{ name: 'activate' }]}
      onAccessibilityAction={(e) => { if (e.nativeEvent.actionName === 'activate') fire(); }}
    >
      <Animated.View style={[styles.labelRow, labelStyle]} pointerEvents="none">
        <Animated.Text style={styles.label} numberOfLines={1}>{title}</Animated.Text>
      </Animated.View>
      <GestureDetector gesture={pan}>
        <Animated.View style={[styles.thumb, thumbStyle]} hitSlop={{ top: 10, bottom: 10, left: 10, right: 10 }}>
          {loading ? (
            <ActivityIndicator size="small" color={color.fg.inverse} />
          ) : (
            <ArrowRight size={22} color={color.fg.inverse} strokeWidth={2.6} />
          )}
        </Animated.View>
      </GestureDetector>
    </Animated.View>
  );
}

const styles = createStyles(() => ({
  // Quiet raised track + hairline border (landing-page mockup look): the accent
  // knob is the only loud element — the commit surface itself never shouts.
  track: {
    height: TRACK_H,
    borderRadius: TRACK_H / 2,
    backgroundColor: color.bg.raised,
    borderWidth: 1,
    borderColor: color.border.base,
    justifyContent: 'center',
    // No `overflow: hidden` — it would clip the knob's shadow. Never let the
    // browser select the label or pop the touch callout mid-drag.
    userSelect: 'none',
    ...(Platform.OS === 'web'
      ? ({ WebkitUserSelect: 'none', WebkitTouchCallout: 'none', touchAction: 'pan-y' } as any)
      : null),
  },
  labelRow: {
    position: 'absolute',
    left: TRACK_H,
    right: TRACK_H, // symmetric: label stays optically centered on the track
    top: 0,
    bottom: 0,
    alignItems: 'center',
    justifyContent: 'center',
  },
  label: {
    fontSize: text.lg,
    ...inter.semibold,
    color: color.fg.muted,
    textAlign: 'center',
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
  disabled: {
    opacity: 0.45,
  },
}));
