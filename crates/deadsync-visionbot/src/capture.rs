//! Windows.Graphics.Capture (WGC) screen capture of the DeadSync game window.
//!
//! This module is the only place that talks to live OS capture APIs; everything
//! downstream consumes an owned [`OwnedFrame`] / borrowed [`FrameView`] and is
//! platform-independent and unit-tested.
//!
//! # Time base
//!
//! All bot-side timestamps are expressed in **100 ns ticks** (the unit of
//! `Windows::Foundation::TimeSpan`). A WGC frame's `SystemRelativeTime` is
//! already in this unit, and [`now_ticks`] converts a `QueryPerformanceCounter`
//! reading into the same unit, so frame times and "now" are directly
//! comparable. The matching tick frequency for the scheduler is
//! [`TICKS_PER_SECOND`] (`10_000_000`).
//!
//! # Design notes (per review)
//!
//! * The frame pool is created with `CreateFreeThreaded`, so frames can be
//!   pulled with `TryGetNextFrame` from any thread without a `DispatcherQueue`
//!   or window message pump — appropriate for a console app.
//! * WinRT is initialized in the multithreaded apartment on first use.
//! * The D3D11 device is created with `BGRA_SUPPORT`; frames are copied to a
//!   CPU-readable **staging** texture and mapped to yield tightly described
//!   BGRA bytes (stride preserved from the GPU row pitch).
//! * [`Capturer::poll_newest`] drains the queue and returns only the newest
//!   frame, and callers should drop frames whose `timestamp` is far behind
//!   [`now_ticks`] (stale) — capture jitter must never feed the timing fit.
//! * A content-size change recreates the staging texture automatically.

use std::sync::Once;

use windows::Foundation::TimeSpan;
use windows::Graphics::Capture::{
    Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession,
};
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::SizeInt32;
use windows::Win32::Foundation::{HMODULE, HWND};
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
    D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE, D3D11_SDK_VERSION,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
use windows::Win32::System::WinRT::Direct3D11::{
    CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};
use windows::Win32::UI::WindowsAndMessaging::FindWindowW;
use windows::core::{HSTRING, Interface, Result as WResult};

use crate::frame::{FrameView, PixelFormat};

/// Tick frequency for bot-side timestamps (100 ns ticks → ticks per second).
pub const TICKS_PER_SECOND: i64 = 10_000_000;

static WINRT_INIT: Once = Once::new();

fn ensure_winrt() {
    WINRT_INIT.call_once(|| {
        // Multithreaded apartment; ignore errors (S_FALSE if already inited,
        // RPC_E_CHANGED_MODE if another apartment was chosen first).
        unsafe {
            let _ = RoInitialize(RO_INIT_MULTITHREADED);
        }
    });
}

/// `QueryPerformanceFrequency` (raw QPC ticks per second).
pub fn qpc_frequency() -> i64 {
    let mut f = 0i64;
    unsafe {
        let _ = QueryPerformanceFrequency(&mut f);
    }
    if f == 0 { 1 } else { f }
}

/// Current time in **100 ns ticks**, matching a frame's `SystemRelativeTime`.
///
/// `qpc_freq` should be the value from [`qpc_frequency`] (cached by the caller).
pub fn now_ticks(qpc_freq: i64) -> i64 {
    let mut c = 0i64;
    unsafe {
        let _ = QueryPerformanceCounter(&mut c);
    }
    // ticks_100ns = counter * 10_000_000 / frequency, done in i128 to avoid overflow.
    ((c as i128 * TICKS_PER_SECOND as i128) / qpc_freq as i128) as i64
}

/// Find a top-level window by its exact title. Returns `None` if not found.
pub fn find_window_by_title(title: &str) -> Option<HWND> {
    let wide = HSTRING::from(title);
    let hwnd = unsafe { FindWindowW(None, &wide) };
    match hwnd {
        Ok(h) if !h.is_invalid() => Some(h),
        _ => None,
    }
}

/// An owned, CPU-side BGRA frame ready for detection.
pub struct OwnedFrame {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// Bytes per row (GPU staging row pitch; `>= width * 4`).
    pub stride: usize,
    /// Capture time in 100 ns ticks (`SystemRelativeTime`).
    pub timestamp: i64,
}

impl OwnedFrame {
    /// Borrow this frame as a [`FrameView`] for the detector.
    pub fn view(&self) -> FrameView<'_> {
        FrameView::new(
            &self.data,
            self.width,
            self.height,
            self.stride,
            PixelFormat::Bgra8,
            self.timestamp,
        )
    }
}

/// Live WGC capturer bound to a single window.
pub struct Capturer {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    _item: GraphicsCaptureItem,
    frame_pool: Direct3D11CaptureFramePool,
    session: GraphicsCaptureSession,
    /// Lazily (re)created CPU-readable staging texture matching the frame size.
    staging: Option<ID3D11Texture2D>,
    staging_w: u32,
    staging_h: u32,
    qpc_freq: i64,
    last_timestamp: i64,
}

