/**
 * Tests for platform.ts web fallbacks — the module exists so callers never scatter
 * Platform.OS checks, and so web "gracefully degrades". Focus on the web branch:
 * the styled AppAlert path + its window.confirm/alert fallback, and clipboard's
 * navigator → execCommand fallback.
 */
jest.mock('react-native', () => ({ Platform: { OS: 'web' } }));
jest.mock('@/components/ui/AppAlert', () => ({ getGlobalShowAlert: jest.fn() }));

import { Platform } from 'react-native';
import { showAlert, copyToClipboard, hapticSuccess } from '@/services/platform';
import { getGlobalShowAlert } from '@/components/ui/AppAlert';

const mockGlobalShowAlert = getGlobalShowAlert as jest.Mock;

beforeEach(() => {
  jest.clearAllMocks();
  (Platform as unknown as { OS: string }).OS = 'web';
  (global as any).window = { confirm: jest.fn(() => true), alert: jest.fn() };
  (global as any).navigator = { clipboard: { writeText: jest.fn(async () => {}) } };
});

describe('showAlert (web)', () => {
  test('routes to the mounted in-app AppAlert when available', () => {
    const styled = jest.fn();
    mockGlobalShowAlert.mockReturnValue(styled);
    const buttons = [{ text: 'OK' }];
    showAlert('Title', 'Message', buttons);
    expect(styled).toHaveBeenCalledWith('Title', 'Message', buttons);
    expect((global as any).window.alert).not.toHaveBeenCalled();
  });

  test('falls back to window.alert (and fires the button) when no AppAlert is mounted', () => {
    mockGlobalShowAlert.mockReturnValue(null);
    const onPress = jest.fn();
    showAlert('T', 'M', [{ text: 'OK', onPress }]);
    expect((global as any).window.alert).toHaveBeenCalledWith('T\n\nM');
    expect(onPress).toHaveBeenCalled();
  });

  test('multi-button fallback: confirm=true runs the non-cancel action', () => {
    mockGlobalShowAlert.mockReturnValue(null);
    (global as any).window.confirm = jest.fn(() => true);
    const cancel = jest.fn();
    const ok = jest.fn();
    showAlert('T', undefined, [
      { text: 'Cancel', style: 'cancel', onPress: cancel },
      { text: 'Delete', onPress: ok },
    ]);
    expect(ok).toHaveBeenCalled();
    expect(cancel).not.toHaveBeenCalled();
  });

  test('multi-button fallback: confirm=false runs the cancel action', () => {
    mockGlobalShowAlert.mockReturnValue(null);
    (global as any).window.confirm = jest.fn(() => false);
    const cancel = jest.fn();
    const ok = jest.fn();
    showAlert('T', undefined, [
      { text: 'Cancel', style: 'cancel', onPress: cancel },
      { text: 'Delete', onPress: ok },
    ]);
    expect(cancel).toHaveBeenCalled();
    expect(ok).not.toHaveBeenCalled();
  });
});

describe('copyToClipboard (web)', () => {
  test('uses navigator.clipboard when available', async () => {
    await copyToClipboard('hello');
    expect((global as any).navigator.clipboard.writeText).toHaveBeenCalledWith('hello');
  });

  test('falls back to a hidden textarea + execCommand when clipboard API throws', async () => {
    (global as any).navigator.clipboard.writeText = jest.fn(async () => { throw new Error('insecure context'); });
    const textarea: any = { style: {}, select: jest.fn(), value: '' };
    const execCommand = jest.fn();
    (global as any).document = {
      createElement: jest.fn(() => textarea),
      body: { appendChild: jest.fn(), removeChild: jest.fn() },
      execCommand,
    };
    await copyToClipboard('fallback-text');
    expect(textarea.value).toBe('fallback-text');
    expect((global as any).document.body.appendChild).toHaveBeenCalledWith(textarea);
    expect(execCommand).toHaveBeenCalledWith('copy');
    expect((global as any).document.body.removeChild).toHaveBeenCalledWith(textarea);
  });
});

describe('haptics (web)', () => {
  test('hapticSuccess is a no-op on web (never throws, no native import)', () => {
    expect(() => hapticSuccess()).not.toThrow();
  });
});
