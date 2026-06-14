/**
 * VelaRefresh — branded, gesture-driven pull-to-refresh.
 *
 * Why custom (not RefreshControl): the native control's spinner can't be styled
 * on iOS, and iOS/Android behave differently. We drive the whole thing from a
 * Reanimated Pan gesture so the feel is identical on iOS, Android and web:
 *
 *   · finger-tracked elastic pull (1:1 until the trigger, then it gets heavy)
 *   · a single crisp haptic the moment you cross the trigger
 *   · a branded arc indicator that "draws" with the pull, then spins while loading
 *   · spring-back release
 *
 * Cross-platform consistency: native overscroll is disabled (`bounces=false`,
 * `overScrollMode="never"`) so this component fully owns the top overscroll
 * region. The Pan runs *simultaneously* with the list's own scroll and only
 * engages a pull when the list is already at the top — so normal scrolling is
 * untouched.
 *
 * Usage — the child is a render-prop that must spread `scrollProps` onto an
 * Animated scrollable (`Animated.FlatList` / `Animated.ScrollView`):
 *
 *   <VelaRefresh refreshing={refreshing} onRefresh={onRefresh}>
 *     {(scrollProps) => (
 *       <Animated.FlatList {...scrollProps} data={...} renderItem={...} />
 *     )}
 *   </VelaRefresh>
 */
import React, { useEffect, useRef } from 'react';
import { Platform, View, type StyleProp, type ViewStyle } from 'react-native';
import { Gesture, GestureDetector } from 'react-native-gesture-handler';
import Animated, {
  cancelAnimation,
  Easing,
  runOnJS,
  useAnimatedProps,
  useAnimatedScrollHandler,
  useAnimatedStyle,
  useSharedValue,
  withRepeat,
  withSpring,
  withTiming,
  type SharedValue,
} from 'react-native-reanimated';
import Svg, { Circle } from 'react-native-svg';

import { color, createStyles, motion } from '@/constants/theme';
import { hapticLight } from '@/services/platform';

const AnimatedCircle = Animated.createAnimatedComponent(Circle);

/** Pull distance (px) at which a release fires a refresh. */
const TRIGGER = 72;
/** Resting indicator height while the refresh runs. */
const REST = 54;
/** Resistance applied to pull beyond the trigger (1 = none, 0 = solid). */
const OVERPULL = 0.4;
const SPRING = motion.spring;

// Indicator geometry.
const RING = 30;
const STROKE = 3;
const R = (RING - STROKE) / 2;
const CIRC = 2 * Math.PI * R;

export interface VelaScrollProps {
  ref: React.RefObject<any>;
  onScroll: ReturnType<typeof useAnimatedScrollHandler>;
  scrollEventThrottle: number;
  bounces: boolean;
  overScrollMode: 'never';
  /** iOS: keep the (now custom) refresh region honest about the content top. */
  contentInsetAdjustmentBehavior: 'never';
}

interface VelaRefreshProps {
  refreshing: boolean;
  onRefresh: () => void;
  children: (scrollProps: VelaScrollProps) => React.ReactNode;
  style?: StyleProp<ViewStyle>;
  /** Disable the pull (e.g. while a sheet is open). Scrolling still works. */
  enabled?: boolean;
}