impl Capturer {
    /// Begin capturing the given window.
    pub fn new(hwnd: HWND) -> WResult<Self> {
        ensure_winrt();

        let (device, context) = create_d3d_device()?;
        let dxgi: IDXGIDevice = device.cast()?;
        let inspectable = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi)? };
        let d3d_device: IDirect3DDevice = inspectable.cast()?;

        // HWND -> GraphicsCaptureItem via the interop factory.
        let interop: IGraphicsCaptureItemInterop =
            windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;
        let item: GraphicsCaptureItem = unsafe { interop.CreateForWindow(hwnd)? };
        let size: SizeInt32 = item.Size()?;

        let frame_pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &d3d_device,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            2,
            size,
        )?;
        let session = frame_pool.CreateCaptureSession(&item)?;
        session.StartCapture()?;

        Ok(Self {
            device,
            context,
            _item: item,
            frame_pool,
            session,
            staging: None,
            staging_w: 0,
            staging_h: 0,
            qpc_freq: qpc_frequency(),
            last_timestamp: i64::MIN,
        })
    }

    /// QPC frequency (raw ticks/sec) captured at construction.
    pub fn qpc_freq(&self) -> i64 {
        self.qpc_freq
    }

    /// "Now" in 100 ns ticks, comparable to [`OwnedFrame::timestamp`].
    pub fn now(&self) -> i64 {
        now_ticks(self.qpc_freq)
    }

    /// Drain the frame queue and return the **newest** available frame, or
    /// `None` if no new frame is ready. Frames whose timestamp duplicates the
    /// previous one are skipped.
    pub fn poll_newest(&mut self) -> WResult<Option<OwnedFrame>> {
        // Pull the newest queued frame. TryGetNextFrame returns the oldest in
        // the queue, so loop (bounded) to reach the freshest one.
        let mut newest: Option<windows::Graphics::Capture::Direct3D11CaptureFrame> = None;
        for _ in 0..16 {
            match self.frame_pool.TryGetNextFrame() {
                Ok(frame) => newest = Some(frame),
                Err(_) => break,
            }
        }
        let Some(frame) = newest else {
            return Ok(None);
        };

        let timestamp: TimeSpan = frame.SystemRelativeTime()?;
        let ts = timestamp.Duration;
        if ts == self.last_timestamp {
            return Ok(None);
        }

        let surface = frame.Surface()?;
        let access: IDirect3DDxgiInterfaceAccess = surface.cast()?;
        let texture: ID3D11Texture2D = unsafe { access.GetInterface()? };

        let mut desc = D3D11_TEXTURE2D_DESC::default();
        unsafe { texture.GetDesc(&mut desc) };

        self.ensure_staging(desc.Width, desc.Height)?;
        let staging = self
            .staging
            .clone()
            .expect("staging created by ensure_staging");

        unsafe {
            self.context.CopyResource(&staging, &texture);
        }

        let owned = self.map_staging(&staging, desc.Width, desc.Height, ts)?;
        self.last_timestamp = ts;
        Ok(Some(owned))
    }

    fn ensure_staging(&mut self, width: u32, height: u32) -> WResult<()> {
        if self.staging.is_some() && self.staging_w == width && self.staging_h == height {
            return Ok(());
        }
        let desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
        };
        let mut tex: Option<ID3D11Texture2D> = None;
        unsafe {
            self.device
                .CreateTexture2D(&desc, None, Some(&mut tex))?;
        }
        self.staging = tex;
        self.staging_w = width;
        self.staging_h = height;
        Ok(())
    }

    fn map_staging(
        &self,
        staging: &ID3D11Texture2D,
        width: u32,
        height: u32,
        timestamp: i64,
    ) -> WResult<OwnedFrame> {
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {
            self.context
                .Map(staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;
        }
        let stride = mapped.RowPitch as usize;
        let mut data = vec![0u8; stride * height as usize];
        unsafe {
            let src = mapped.pData as *const u8;
            std::ptr::copy_nonoverlapping(src, data.as_mut_ptr(), data.len());
            self.context.Unmap(staging, 0);
        }
        Ok(OwnedFrame {
            data,
            width,
            height,
            stride,
            timestamp,
        })
    }
}

impl Drop for Capturer {
    fn drop(&mut self) {
        let _ = self.session.Close();
        let _ = self.frame_pool.Close();
    }
}

fn create_d3d_device() -> WResult<(ID3D11Device, ID3D11DeviceContext)> {
    let mut device: Option<ID3D11Device> = None;
    let mut context: Option<ID3D11DeviceContext> = None;
    let mut feature_level = D3D_FEATURE_LEVEL::default();
    unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            Some(&mut feature_level),
            Some(&mut context),
        )?;
    }
    Ok((
        device.expect("D3D11CreateDevice yielded no device"),
        context.expect("D3D11CreateDevice yielded no context"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_ticks_is_monotonic_and_in_100ns() {
        let f = qpc_frequency();
        assert!(f > 0);
        let a = now_ticks(f);
        let b = now_ticks(f);
        assert!(b >= a);
    }

    #[test]
    fn missing_window_returns_none() {
        assert!(find_window_by_title("a window that does not exist 9z9z9z").is_none());
    }
}
