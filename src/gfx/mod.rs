//! Direct2D renderer: D3D11 device -> D2D device context -> GDI-compatible BGRA bitmap ->
//! `UpdateLayeredWindow` (per-pixel alpha, anti-aliased edges).
pub mod anim;
pub mod icons;
pub mod image;
pub mod svg;
pub mod text;

use windows::core::Interface;
use windows::Win32::Foundation::{COLORREF, HMODULE, HWND, POINT, SIZE};
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::Graphics::Gdi::{AC_SRC_ALPHA, AC_SRC_OVER, BLENDFUNCTION};
use windows::Win32::UI::WindowsAndMessaging::{UpdateLayeredWindow, ULW_ALPHA};

pub struct Gfx {
    pub factory: ID2D1Factory1,
    d3d: ID3D11Device,
    device: ID2D1Device,
    pub dc: ID2D1DeviceContext,
    bitmap: ID2D1Bitmap1,
    gdi: ID2D1GdiInteropRenderTarget,
    pub w: u32,
    pub h: u32,
}

fn create_d3d() -> windows::core::Result<ID3D11Device> {
    let mut dev = None;
    for driver in [D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP] {
        let r = unsafe {
            D3D11CreateDevice(
                None::<&windows::Win32::Graphics::Dxgi::IDXGIAdapter>,
                driver,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut dev),
                None,
                None,
            )
        };
        if r.is_ok() && dev.is_some() {
            break;
        }
    }
    dev.ok_or_else(|| windows::core::Error::from(windows::Win32::Foundation::E_FAIL))
}

fn create_bitmap(dc: &ID2D1DeviceContext, w: u32, h: u32) -> windows::core::Result<ID2D1Bitmap1> {
    let props = D2D1_BITMAP_PROPERTIES1 {
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
        },
        dpiX: 96.0,
        dpiY: 96.0,
        bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_GDI_COMPATIBLE,
        colorContext: std::mem::ManuallyDrop::new(None),
    };
    unsafe { dc.CreateBitmap(D2D_SIZE_U { width: w, height: h }, None, 0, &props) }
}

impl Gfx {
    pub fn new(w: u32, h: u32) -> windows::core::Result<Gfx> {
        unsafe {
            let factory: ID2D1Factory1 =
                D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let d3d = create_d3d()?;
            let dxgi: IDXGIDevice = d3d.cast()?;
            let device = factory.CreateDevice(&dxgi)?;
            let dc = device.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)?;
            let bitmap = create_bitmap(&dc, w, h)?;
            dc.SetTarget(&bitmap);
            let gdi: ID2D1GdiInteropRenderTarget = dc.cast()?;
            Ok(Gfx { factory, d3d, device, dc, bitmap, gdi, w, h })
        }
    }

    /// Rebuilds the render target after a window resize / DPI change.
    pub fn resize(&mut self, w: u32, h: u32) -> windows::core::Result<()> {
        if (w, h) == (self.w, self.h) {
            return Ok(());
        }
        unsafe {
            self.dc.SetTarget(None);
            self.bitmap = create_bitmap(&self.dc, w, h)?;
            self.dc.SetTarget(&self.bitmap);
        }
        self.w = w;
        self.h = h;
        Ok(())
    }

    /// Recreates device resources after `D2DERR_RECREATE_TARGET` / device loss.
    pub fn rebuild(&mut self) -> windows::core::Result<()> {
        *self = Gfx::new(self.w, self.h)?;
        Ok(())
    }

    pub fn begin(&self, scale: f32) {
        unsafe {
            self.dc.BeginDraw();
            self.dc.Clear(Some(&D2D1_COLOR_F { r: 0.0, g: 0.0, b: 0.0, a: 0.0 }));
            self.dc.SetTransform(&windows_numerics_scale(scale));
        }
    }

    /// Ends the frame and pushes it to the layered window at screen position `(x, y)`.
    /// Returns `Err` with `D2DERR_RECREATE_TARGET` when the device must be rebuilt.
    pub fn present(&self, hwnd: HWND, x: i32, y: i32) -> windows::core::Result<()> {
        unsafe {
            let hdc = self.gdi.GetDC(D2D1_DC_INITIALIZE_MODE_COPY)?;
            let pt = POINT { x, y };
            let size = SIZE { cx: self.w as i32, cy: self.h as i32 };
            let src = POINT { x: 0, y: 0 };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            let r = UpdateLayeredWindow(
                hwnd,
                None,
                Some(&pt),
                Some(&size),
                Some(hdc),
                Some(&src),
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            );
            let _ = self.gdi.ReleaseDC(None);
            self.dc.EndDraw(None, None)?;
            r
        }
    }

    pub fn solid(&self, r: f32, g: f32, b: f32, a: f32) -> windows::core::Result<ID2D1SolidColorBrush> {
        unsafe { self.dc.CreateSolidColorBrush(&D2D1_COLOR_F { r, g, b, a }, None) }
    }

    /// Keeps the D3D device alive for the lifetime of the renderer.
    pub fn _d3d(&self) -> &ID3D11Device {
        &self.d3d
    }
    pub fn _device(&self) -> &ID2D1Device {
        &self.device
    }
}

