//! Direct2D renderer: D3D11 device -> D2D device context -> target bitmap -> CPU readback of the
//! dirty rect into a DIB -> `UpdateLayeredWindowIndirect` (per-pixel alpha, anti-aliased edges).
pub mod anim;
pub mod image;
pub mod svg;
pub mod text;

use std::cell::Cell;
use windows::core::Interface;
use windows::Win32::Foundation::{COLORREF, HMODULE, HWND, POINT, RECT, SIZE};
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::{UpdateLayeredWindowIndirect, ULW_ALPHA, UPDATELAYEREDWINDOWINFO};

pub struct Gfx {
    pub factory: ID2D1Factory1,
    d3d: ID3D11Device,
    device: ID2D1Device,
    pub dc: ID2D1DeviceContext,
    bitmap: ID2D1Bitmap1,
    /// CPU-readable staging copy of `bitmap`.
    cpu: ID2D1Bitmap1,
    mem_dc: HDC,
    dib: HBITMAP,
    /// Top-down BGRA pixels of `dib`, `w * 4` bytes per row.
    bits: *mut u8,
    clipped: Cell<bool>,
    pub w: u32,
    pub h: u32,
}

impl Drop for Gfx {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteDC(self.mem_dc);
            let _ = DeleteObject(self.dib.into());
        }
    }
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

fn bitmap_props(options: D2D1_BITMAP_OPTIONS) -> D2D1_BITMAP_PROPERTIES1 {
    D2D1_BITMAP_PROPERTIES1 {
        pixelFormat: D2D1_PIXEL_FORMAT { format: DXGI_FORMAT_B8G8R8A8_UNORM, alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED },
        dpiX: 96.0,
        dpiY: 96.0,
        bitmapOptions: options,
        colorContext: std::mem::ManuallyDrop::new(None),
    }
}

type Surfaces = (ID2D1Bitmap1, ID2D1Bitmap1, HDC, HBITMAP, *mut u8);

