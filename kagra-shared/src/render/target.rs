//! 描画先の指定。プラットフォームごとのハンドルを 1 箇所に閉じ込める。

use std::ffi::c_void;
use std::ptr::NonNull;

use raw_window_handle as rwh;

/// シェルから渡されるサーフェスの正体。
pub enum SurfaceSource {
    /// Android: `ANativeWindow*`（`ANativeWindow_fromSurface` の戻り値）。
    AndroidNativeWindow(*mut c_void),
    /// iOS: `UIView*`（`CAMetalLayer` を backing layer に持つ view）。
    UiKitView(*mut c_void),
    /// Web: 描画先 canvas。
    #[cfg(target_arch = "wasm32")]
    Canvas(web_sys::HtmlCanvasElement),
}

impl SurfaceSource {
    pub(crate) fn create(
        self,
        instance: &wgpu::Instance,
    ) -> Result<wgpu::Surface<'static>, String> {
        match self {
            Self::AndroidNativeWindow(ptr) => {
                let window = NonNull::new(ptr).ok_or("null ANativeWindow")?;
                let handle =
                    rwh::RawWindowHandle::AndroidNdk(rwh::AndroidNdkWindowHandle::new(window));
                let display = rwh::RawDisplayHandle::Android(rwh::AndroidDisplayHandle::new());
                // SAFETY: 呼び出し側がサーフェス破棄まで ANativeWindow を生かす契約
                // （Android シェルは surfaceDestroyed で detach する）。
                unsafe { create_raw(instance, display, handle) }
            }
            Self::UiKitView(ptr) => {
                let view = NonNull::new(ptr).ok_or("null UIView")?;
                let handle = rwh::RawWindowHandle::UiKit(rwh::UiKitWindowHandle::new(view));
                let display = rwh::RawDisplayHandle::UiKit(rwh::UiKitDisplayHandle::new());
                // SAFETY: 上と同じ。UIView は shell 側が保持する。
                unsafe { create_raw(instance, display, handle) }
            }
            #[cfg(target_arch = "wasm32")]
            Self::Canvas(canvas) => instance
                .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
                .map_err(|e| e.to_string()),
        }
    }
}

/// # Safety
/// ハンドルはサーフェスより長生きしなければならない。
unsafe fn create_raw(
    instance: &wgpu::Instance,
    raw_display_handle: rwh::RawDisplayHandle,
    raw_window_handle: rwh::RawWindowHandle,
) -> Result<wgpu::Surface<'static>, String> {
    instance
        .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: Some(raw_display_handle),
            raw_window_handle,
        })
        .map_err(|e| e.to_string())
}
