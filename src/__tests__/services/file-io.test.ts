/**
 * saveTextFile — issue #79: on Android the file must be savable to device
 * storage (Storage Access Framework), not only shareable. Everything is lazy-
 * imported inside the function, so these mocks resolve only when it runs.
 */
const platform = { OS: 'android' as string };
jest.mock('react-native', () => ({ Platform: platform }));

const saf = {
  requestDirectoryPermissionsAsync: jest.fn(),
  createFileAsync: jest.fn(),
  writeAsStringAsync: jest.fn(),
};
jest.mock('expo-file-system/legacy', () => ({ StorageAccessFramework: saf }));

const fileWrite = jest.fn();
const fileCreate = jest.fn();
jest.mock('expo-file-system', () => ({
  File: jest.fn().mockImplementation(() => ({ create: fileCreate, write: fileWrite, uri: 'file:///cache/out' })),
  Paths: { cache: 'file:///cache' },
}));

const shareAsync = jest.fn(async () => {});
const isAvailableAsync = jest.fn(async () => true);
jest.mock('expo-sharing', () => ({ isAvailableAsync, shareAsync }));

import { saveTextFile } from '@/services/file-io';

beforeEach(() => {
  jest.clearAllMocks();
  platform.OS = 'android';
  saf.requestDirectoryPermissionsAsync.mockResolvedValue({ granted: true, directoryUri: 'content://tree/downloads' });
  saf.createFileAsync.mockResolvedValue('content://tree/downloads/doc');
  saf.writeAsStringAsync.mockResolvedValue(undefined);
});

describe('saveTextFile', () => {
  it('Android: writes to the user-picked folder via SAF and does NOT share', async () => {
    await saveTextFile('vela-payroll-template.csv', 'a,b,c', 'text/csv');
    expect(saf.requestDirectoryPermissionsAsync).toHaveBeenCalled();
    expect(saf.createFileAsync).toHaveBeenCalledWith('content://tree/downloads', 'vela-payroll-template.csv', 'text/csv');
    expect(saf.writeAsStringAsync).toHaveBeenCalledWith('content://tree/downloads/doc', 'a,b,c');
    expect(shareAsync).not.toHaveBeenCalled();
  });

  it('Android: falls back to the share sheet when the folder picker is cancelled', async () => {
    saf.requestDirectoryPermissionsAsync.mockResolvedValue({ granted: false, directoryUri: '' });
    await saveTextFile('vela-payroll-template.csv', 'x', 'text/csv');
    expect(saf.createFileAsync).not.toHaveBeenCalled();
    expect(fileWrite).toHaveBeenCalledWith('x');
    expect(shareAsync).toHaveBeenCalled();
  });

  it('Android: falls back to share when SAF throws (unavailable)', async () => {
    saf.requestDirectoryPermissionsAsync.mockRejectedValue(new Error('no SAF'));
    await saveTextFile('f.csv', 'x', 'text/csv');
    expect(shareAsync).toHaveBeenCalled();
  });

  it('iOS: never touches SAF; shares (share sheet includes Save to Files)', async () => {
    platform.OS = 'ios';
    await saveTextFile('f.csv', 'x', 'text/csv');
    expect(saf.requestDirectoryPermissionsAsync).not.toHaveBeenCalled();
    expect(fileWrite).toHaveBeenCalledWith('x');
    expect(shareAsync).toHaveBeenCalled();
  });
});
