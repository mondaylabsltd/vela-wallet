> **History (2026-09-11).** This document described the text-scale architecture of the React Native / Expo app (its style system, `createStyles`, the level table), retired and deleted in spec 039. The living rule is the `--text-scale` token layer in `app-web/vela-wallet/src/lib/tokens/` and the level table in `src/app.html`; the desktop and native shells scale through their platform APIs. Kept as the record of the level names and ratios.

# Text Scale Architecture & Postmortem

> This document records the full implementation journey of Vela Wallet's text scale feature: mistakes made, why each was wrong, and the design principles behind the final solution.  
> Purpose: avoid repeating these mistakes when building similar "global setting → instant UI update" features.

---

## Final Solution (One Sentence)

**Use a `useStyles` hook that recomputes styles inside the component via `useMemo(factory, [version])` — no `key`-based remounting anywhere.**

---

## 1. Requirements

User adjusts text size (A-/A+) in Settings and expects:
- All text on the current screen updates **instantly** (same frame)
- **Zero flicker** on both iOS and Android
- Other screens reflect the new size when navigated to

---

## 2. Technical Background

### React Native Style System
- `StyleSheet.create()` returns registered IDs (integers), not CSS objects
- Components only read new style props during a **re-render**
- React Navigation memoizes screen components — they don't re-render just because a parent re-renders

### createStyles Proxy
```ts
const styles = createStyles(() => ({
  title: { fontSize: text.xl }
}));
```
- Defined at module level, returns a Proxy object
- Proxy's `get` trap checks `_styleVersion`; if changed, calls `factory()` to rebuild
- **Prerequisite**: the component must re-render for the Proxy's `get` to fire

### React Context Propagation Rules
- Context value changes → all `useContext` consumers re-render
- `React.memo` does NOT block context-triggered re-renders
- **However**: when a parent re-renders, children passed via the `children` prop do **not** automatically re-render (they are JSX elements created by the grandparent; if the reference is unchanged, React skips them)

---

## 3. Mistakes Made (Chronological)

### Mistake 1: `key={version}` on the entire Stack
```tsx
// ❌ Initial approach
<Stack key={version}>
```
**Problem**: Key change unmounts and remounts the entire navigation tree (all screens, all state). White flash on both platforms.  
**Lesson**: `key` is a nuclear tool. Before using it, always ask: "Do I actually need to unmount/remount, or do I just need a re-render?"

### Mistake 2: Crossfade animation to mask the flash
```tsx
// ❌ Wrapped Android Stack in Animated.View, fade out → swap key → fade in
opacity.value = withTiming(0, { duration: 120 });
setTimeout(() => setRenderedVersion(version), 130);
```
**Problems**:
- `setTimeout` and animation duration are not synchronized
- The key change still triggers React's unmount/remount under the fade — layout rebuild is still visible
- iOS had no protection at all

**Lesson**: Don't use animation to mask architectural problems. The root cause of flicker is "unmount/remount." The fix should be "don't unmount," not "unmount but hide it with a fade."

### Mistake 3: Moving `key={version}` to ScreenContainer's SafeAreaView
```tsx
// ❌ Smaller scope, but still key-based remounting
<SafeAreaView key={version}>
  {children}
</SafeAreaView>
```
**Problem**: Navigation stack no longer remounts, but screen content still does. Still flickers.  
**Lesson**: `key` at any level causes subtree unmount/remount. There is no such thing as a "safe key rebuild."

### Mistake 4: Removing key but relying on Proxy auto-update
```tsx
// ❌ Expected the Proxy to return fresh styles during re-render
export function ScreenContainer({ children }) {
  useTextScale(); // subscribe to context
  return <View><SafeAreaView>{children}</SafeAreaView></View>;
}
```
**Problem**:
- ScreenContainer does re-render (it consumes the context)
- But `{children}` is JSX created by the parent screen component
- If the screen component itself doesn't re-render, children reference is unchanged, React skips them
- Even when the screen does re-render, the Proxy-based `StyleSheet.create` result didn't reliably trigger visual updates

**Lesson**: Don't assume "the Proxy will just work." Verify the complete data flow: who triggers the re-render → who accesses styles → whether new styles are actually applied.

### Mistake 5: `await AsyncStorage` blocking state updates
```tsx
// ❌ await before setState
const change = async (delta) => {
  await AsyncStorage.setItem(key, value);  // ← pauses here
  rebuildTextScale();                       // ← delayed
  setVersion(v => v + 1);                   // ← delayed
};
```
**Problem**: `await` pushes subsequent code to a microtask. React's synchronous batching cannot cover `setState` calls inside async callbacks, so the style rebuild and re-render don't happen in the same frame.  
**Lesson**: **UI first, I/O second.** Any operation that affects visuals must complete synchronously. Persistence goes to the background.

