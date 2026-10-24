//! WIC image decode -> D2D bitmap, plus cover-art average colour (port of `avgColor` in ui/lib.js).
//! COM must already be initialised on the calling thread.
use windows::core::Interface;
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;
use windows::Win32::Graphics::Direct2D::ID2D1DeviceContext;
use windows::Win32::Graphics::Imaging::*;
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};

/// RGBA pixels -> mean [r,g,b], skipping translucent (a < 128), near-black (max < 24) and
/// near-white (min > 232) pixels; `None` if no pixel qualifies. Trailing partial pixel ignored.
pub fn avg_color(rgba: &[u8]) -> Option<[u8; 3]> {
    let (mut r, mut g, mut b, mut n) = (0u64, 0u64, 0u64, 0u64);
    for p in rgba.chunks_exact(4) {
        if p[3] < 128 {
            continue;
        }
        let max = p[0].max(p[1]).max(p[2]);
        let min = p[0].min(p[1]).min(p[2]);
        if max < 24 || min > 232 {
            continue;
        }
        r += p[0] as u64;
        g += p[1] as u64;
        b += p[2] as u64;
        n += 1;
    }
    if n == 0 {
        return None;
    }
    // f64::round is half-away-from-zero == JS Math.round for non-negative values.
    let m = |s: u64| (s as f64 / n as f64).round() as u8;
    Some([m(r), m(g), m(b)])
}

/// Decode an encoded image (PNG/JPEG/BMP/...) into a premultiplied BGRA D2D bitmap and the
/// cover tint. The tint is `None` when `avg_color` finds no usable pixel (old UI then removed `--art`).
/// Tint is sampled from an 8x8 Fant downscale, like the old 8x8 canvas.
pub fn decode_to_bitmap(
    dc: &ID2D1DeviceContext,
    bytes: &[u8],
) -> Option<(ID2D1Bitmap1, Option<[u8; 3]>)> {
    unsafe {
        let factory: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).ok()?;
        let stream = factory.CreateStream().ok()?;
        stream.InitializeFromMemory(bytes).ok()?; // `bytes` outlives `stream` (both local)
        let decoder = factory
            .CreateDecoderFromStream(&stream, std::ptr::null(), WICDecodeMetadataCacheOnDemand)
            .ok()?;
        let frame = decoder.GetFrame(0).ok()?;

        let pbgra = factory.CreateFormatConverter().ok()?;
        pbgra
            .Initialize(
                &frame,
                &GUID_WICPixelFormat32bppPBGRA,
                WICBitmapDitherTypeNone,
                None::<&IWICPalette>,
                0.0,
                WICBitmapPaletteTypeCustom,
            )
            .ok()?;
        let bitmap = dc.CreateBitmapFromWicBitmap(&pbgra, None).ok()?;

        let tint = (|| -> Option<[u8; 3]> {
            let small = factory.CreateBitmapScaler().ok()?;
            small
                .Initialize(&pbgra, 8, 8, WICBitmapInterpolationModeFant)
                .ok()?;
            let rgba = factory.CreateFormatConverter().ok()?; // straight (non-premultiplied) alpha
            rgba.Initialize(
                &small,
                &GUID_WICPixelFormat32bppRGBA,
                WICBitmapDitherTypeNone,
                None::<&IWICPalette>,
                0.0,
                WICBitmapPaletteTypeCustom,
            )
            .ok()?;
            let mut px = [0u8; 8 * 8 * 4];
            rgba.CopyPixels(std::ptr::null(), 8 * 4, &mut px).ok()?;
            avg_color(&px)
        })();

        Some((bitmap, tint))
    }
}

#[cfg(test)]
mod tests {
    use super::avg_color;

    #[test]
    fn averages_and_rounds_half_up() {
        // (255,0,0) + (0,0,255) -> (127.5, 0, 127.5) -> [128, 0, 128]
        assert_eq!(avg_color(&[255, 0, 0, 255, 0, 0, 255, 255]), Some([128, 0, 128]));
    }

    #[test]
    fn skips_near_black_and_near_white() {
        assert_eq!(avg_color(&[0, 0, 0, 255, 255, 255, 255, 255]), None);
        // boundaries: max 23 skipped, max 24 kept; min 233 skipped, min 232 kept
        assert_eq!(avg_color(&[23, 0, 0, 255]), None);
        assert_eq!(avg_color(&[24, 0, 0, 255]), Some([24, 0, 0]));
        assert_eq!(avg_color(&[233, 233, 233, 255]), None);
        assert_eq!(avg_color(&[232, 232, 232, 255]), Some([232, 232, 232]));
    }

    #[test]
    fn skips_translucent() {
        assert_eq!(avg_color(&[10, 200, 10, 0]), None);
        assert_eq!(avg_color(&[10, 200, 10, 127]), None);
        assert_eq!(avg_color(&[10, 200, 10, 128]), Some([10, 200, 10]));
    }

    #[test]
    fn empty_and_partial_pixel() {
        assert_eq!(avg_color(&[]), None);
        assert_eq!(avg_color(&[200, 100, 50]), None);
        assert_eq!(avg_color(&[200, 100, 50, 255, 9, 9, 9]), Some([200, 100, 50]));
    }
}
