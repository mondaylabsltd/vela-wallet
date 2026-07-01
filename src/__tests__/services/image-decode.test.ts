import { decodeBase64Image } from '@/services/image-decode';
import jpeg from 'jpeg-js';

describe('decodeBase64Image', () => {
  test('decodes JPEG files for the jsQR fallback', () => {
    const encoded = jpeg.encode({
      data: Buffer.from([255, 255, 255, 255]),
      width: 1,
      height: 1,
    }, 90);

    const image = decodeBase64Image(encoded.data.toString('base64'));

    expect(image).not.toBeNull();
    expect(image).toMatchObject({ width: 1, height: 1 });
    expect(image?.data).toHaveLength(4);
  });

  test('leaves PNG files to the native gallery scanner', () => {
    // PNG signature plus a few bytes is enough to exercise format selection.
    const png = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);

    expect(decodeBase64Image(png.toString('base64'))).toBeNull();
  });

  test('returns null for an unknown image format', () => {
    expect(decodeBase64Image(Buffer.from('not an image').toString('base64'))).toBeNull();
  });
});
