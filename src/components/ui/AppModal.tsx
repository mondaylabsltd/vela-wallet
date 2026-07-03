/**
 * Cross-platform modal.
 *
 * - iOS: native <Modal pageSheet> with `allowSwipeDismissal`, which re-enables
 *   UIKit's interactive pull-down (RN pins pageSheet modals by default). The
 *   grabber is purely visual — the OS gesture tracks the finger 1:1 and hands
 *   off from an inner ScrollView at the top on its own.
 * - Android: <Modal> is full-screen (no native sheet gesture), so we add our own
 *   whole-sheet drag-to-dismiss with a threshold haptic. The drag initiates from
 *   the top handle region only, so it never fights an inner ScrollView.
 * - Web: portal to #root with slide-up animation, backdrop + drag dismiss.
 */
import React, { useEffect, useState, useRef } from 'react';
import {
  Modal,
  View,
  Platform,
  PanResponder,
  Animated,
  Dimensions,
  KeyboardAvoidingView,
} from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';
import { hapticLight } from '@/services/platform';
import { color, createStyles, radius } from '@/constants/theme';
import { useWebDialog } from '@/hooks/use-web-dialog';

/** Drag past this many px (or fling faster than VY) to dismiss. */
const DISMISS_DY = 90;
const DISMISS_VY = 0.5;
/** Off-screen target for the dismiss throw. */
const SCREEN_H = Dimensions.get('window').height;

interface Props {
  visible: boolean;
  children: React.ReactNode;
  onClose?: () => void;
}

export function AppModal({ visible, children, onClose }: Props) {
  if (Platform.OS === 'web') {
    return <WebModal visible={visible} onClose={onClose}>{children}</WebModal>;
  }
  if (Platform.OS === 'android') {
    return <AndroidSheet visible={visible} onClose={onClose}>{children}</AndroidSheet>;
  }
  // iOS — allowSwipeDismissal unpins the pageSheet so UIKit's pull-down tracks
  // the finger (without it the sheet only rubber-bands, then closes by a canned
  // animation on release). The handle is static: a JS drag here would fight the
  // live native gesture.
  return (
    <Modal
      visible={visible}
      animationType="slide"
      presentationStyle="pageSheet"
      allowSwipeDismissal
      onRequestClose={onClose}
    >
      <View style={styles.nativeRoot}>
        <View style={styles.handleArea}>
          <View style={styles.handleBar} />
        </View>
        <KeyboardAvoidingView
          style={styles.nativeContent}
          behavior="padding"
        >
          <SafeAreaView style={styles.nativeContent} edges={['bottom']}>
            {children}
          </SafeAreaView>
        </KeyboardAvoidingView>
      </View>
    </Modal>
  );
}

// ---------------------------------------------------------------------------
// Android sheet — whole content follows the drag, dismisses past a threshold.
// ---------------------------------------------------------------------------

function AndroidSheet({ visible, onClose, children }: { visible: boolean; onClose?: () => void; children: React.ReactNode }) {
  const pan = useRef(new Animated.Value(0)).current;
  const armed = useRef(false);
  const dismissing = useRef(false);
  // PanResponder is created once; read the latest onClose through a ref.
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;

  // Reset position whenever the sheet (re)opens.
  useEffect(() => {
    if (visible) { pan.setValue(0); armed.current = false; dismissing.current = false; }
  }, [visible, pan]);

  const springBack = () =>
    Animated.spring(pan, { toValue: 0, useNativeDriver: true, tension: 80, friction: 10 }).start();

  const responder = useRef(
    PanResponder.create({
      // Claim on MOVE, not start, so taps fall through and only a deliberate
      // downward drag (not a horizontal swipe) is captured. (onStart=true would
      // make the move gate dead code.)
      onStartShouldSetPanResponder: () => false,
      onMoveShouldSetPanResponder: (_, g) => g.dy > 4 && g.dy > Math.abs(g.dx),
      onPanResponderMove: (_, g) => {
        const dy = Math.max(0, g.dy);
        pan.setValue(dy);
        // Fire one "armed to dismiss" haptic when crossing the threshold.
        if (!armed.current && dy > DISMISS_DY) { armed.current = true; hapticLight(); }
        else if (armed.current && dy <= DISMISS_DY) { armed.current = false; }
      },
      onPanResponderRelease: (_, g) => {
        armed.current = false;
        if (g.dy > DISMISS_DY || g.vy > DISMISS_VY) {
          // Throw the sheet fully off-screen, THEN close — so the native
          // slide-out begins from a settled (off-screen) state, no top-gap jump.
          if (dismissing.current) return;
          dismissing.current = true;
          Animated.timing(pan, { toValue: SCREEN_H, duration: 200, useNativeDriver: true })
            .start(() => onCloseRef.current?.());
        } else {
          springBack();
        }
      },
      onPanResponderTerminate: () => {
        armed.current = false;
        if (!dismissing.current) springBack();
      },
    }),
  ).current;

  return (
    <Modal visible={visible} animationType="slide" presentationStyle="pageSheet" onRequestClose={onClose}>
      {/* Static full-screen backdrop in the sheet color — the inner content
          translates over it, so the revealed area above stays seamless. */}
      <View style={styles.nativeRoot}>
        <Animated.View style={[styles.sheetInner, { transform: [{ translateY: pan }] }]}>
          <View style={styles.handleArea} {...responder.panHandlers}>
            <View style={styles.handleBar} />
          </View>
          <KeyboardAvoidingView style={styles.nativeContent} behavior="padding">
            <SafeAreaView style={styles.nativeContent} edges={['bottom']}>
              {children}
            </SafeAreaView>
          </KeyboardAvoidingView>
        </Animated.View>
      </View>
    </Modal>
  );
}