export function VelaRefresh({ refreshing, onRefresh, children, style, enabled = true }: VelaRefreshProps) {
  const scrollRef = useRef<any>(null);

  const pull = useSharedValue(0);
  const scrollY = useSharedValue(0);
  const baseline = useSharedValue(0); // translationY captured at the moment we reach the top
  const armed = useSharedValue(false); // haptic latch — fire once per threshold crossing
  const busy = useSharedValue(false); // mirrors `refreshing` on the UI thread

  const scrollHandler = useAnimatedScrollHandler({
    onScroll: (e) => { scrollY.value = e.contentOffset.y; },
  });

  // Keep the refresh running until the parent clears `refreshing`, then settle.
  useEffect(() => {
    busy.value = refreshing;
    if (refreshing) {
      pull.value = withSpring(REST, SPRING);
    } else {
      pull.value = withSpring(0, SPRING);
      armed.value = false;
    }
  }, [refreshing, pull, busy, armed]);

  const fireRefresh = () => onRefresh();

  const pan = Gesture.Pan()
    .enabled(enabled)
    .simultaneousWithExternalGesture(scrollRef)
    // Engage on a downward drag; bail to the scroll view on an upward drag.
    .activeOffsetY(12)
    .failOffsetY(-12)
    .onChange((e) => {
      'worklet';
      if (busy.value) return;
      // Not at the top → no pull; keep the baseline pinned so the pull starts
      // from zero the instant the user reaches the top mid-drag.
      if (scrollY.value > 1) {
        if (pull.value !== 0) pull.value = 0;
        baseline.value = e.translationY;
        if (armed.value) armed.value = false;
        return;
      }
      const d = e.translationY - baseline.value;
      if (d <= 0) {
        if (pull.value !== 0) pull.value = 0;
        if (armed.value) armed.value = false;
        return;
      }
      // 1:1 to the trigger, then heavy — the resistance change *is* the threshold.
      const next = d < TRIGGER ? d : TRIGGER + (d - TRIGGER) * OVERPULL;
      pull.value = next;
      if (next >= TRIGGER && !armed.value) {
        armed.value = true;
        runOnJS(hapticLight)();
      } else if (next < TRIGGER && armed.value) {
        armed.value = false;
      }
    })
    .onEnd(() => {
      'worklet';
      armed.value = false;
      baseline.value = 0;
      if (busy.value) return;
      if (pull.value >= TRIGGER) {
        pull.value = withSpring(REST, SPRING);
        runOnJS(fireRefresh)();
      } else {
        pull.value = withSpring(0, SPRING);
      }
    });

  const listStyle = useAnimatedStyle(() => ({ transform: [{ translateY: pull.value }] }));
  const bandStyle = useAnimatedStyle(() => ({
    opacity: Math.min(1, pull.value / (TRIGGER * 0.7)),
    transform: [{ translateY: pull.value - TRIGGER }],
  }));

  const scrollProps: VelaScrollProps = {
    ref: scrollRef,
    onScroll: scrollHandler,
    scrollEventThrottle: 16,
    bounces: false,
    overScrollMode: 'never',
    contentInsetAdjustmentBehavior: 'never',
  };

  return (
    <GestureDetector gesture={pan}>
      <View style={[styles.clip, style]}>
        <Animated.View pointerEvents="none" style={[styles.band, bandStyle]}>
          <RefreshIndicator pull={pull} refreshing={refreshing} />
        </Animated.View>
        <Animated.View style={[styles.fill, listStyle]}>
          {children(scrollProps)}
        </Animated.View>
      </View>
    </GestureDetector>
  );
}

// ---------------------------------------------------------------------------
// Indicator — accent arc that draws with the pull, then spins while loading.
// ---------------------------------------------------------------------------

function RefreshIndicator({ pull, refreshing }: { pull: SharedValue<number>; refreshing: boolean }) {
  const spin = useSharedValue(0);

  useEffect(() => {
    if (refreshing) {
      spin.value = withRepeat(withTiming(360, { duration: 750, easing: Easing.linear }), -1, false);
    } else {
      cancelAnimation(spin);
      spin.value = withTiming(0, { duration: 160 });
    }
  }, [refreshing, spin]);

  const wrapStyle = useAnimatedStyle(() => {
    const p = Math.min(1, Math.max(0, pull.value / TRIGGER));
    const rotate = refreshing ? spin.value : p * 130;
    return {
      transform: [
        { scale: refreshing ? 1 : 0.55 + p * 0.45 },
        { rotate: `${rotate}deg` },
      ],
    };
  });

  const arcProps = useAnimatedProps(() => {
    const p = Math.min(1, Math.max(0, pull.value / TRIGGER));
    const frac = refreshing ? 0.72 : 0.08 + p * 0.62;
    return { strokeDashoffset: CIRC * (1 - frac) };
  });

  return (
    <Animated.View style={[styles.indicator, wrapStyle]}>
      <Svg width={RING} height={RING} viewBox={`0 0 ${RING} ${RING}`}>
        {/* Faint full-ring track for depth. */}
        <Circle cx={RING / 2} cy={RING / 2} r={R} stroke={color.border.base} strokeWidth={STROKE} fill="none" opacity={0.6} />
        <AnimatedCircle
          cx={RING / 2}
          cy={RING / 2}
          r={R}
          stroke={color.accent.base}
          strokeWidth={STROKE}
          strokeLinecap="round"
          fill="none"
          strokeDasharray={CIRC}
          animatedProps={arcProps}
          // Start the arc from 12 o'clock.
          transform={`rotate(-90 ${RING / 2} ${RING / 2})`}
        />
      </Svg>
    </Animated.View>
  );
}

const styles = createStyles(() => ({
  clip: { flex: 1, overflow: 'hidden' },
  fill: { flex: 1 },
  band: {
    position: 'absolute',
    top: 0,
    left: 0,
    right: 0,
    height: TRIGGER,
    alignItems: 'center',
    justifyContent: 'center',
    zIndex: 0,
  },
  indicator: {
    width: RING + 14,
    height: RING + 14,
    borderRadius: (RING + 14) / 2,
    alignItems: 'center',
    justifyContent: 'center',
    backgroundColor: Platform.OS === 'web' ? 'transparent' : color.bg.raised,
    ...(Platform.OS === 'web'
      ? {}
      : {
          shadowColor: '#000000',
          shadowOffset: { width: 0, height: 2 },
          shadowOpacity: 0.08,
          shadowRadius: 6,
          elevation: 3,
        }),
  },
}));
