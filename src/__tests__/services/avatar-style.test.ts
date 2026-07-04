/**
 * Avatar-style preference — default, persistence round-trip, validation of
 * stored values, and listener notification (what useAvatarStyle hangs off).
 */
import AsyncStorage from '@react-native-async-storage/async-storage';
import {
  getAvatarStyle,
  loadAvatarStyle,
  setAvatarStyle,
  subscribeAvatarStyle,
} from '@/services/avatar-style';

jest.mock('@react-native-async-storage/async-storage', () => {
  const store = new Map<string, string>();
  return {
    getItem: jest.fn(async (k: string) => (store.has(k) ? store.get(k)! : null)),
    setItem: jest.fn(async (k: string, v: string) => { store.set(k, v); }),
    removeItem: jest.fn(async (k: string) => { store.delete(k); }),
    __store: store,
  };
});

const store: Map<string, string> = (AsyncStorage as unknown as { __store: Map<string, string> }).__store;

beforeEach(async () => {
  store.clear();
  await setAvatarStyle('initials');
});

describe('avatar-style service', () => {
  it('defaults to initials', () => {
    expect(getAvatarStyle()).toBe('initials');
  });

  it('persists and reloads the chosen style', async () => {
    await setAvatarStyle('identicon');
    expect(store.get('vela.avatarStyle')).toBe('identicon');

    // Simulate a fresh boot: cache poisoned back to default, then load.
    await setAvatarStyle('initials');
    store.set('vela.avatarStyle', 'identicon');
    await loadAvatarStyle();
    expect(getAvatarStyle()).toBe('identicon');
  });

  it('ignores corrupt stored values', async () => {
    store.set('vela.avatarStyle', 'garbage');
    await loadAvatarStyle();
    expect(getAvatarStyle()).toBe('initials');
  });

  it('notifies subscribers on change, not on no-ops', async () => {
    const seen: string[] = [];
    const unsubscribe = subscribeAvatarStyle(() => seen.push(getAvatarStyle()));

    await setAvatarStyle('identicon');
    await setAvatarStyle('identicon'); // no-op — same value
    await setAvatarStyle('initials');
    expect(seen).toEqual(['identicon', 'initials']);

    unsubscribe();
    await setAvatarStyle('identicon');
    expect(seen).toEqual(['identicon', 'initials']);
  });

  it('keeps the cached style when storage reads fail', async () => {
    await setAvatarStyle('identicon');
    (AsyncStorage.getItem as jest.Mock).mockRejectedValueOnce(new Error('disk'));
    await expect(loadAvatarStyle()).resolves.toBe('identicon');
    expect(getAvatarStyle()).toBe('identicon');
  });

  it('a set during an in-flight load wins over the stored value', async () => {
    store.set('vela.avatarStyle', 'identicon');
    // Hold the read open until we release it, so the set lands mid-load.
    let release!: (v: string) => void;
    (AsyncStorage.getItem as jest.Mock).mockReturnValueOnce(new Promise((r) => { release = r; }));

    const load = loadAvatarStyle();
    await setAvatarStyle('initials');
    release('identicon');
    await load;

    expect(getAvatarStyle()).toBe('initials');
  });
});