// ---------------------------------------------------------------------------
// Drag handle (web): handle-region drag that closes past a threshold.
// ---------------------------------------------------------------------------

function DragHandle({ onClose }: { onClose?: () => void }) {
  const pan = useRef(new Animated.Value(0)).current;

  const responder = useRef(
    PanResponder.create({
      onStartShouldSetPanResponder: () => true,
      onMoveShouldSetPanResponder: (_, g) => g.dy > 4,
      onPanResponderMove: (_, g) => { if (g.dy > 0) pan.setValue(g.dy); },
      onPanResponderRelease: (_, g) => {
        if (g.dy > 80 || g.vy > 0.5) {
          onClose?.();
        }
        Animated.spring(pan, { toValue: 0, useNativeDriver: true, tension: 80, friction: 10 }).start();
      },
    }),
  ).current;

  return (
    <Animated.View style={[styles.handleArea, { transform: [{ translateY: pan }] }]} {...responder.panHandlers}>
      <View style={styles.handleBar} />
    </Animated.View>
  );
}

// ---------------------------------------------------------------------------
// Web modal (rendered via DOM portal into #root)
// ---------------------------------------------------------------------------

function WebModal({ visible, onClose, children }: { visible: boolean; onClose?: () => void; children: React.ReactNode }) {
  const [mounted, setMounted] = useState(false);
  const [show, setShow] = useState(false);
  const [container, setContainer] = useState<HTMLDivElement | null>(null);

  // Create a DOM container as direct child of #root
  useEffect(() => {
    if (typeof document === 'undefined') return;
    const el = document.createElement('div');
    el.style.cssText = 'position:absolute;top:0;left:0;right:0;bottom:0;z-index:99999;pointer-events:none;';
    const root = document.getElementById('root');
    if (root) {
      root.appendChild(el);
      setContainer(el);
    }
    return () => { el.remove(); };
  }, []);

  useEffect(() => {
    if (!container) return;
    if (visible) {
      container.style.pointerEvents = 'auto';
      setMounted(true);
      requestAnimationFrame(() => requestAnimationFrame(() => setShow(true)));
    } else {
      setShow(false);
      const t = setTimeout(() => {
        setMounted(false);
        if (container) container.style.pointerEvents = 'none';
      }, 300);
      return () => clearTimeout(t);
    }
  }, [visible, container]);

  // Escape-to-close, focus trap, focus restore + background scroll lock (web only).
  const dialogRef = useWebDialog(visible, onClose);

  if (!container || !mounted) return null;

  const { createPortal } = require('react-dom');

  return createPortal(
    <div style={{ position: 'absolute', inset: 0, display: 'flex', flexDirection: 'column', justifyContent: 'flex-end' }}>
      {/* Backdrop */}
      <div
        onClick={onClose}
        style={{
          position: 'absolute', inset: 0,
          backgroundColor: show ? 'rgba(0,0,0,0.35)' : 'rgba(0,0,0,0)',
          transition: 'background-color 0.3s ease',
        }}
      />
      {/* Content sheet */}
      <div
        ref={dialogRef}
        style={{
          position: 'relative',
          backgroundColor: color.bg.base,
          borderTopLeftRadius: radius['2xl'],
          borderTopRightRadius: radius['2xl'],
          maxHeight: '92%',
          overflow: 'auto',
          outline: 'none',
          transform: show ? 'translateY(0)' : 'translateY(100%)',
          transition: 'transform 0.3s cubic-bezier(0.4, 0, 0.2, 1)',
        }}
      >
        <DragHandle onClose={onClose} />
        <View style={styles.webContent}>{children}</View>
      </div>
    </div>,
    container,
  );
}

// ---------------------------------------------------------------------------
// Styles
// ---------------------------------------------------------------------------

const styles = createStyles(() => ({
  nativeRoot: { flex: 1, backgroundColor: color.bg.base },
  sheetInner: { flex: 1 },
  nativeContent: { flex: 1 },
  handleArea: { alignItems: 'center', paddingTop: 10, paddingBottom: 6 },
  handleBar: { width: 36, height: 5, borderRadius: 3, backgroundColor: color.border.base },
  webContent: { flex: 1 },
}));