### Mistake 6: Splitting styles — some instant, some delayed
```tsx
// ❌ Main screen uses useStyles (instant), sub-components use createStyles (delayed)
const styles = useStyles(mainFactory);      // titles, buttons → instant
const subStyles = createStyles(subFactory); // rows, modals → not updating
```
**Problem**: User sees titles change but row content stays the same. Visually inconsistent.  
**Lesson**: **Style updates must be all-or-nothing within a screen.** Don't mix instant and delayed mechanisms. Visual consistency within a screen is more important than engineering convenience.

---

## 4. Correct Solution

### 4.1 Data Flow

```
User taps A+
  ↓ synchronous
rebuildTextScale()          → text.xl = 20, _styleVersion = 2
  ↓ synchronous
setLevelIndex(next)         → schedule re-render
setVersion(v => v + 1)      → schedule re-render
  ↓ synchronous
AsyncStorage.setItem(...)   → background persistence (no await)
  ↓ React batch processing
TextScaleProvider re-render → context value changes
  ↓ context propagation
SettingsScreen re-render    → useStyles detects version changed
  ↓ useMemo recomputes
StyleSheet.create(factory()) → reads new text.xl = 20 → new style objects
  ↓ props passed to children
All sub-components receive new styles → render with new font sizes in same frame
```

### 4.2 Key Code

**text-scale.ts** — synchronous update, async persistence:
```ts
const change = useCallback((delta: number) => {
  // 1. Rebuild synchronously FIRST
  rebuildTextScale();
  // 2. Trigger re-render synchronously (React batches these)
  setLevelIndex(next);
  setVersion(v => v + 1);
  // 3. Persist in background — never block the UI
  AsyncStorage.setItem(STORAGE_KEY, level.key).catch(() => {});
}, [levelIndex]);
```

**theme.ts** — useStyles hook:
```ts
export function useStyles<T>(factory: () => T): T {
  const { version } = useTextScale();
  return useMemo(() => StyleSheet.create(factory()), [version]);
}
```

**SettingsScreen.tsx** — single factory, passed via prop:
```tsx
const styleFactory = () => ({
  // ALL styles in one place — no split between main/sub
  screenTitle: { fontSize: text['2xl'], ... },
  settingsRowTitle: { fontSize: text.lg, ... },
  modalTitle: { fontSize: text.xl, ... },
});

export default function SettingsScreen() {
  const styles = useStyles(styleFactory);
  return (
    <>
      <SettingsRow s={styles} ... />
      <AccountModal s={styles} ... />
    </>
  );
}
```

### 4.3 Other Screens

Screens outside Settings (HomeScreen, TokenDetail, etc.) don't need instant updates. They:
- Continue using module-level `createStyles` (Proxy-based)
- Pick up new text sizes on next **focus** (`useFocusEffect` triggers state change → re-render) or **mount** (modals re-open)
- Users don't perceive any delay because they must leave Settings first

---

## 5. Design Rules (Must Follow for Future Features)

### Rule 1: Map the full re-render chain before writing code

Before implementing any "change setting → UI updates instantly" feature, draw the complete chain:
```
Who triggers the change → Who re-renders → Who reads new values → How values reach the UI
```
Verify every arrow. Don't assume "it should re-render."

### Rule 2: Never use `key` for style refresh

`key` change = unmount + remount. This means:
- All child component state is lost
- All animations reset
- Layout recalculation may cause flicker
- List scroll positions are lost

Correct approach: make the component **re-render** (not remount) and read fresh values during render.

### Rule 3: UI first, I/O second

```ts
// ✅ Correct order
updateInMemoryState();     // synchronous
triggerReRender();         // synchronous
persistToStorage();        // async, no await

// ❌ Wrong order
await persistToStorage();  // blocks
updateInMemoryState();     // delayed
triggerReRender();         // delayed
```

### Rule 4: Use `useStyles` for screens that need instant feedback

```ts
// Settings, theme picker, etc. → useStyles
const styles = useStyles(factory);

// Regular display screens → createStyles
const styles = createStyles(() => ({...}));
```

### Rule 5: One screen, one style source

Don't mix `useStyles` and `createStyles` in the same screen. Either all styles update instantly or all update lazily. Mixing causes visual tearing.

### Rule 6: Sub-components receive styles via props

When using `useStyles`, the hook can only be called inside a component. Sub-components receive the styles object via a prop:
```tsx
function SubComponent({ s }: { s: Styles }) {
  return <Text style={s.title}>...</Text>;
}
```
Don't let sub-components reference module-level `styles` or call `useStyles` themselves.

### Rule 7: Don't use animation to mask architectural flaws

If a transition or update flickers, first check whether components are unnecessarily unmounting/remounting. The fix for flicker is **eliminating the remount**, not **hiding the remount behind a fade**.

---

## 6. Applicability

These rules apply not just to text scaling, but to all "global setting → instant UI response" scenarios:
- Theme switching (light/dark)
- Language switching
- Currency unit switching
- Any feature requiring "change a setting → global UI responds instantly"

The core pattern is always the same: **synchronous in-memory update → Context triggers re-render → read fresh values during render**.