/// Target bitmap, CPU staging bitmap and a top-down 32-bit DIB selected into a memory DC.
fn surfaces(dc: &ID2D1DeviceContext, w: u32, h: u32) -> windows::core::Result<Surfaces> {
    unsafe {
        let size = D2D_SIZE_U { width: w, height: h };
        let bitmap = dc.CreateBitmap(size, None, 0, &bitmap_props(D2D1_BITMAP_OPTIONS_TARGET))?;
        let cpu = dc.CreateBitmap(size, None, 0, &bitmap_props(D2D1_BITMAP_OPTIONS_CPU_READ | D2D1_BITMAP_OPTIONS_CANNOT_DRAW))?;
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w as i32,
                biHeight: -(h as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits = std::ptr::null_mut();
        let dib = CreateDIBSection(None, &bmi, DIB_RGB_COLORS, &mut bits, None, 0)?;
        let mem_dc = CreateCompatibleDC(None);
        SelectObject(mem_dc, dib.into());
        Ok((bitmap, cpu, mem_dc, dib, bits as *mut u8))
    }
}

impl Gfx {
    pub fn new(w: u32, h: u32) -> windows::core::Result<Gfx> {
        unsafe {
            let factory: ID2D1Factory1 = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let d3d = create_d3d()?;
            let dxgi: IDXGIDevice = d3d.cast()?;
            let device = factory.CreateDevice(&dxgi)?;
            let dc = device.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)?;
            let (bitmap, cpu, mem_dc, dib, bits) = surfaces(&dc, w, h)?;
            dc.SetTarget(&bitmap);
            dc.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
            Ok(Gfx { factory, d3d, device, dc, bitmap, cpu, mem_dc, dib, bits, clipped: Cell::new(false), w, h })
        }
    }

    /// Rebuilds the render target after a window resize / DPI change.
    pub fn resize(&mut self, w: u32, h: u32) -> windows::core::Result<()> {
        if (w, h) == (self.w, self.h) {
            return Ok(());
        }
        unsafe {
            self.dc.SetTarget(None);
            let (bitmap, cpu, mem_dc, dib, bits) = surfaces(&self.dc, w, h)?;
            let _ = DeleteDC(self.mem_dc);
            let _ = DeleteObject(self.dib.into());
            (self.bitmap, self.cpu, self.mem_dc, self.dib, self.bits) = (bitmap, cpu, mem_dc, dib, bits);
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

    /// Starts a frame. `dirty` = `[left, top, right, bottom]` device pixels to redraw; pixels
    /// outside it keep the previous frame.
    pub fn begin(&self, scale: f32, dirty: Option<[u32; 4]>) {
        unsafe {
            self.dc.BeginDraw();
            self.dc.SetTransform(&windows_numerics_scale(1.0));
            if let Some(r) = dirty {
                let rect = D2D_RECT_F { left: r[0] as f32, top: r[1] as f32, right: r[2] as f32, bottom: r[3] as f32 };
                self.dc.PushAxisAlignedClip(&rect, D2D1_ANTIALIAS_MODE_ALIASED);
                self.clipped.set(true);
            }
            self.dc.Clear(Some(&D2D1_COLOR_F { r: 0.0, g: 0.0, b: 0.0, a: 0.0 }));
            self.dc.SetTransform(&windows_numerics_scale(scale));
        }
    }

    /// Ends the frame and copies device rect `r` into the DIB.
    fn finish(&self, r: [u32; 4]) -> windows::core::Result<()> {
        unsafe {
            self.end()?;
            if r[2] <= r[0] || r[3] <= r[1] {
                return Ok(());
            }
            self.cpu.CopyFromBitmap(
                Some(&D2D_POINT_2U { x: r[0], y: r[1] }),
                &self.bitmap,
                Some(&D2D_RECT_U { left: r[0], top: r[1], right: r[2], bottom: r[3] }),
            )?;
            let m = self.cpu.Map(D2D1_MAP_OPTIONS_READ)?;
            let (x0, n) = (r[0] as usize * 4, (r[2] - r[0]) as usize * 4);
            for y in r[1] as usize..r[3] as usize {
                let src = m.bits.add(y * m.pitch as usize + x0);
                let dst = self.bits.add(y * self.w as usize * 4 + x0);
                std::ptr::copy_nonoverlapping(src, dst, n);
            }
            self.cpu.Unmap()
        }
    }

    /// Ends the frame and pushes it to the layered window at screen position `(x, y)`; only
    /// `dirty` (device pixels) is read back and updated. Returns `Err` with
    /// `D2DERR_RECREATE_TARGET` when the device must be rebuilt.
    pub fn present(&self, hwnd: HWND, x: i32, y: i32, dirty: Option<[u32; 4]>) -> windows::core::Result<()> {
        self.finish(dirty.unwrap_or([0, 0, self.w, self.h]))?;
        let pt = POINT { x, y };
        let size = SIZE { cx: self.w as i32, cy: self.h as i32 };
        let src = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION { BlendOp: AC_SRC_OVER as u8, BlendFlags: 0, SourceConstantAlpha: 255, AlphaFormat: AC_SRC_ALPHA as u8 };
        let rc = dirty.map(|r| RECT { left: r[0] as i32, top: r[1] as i32, right: r[2] as i32, bottom: r[3] as i32 });
        let mut info = UPDATELAYEREDWINDOWINFO {
            cbSize: std::mem::size_of::<UPDATELAYEREDWINDOWINFO>() as u32,
            hdcDst: HDC::default(),
            pptDst: &pt,
            psize: &size,
            hdcSrc: self.mem_dc,
            pptSrc: &src,
            crKey: COLORREF(0),
            pblend: &blend,
            dwFlags: ULW_ALPHA,
            prcDirty: rc.as_ref().map_or(std::ptr::null(), |r| r as *const RECT),
        };
        unsafe {
            let r = UpdateLayeredWindowIndirect(hwnd, &info).ok();
            if r.is_err() && rc.is_some() {
                // Some drivers reject partial updates: retry the same frame in full.
                info.prcDirty = std::ptr::null();
                return UpdateLayeredWindowIndirect(hwnd, &info).ok();
            }
            r
        }
    }

    /// Ends the frame and blits it to a normal (non-layered) window's client area.
    pub fn present_blit(&self, hwnd: HWND) -> windows::core::Result<()> {
        self.finish([0, 0, self.w, self.h])?;
        unsafe {
            let dc = GetDC(Some(hwnd));
            let _ = BitBlt(dc, 0, 0, self.w as i32, self.h as i32, Some(self.mem_dc), 0, 0, SRCCOPY);
            ReleaseDC(Some(hwnd), dc);
        }
        Ok(())
    }

    /// Ends a frame without presenting (snapshot rendering).
    pub fn end(&self) -> windows::core::Result<()> {
        unsafe {
            if self.clipped.take() {
                self.dc.PopAxisAlignedClip();
            }
            self.dc.EndDraw(None, None)
        }
    }

    /// A second device context on the same device, for rendering into command lists.
    pub fn aux(&self) -> windows::core::Result<ID2D1DeviceContext> {
        unsafe {
            let dc = self.device.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)?;
            dc.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
            Ok(dc)
        }
    }

    /// Straight-alpha RGBA copy of the last rendered frame (call after `end`).
    pub fn read_rgba(&self) -> windows::core::Result<Vec<u8>> {
        unsafe {
            let cpu = self.dc.CreateBitmap(
                D2D_SIZE_U { width: self.w, height: self.h },
                None,
                0,
                &bitmap_props(D2D1_BITMAP_OPTIONS_CANNOT_DRAW | D2D1_BITMAP_OPTIONS_CPU_READ),
            )?;
            cpu.CopyFromBitmap(None, &self.bitmap, None)?;
            let m = cpu.Map(D2D1_MAP_OPTIONS_READ)?;
            let mut out = Vec::with_capacity((self.w * self.h * 4) as usize);
            for y in 0..self.h {
                let row = std::slice::from_raw_parts(m.bits.add((y * m.pitch) as usize), (self.w * 4) as usize);
                for px in row.chunks_exact(4) {
                    let a = px[3] as u32;
                    let un = |c: u8| if a == 0 { 0 } else { ((c as u32 * 255 + a / 2) / a).min(255) as u8 };
                    out.extend_from_slice(&[un(px[2]), un(px[1]), un(px[0]), px[3]]);
                }
            }
            cpu.Unmap()?;
            Ok(out)
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