fn windows_numerics_scale(s: f32) -> windows_numerics::Matrix3x2 {
    windows_numerics::Matrix3x2 { M11: s, M12: 0.0, M21: 0.0, M22: s, M31: 0.0, M32: 0.0 }
}

/// The island silhouette: flat top with concave "ears", convex rounded bottom corners.
/// All coordinates in logical px, `left` = x of the island's left edge.
pub fn island_path(
    f: &ID2D1Factory1,
    left: f32,
    w: f32,
    h: f32,
    r: f32,
    e: f32,
) -> windows::core::Result<ID2D1PathGeometry1> {
    let right = left + w;
    let r = r.clamp(0.0, (w / 2.0).min(h));
    unsafe {
        let g = f.CreatePathGeometry()?;
        let s = g.Open()?;
        s.SetFillMode(D2D1_FILL_MODE_WINDING);
        let pt = |x: f32, y: f32| windows_numerics::Vector2 { X: x, Y: y };
        s.BeginFigure(pt(left - e, 0.0), D2D1_FIGURE_BEGIN_FILLED);
        if e > 0.0 {
            s.AddArc(&D2D1_ARC_SEGMENT {
                point: pt(left, e),
                size: D2D_SIZE_F { width: e, height: e },
                rotationAngle: 0.0,
                sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                arcSize: D2D1_ARC_SIZE_SMALL,
            });
        } else {
            s.AddLine(pt(left, 0.0));
        }
        s.AddLine(pt(left, (h - r).max(e)));
        if r > 0.0 {
            s.AddArc(&D2D1_ARC_SEGMENT {
                point: pt(left + r, h),
                size: D2D_SIZE_F { width: r, height: r },
                rotationAngle: 0.0,
                sweepDirection: D2D1_SWEEP_DIRECTION_COUNTER_CLOCKWISE,
                arcSize: D2D1_ARC_SIZE_SMALL,
            });
        }
        s.AddLine(pt(right - r, h));
        if r > 0.0 {
            s.AddArc(&D2D1_ARC_SEGMENT {
                point: pt(right, h - r),
                size: D2D_SIZE_F { width: r, height: r },
                rotationAngle: 0.0,
                sweepDirection: D2D1_SWEEP_DIRECTION_COUNTER_CLOCKWISE,
                arcSize: D2D1_ARC_SIZE_SMALL,
            });
        }
        s.AddLine(pt(right, e));
        if e > 0.0 {
            s.AddArc(&D2D1_ARC_SEGMENT {
                point: pt(right + e, 0.0),
                size: D2D_SIZE_F { width: e, height: e },
                rotationAngle: 0.0,
                sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                arcSize: D2D1_ARC_SIZE_SMALL,
            });
        }
        s.EndFigure(D2D1_FIGURE_END_CLOSED);
        s.Close()?;
        Ok(g)
    }
}
