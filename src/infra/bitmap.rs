//! Turns raw pixels into a BMP file that WinUI can show.

const FILE_HEADER_SIZE: u32 = 14;
const INFO_HEADER_SIZE: u32 = 124;
const BI_BITFIELDS: u32 = 3;
const LCS_SRGB: u32 = 0x7352_4742;
const LCS_GM_IMAGES: u32 = 4;
/// 72 dots per inch.
const PIXELS_PER_METER: u32 = 2835;

/// Encodes 32-bit BGRA pixels as a BMP file with an alpha channel.
///
/// `bgra` holds the rows from top to bottom. Its length must be `width * height * 4`.
/// When every pixel has an alpha of 0, the image is taken as opaque. Some Windows bitmaps have
/// no alpha at all and leave the byte at 0.
pub fn encode_bmp(width: u32, height: u32, bgra: &[u8]) -> Vec<u8> {
    let row = width as usize * 4;
    assert_eq!(
        bgra.len(),
        row * height as usize,
        "the pixels must be width * height * 4 bytes"
    );
    let opaque = bgra.as_chunks::<4>().0.iter().all(|pixel| pixel[3] == 0);
    let image_size = bgra.len() as u32;
    let offset = FILE_HEADER_SIZE + INFO_HEADER_SIZE;

    let mut out = Vec::with_capacity((offset + image_size) as usize);
    // BITMAPFILEHEADER
    out.extend_from_slice(b"BM");
    put(&mut out, offset + image_size);
    put(&mut out, 0);
    put(&mut out, offset);
    // BITMAPV5HEADER. A positive height means the rows are stored from bottom to top.
    put(&mut out, INFO_HEADER_SIZE);
    put(&mut out, width);
    put(&mut out, height);
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    put(&mut out, BI_BITFIELDS);
    put(&mut out, image_size);
    put(&mut out, PIXELS_PER_METER);
    put(&mut out, PIXELS_PER_METER);
    put(&mut out, 0);
    put(&mut out, 0);
    for mask in [0x00FF_0000, 0x0000_FF00, 0x0000_00FF, 0xFF00_0000] {
        put(&mut out, mask);
    }
    put(&mut out, LCS_SRGB);
    // The endpoints and the gamma are not used with sRGB.
    out.extend_from_slice(&[0; 48]);
    put(&mut out, LCS_GM_IMAGES);
    put(&mut out, 0);
    put(&mut out, 0);
    put(&mut out, 0);

    for line in bgra.chunks_exact(row.max(1)).rev() {
        if opaque {
            for pixel in line.as_chunks::<4>().0 {
                out.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 0xFF]);
            }
        } else {
            out.extend_from_slice(line);
        }
    }
    out
}

fn put(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u32_at(bytes: &[u8], at: usize) -> u32 {
        u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
    }

    // BMPのヘッダーには、ファイルの大きさ、画素の位置、幅と高さが書かれる。
    #[test]
    fn header_holds_the_file_size_the_pixel_offset_and_the_size() {
        let bmp = encode_bmp(2, 3, &[1; 2 * 3 * 4]);

        assert_eq!(&bmp[0..2], b"BM");
        assert_eq!(u32_at(&bmp, 2) as usize, bmp.len());
        assert_eq!(u32_at(&bmp, 10), 138);
        assert_eq!(bmp.len(), 138 + 24);
        assert_eq!(u32_at(&bmp, 14), 124);
        assert_eq!(u32_at(&bmp, 18), 2);
        assert_eq!(u32_at(&bmp, 22), 3);
        assert_eq!(u32_at(&bmp, 34), 24);
    }

    // 画素は、下の行から順に書かれる。
    #[test]
    fn rows_are_written_from_bottom_to_top() {
        let top = [1, 2, 3, 128];
        let bottom = [4, 5, 6, 255];

        let bmp = encode_bmp(1, 2, &[top, bottom].concat());

        assert_eq!(&bmp[138..], [bottom, top].concat());
    }

    // すべての画素のアルファが0なら、不透明として書かれる。
    #[test]
    fn pixels_without_any_alpha_are_written_as_opaque() {
        let bmp = encode_bmp(2, 1, &[1, 2, 3, 0, 4, 5, 6, 0]);

        assert_eq!(&bmp[138..], [1, 2, 3, 255, 4, 5, 6, 255]);
    }

    // 画素の数が幅と高さに合わないと、止まる。
    #[test]
    #[should_panic]
    fn pixels_that_do_not_match_the_size_are_rejected() {
        encode_bmp(2, 2, &[0; 4]);
    }
}
