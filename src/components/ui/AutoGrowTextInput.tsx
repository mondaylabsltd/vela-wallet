/**
 * A multiline TextInput that grows with its content instead of staying at a
 * fixed height and scrolling internally.
 *
 * Why a wrapper: React Native's multiline `TextInput` does NOT auto-size on the
 * web (react-native-web renders a `<textarea>` that keeps its initial height and
 * scrolls). Even on native, growing requires tracking the measured content
 * height. This component unifies both:
 *   - native: drives height from `onContentSizeChange` (which also shrinks).
 *   - web:    re-measures `scrollHeight` after resetting to `auto` so the box
 *             grows AND shrinks as lines are added/removed (a plain explicit
 *             height freezes `scrollHeight` and would never shrink back).
 *
 * Pass `maxHeight` to cap growth and scroll past it; omit it to grow freely
 * (best inside a ScrollView, e.g. the bug-report form). `minHeight` is the
 * resting/empty height.
 */
import React, { forwardRef, useCallback, useLayoutEffect, useRef, useState } from 'react';
import { Platform, TextInput, type TextInputProps } from 'react-native';

export type AutoGrowTextInputProps = TextInputProps & {
  /** Resting height when empty (also the floor the box never shrinks below). */
  minHeight?: number;
  /** Cap growth here and scroll past it. Omit to grow without bound. */
  maxHeight?: number;
};

export const AutoGrowTextInput = forwardRef<TextInput, AutoGrowTextInputProps>(
  function AutoGrowTextInput(
    { minHeight = 44, maxHeight, style, value, onContentSizeChange, ...props },
    ref,
  ) {
    const [height, setHeight] = useState(minHeight);
    const nodeRef = useRef<TextInput | null>(null);

    const clamp = useCallback(
      (h: number) => Math.max(minHeight, maxHeight != null ? Math.min(maxHeight, h) : h),
      [minHeight, maxHeight],
    );

    // Web: an explicitly-set height keeps `scrollHeight` from ever shrinking, so
    // reset to `auto`, read the true content height, then re-apply it (both
    // imperatively — to handle the no-state-change case — and via state).
    useLayoutEffect(() => {
      if (Platform.OS !== 'web') return;
      const node = nodeRef.current as unknown as HTMLTextAreaElement | null;
      if (!node) return;
      node.style.height = 'auto';
      const next = clamp(node.scrollHeight);
      node.style.height = `${next}px`;
      setHeight(next);
    }, [value, clamp]);

    const setRef = useCallback(
      (node: TextInput | null) => {
        nodeRef.current = node;
        if (typeof ref === 'function') ref(node);
        else if (ref) (ref as React.MutableRefObject<TextInput | null>).current = node;
      },
      [ref],
    );

    return (
      <TextInput
        textAlignVertical="top"
        {...props}
        ref={setRef}
        value={value}
        multiline
        scrollEnabled={maxHeight != null}
        onContentSizeChange={(e) => {
          // Native measures (and shrinks) correctly; web is handled above.
          if (Platform.OS !== 'web') setHeight(clamp(e.nativeEvent.contentSize.height));
          onContentSizeChange?.(e);
        }}
        style={[style, { height }]}
      />
    );
  },
);
