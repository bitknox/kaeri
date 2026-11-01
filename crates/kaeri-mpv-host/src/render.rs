//! The video, drawn by mpv into the app's own OpenGL framebuffer through libmpv's render API.
//!
//! Toolkit-agnostic: the app hands in how to look up GL functions and, for hardware decoding's
//! interop, its native display; then it draws when told a frame is due. A GTK `GtkGLArea` does
//! that from its `render` signal; another toolkit's GL widget would do the same.
//!
//! Every call on a [`GlRenderer`], and dropping it, has to happen on the thread whose GL context
//! is current, with that context current: libmpv's rule for the render API.

#![allow(unsafe_code)]

use std::ffi::{c_char, c_void};

use crate::{
    mpv::{Mpv, MpvError, c_text, opaque},
    sys,
};

/// Looks up a GL function by name, as `eglGetProcAddress` does: null for none.
pub type GetProcAddress = Box<dyn Fn(&str) -> *mut c_void>;

/// The display the GL context is on, which hardware decoding's interop needs to share frames
/// with it: VA-API's EGL import reads it. `None` decodes in software or copies back.
#[derive(Debug, Clone, Copy)]
pub enum NativeDisplay {
    None,
    /// An X11 `Display *`.
    X11(*mut c_void),
    /// A Wayland `struct wl_display *`.
    Wayland(*mut c_void),
}

/// Called when mpv has a new frame to draw, on one of mpv's threads: it must not call mpv, only
/// ask the app's main loop to redraw.
pub type UpdateCallback = Box<dyn Fn() + Send + Sync>;

/// A render context on a [`Mpv`] core.
pub struct GlRenderer {
    ctx: *mut sys::mpv_render_context,
    /// Kept for as long as the context: libmpv may ask it for functions after the create.
    _loader: Box<GetProcAddress>,
    update: Option<Box<UpdateCallback>>,
    /// Holds the core up until the context is freed.
    _mpv: Mpv,
}

unsafe extern "C" fn get_proc_address(ctx: *mut c_void, name: *const c_char) -> *mut c_void {
    // SAFETY: `ctx` is the boxed loader the renderer keeps alive; `name` is mpv's C string.
    unsafe {
        let loader = &*ctx.cast::<GetProcAddress>();
        let name = c_text(name);
        if name == "glTexImage2D" {
            return float_uploads::wrap(loader);
        }
        loader(name)
    }
}

/// libmpv (0.40, and upstream since) fills its scaler weight tables from memory it never
/// clears: a filter with six taps, lanczos for one, gets rows of eight floats, and the last two
/// of each row hold whatever was on the heap. The shader never reads those two, but the table is
/// sampled with linear filtering, and a neighbour weighted by zero still counts: zero times NaN
/// or infinity is NaN. When the heap held such a value, every frame of the session comes out
/// black. llvmpipe always does that maths; GPUs differ. So mpv's float texture uploads go
/// through here, and any value a half-float texture cannot hold becomes zero.
mod float_uploads {
    use std::{
        ffi::c_void,
        sync::atomic::{AtomicPtr, Ordering},
    };

    use super::GetProcAddress;

    const GL_FLOAT: u32 = 0x1406;
    const GL_RED: u32 = 0x1903;
    const GL_RG: u32 = 0x8227;
    const GL_RGB: u32 = 0x1907;
    const GL_RGBA: u32 = 0x1908;
    const GL_UNPACK_ROW_LENGTH: u32 = 0x0CF2;
    const GL_UNPACK_SKIP_ROWS: u32 = 0x0CF3;
    const GL_UNPACK_SKIP_PIXELS: u32 = 0x0CF4;
    const GL_PIXEL_UNPACK_BUFFER_BINDING: u32 = 0x88EF;
    /// The largest finite half float.
    const HALF_MAX: f32 = 65504.0;
    /// Weight tables are a few thousand floats; anything this big is not one, and passes as is.
    const MOST: usize = 1 << 16;

    type TexImage2D =
        unsafe extern "C" fn(u32, i32, i32, i32, i32, i32, u32, u32, *const c_void);
    type GetIntegerv = unsafe extern "C" fn(u32, *mut i32);

    static REAL: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
    static GET_INTEGER: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

    /// The wrapped `glTexImage2D`, or the loader's own answer when it has none.
    pub(super) fn wrap(loader: &GetProcAddress) -> *mut c_void {
        let real = loader("glTexImage2D");
        let get = loader("glGetIntegerv");
        if real.is_null() || get.is_null() {
            return real;
        }
        // The same functions for every context: EGL hands out one entry point per name.
        REAL.store(real, Ordering::Release);
        GET_INTEGER.store(get, Ordering::Release);
        tex_image_2d as *mut c_void
    }

    /// Zeroes what a half float cannot hold: NaN, the infinities and anything past its range.
    /// The count changed.
    pub(super) fn clean(values: &mut [f32]) -> usize {
        let mut changed = 0;
        for v in values.iter_mut().filter(|v| !v.is_finite() || v.abs() > HALF_MAX) {
            *v = 0.0;
            changed += 1;
        }
        changed
    }

    fn components(format: u32) -> Option<usize> {
        match format {
            GL_RED => Some(1),
            GL_RG => Some(2),
            GL_RGB => Some(3),
            GL_RGBA => Some(4),
            _ => None,
        }
    }

    #[allow(clippy::too_many_arguments)]
    unsafe extern "C" fn tex_image_2d(
        target: u32,
        level: i32,
        internal: i32,
        width: i32,
        height: i32,
        border: i32,
        format: u32,
        kind: u32,
        pixels: *const c_void,
    ) {
        // SAFETY: stored by `wrap` before mpv could call this, and never cleared.
        let real: TexImage2D = unsafe { std::mem::transmute(REAL.load(Ordering::Acquire)) };
        // SAFETY: as above, the GL function of that name.
        let get: GetIntegerv =
            unsafe { std::mem::transmute(GET_INTEGER.load(Ordering::Acquire)) };
        let count = components(format)
            .zip(usize::try_from(width).ok())
            .zip(usize::try_from(height).ok())
            .map(|((c, w), h)| c * w * h);
        let tight = || {
            [
                GL_PIXEL_UNPACK_BUFFER_BINDING,
                GL_UNPACK_ROW_LENGTH,
                GL_UNPACK_SKIP_ROWS,
                GL_UNPACK_SKIP_PIXELS,
            ]
            .into_iter()
            .all(|state| {
                let mut value = 0;
                // SAFETY: the context mpv calls from is current.
                unsafe { get(state, &mut value) };
                value == 0
            })
        };
        match count {
            // Read from client memory, packed: copied, cleaned and handed on.
            Some(n) if kind == GL_FLOAT && !pixels.is_null() && n <= MOST && tight() => {
                // SAFETY: GL reads `n` floats from `pixels` for this call, so they are there.
                let mut values =
                    unsafe { std::slice::from_raw_parts(pixels.cast::<f32>(), n) }.to_vec();
                if clean(&mut values) > 0 {
                    tracing::debug!(
                        width,
                        height,
                        "mpv uploaded floats a half float cannot hold"
                    );
                }
                // SAFETY: the same call with a copy of the data.
                unsafe {
                    real(
                        target,
                        level,
                        internal,
                        width,
                        height,
                        border,
                        format,
                        kind,
                        values.as_ptr().cast(),
                    )
                };
            }
            // SAFETY: mpv's call, passed on unchanged.
            _ => unsafe {
                real(target, level, internal, width, height, border, format, kind, pixels)
            },
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn clean_zeroes_only_what_a_half_float_cannot_hold() {
            let mut values = [
                0.25,
                -0.136,
                f32::NAN,
                f32::INFINITY,
                f32::NEG_INFINITY,
                1.0e30,
                HALF_MAX,
                -HALF_MAX,
            ];
            assert_eq!(clean(&mut values), 4);
            assert_eq!(values, [0.25, -0.136, 0.0, 0.0, 0.0, 0.0, HALF_MAX, -HALF_MAX]);
        }

        static SEEN: std::sync::Mutex<Vec<f32>> = std::sync::Mutex::new(Vec::new());

        #[allow(clippy::too_many_arguments)]
        unsafe extern "C" fn fake_tex_image_2d(
            _: u32,
            _: i32,
            _: i32,
            width: i32,
            height: i32,
            _: i32,
            _: u32,
            _: u32,
            pixels: *const c_void,
        ) {
            let n = 4 * width as usize * height as usize;
            // SAFETY: the test hands in `n` floats.
            let values = unsafe { std::slice::from_raw_parts(pixels.cast::<f32>(), n) };
            *SEEN.lock().unwrap() = values.to_vec();
        }

        unsafe extern "C" fn fake_get_integerv(_: u32, value: *mut i32) {
            // SAFETY: GL's contract: one int written.
            unsafe { *value = 0 };
        }

        #[test]
        fn a_weight_table_reaches_gl_without_nan_in_its_padding() {
            let loader: GetProcAddress = Box::new(|name| match name {
                "glTexImage2D" => fake_tex_image_2d as *mut c_void,
                "glGetIntegerv" => fake_get_integerv as *mut c_void,
                _ => std::ptr::null_mut(),
            });
            let wrapped = wrap(&loader);
            assert_ne!(wrapped, fake_tex_image_2d as *mut c_void);
            // SAFETY: `wrap` answered with a function of glTexImage2D's signature.
            let tex_image_2d: TexImage2D = unsafe { std::mem::transmute(wrapped) };
            // One row of a six-tap table: two texels of four, the last two floats left over.
            let row = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, f32::NAN, f32::INFINITY];
            // SAFETY: the fakes stand in for GL; `row` is two RGBA float texels.
            unsafe {
                tex_image_2d(0x0DE1, 0, 0x881A, 2, 1, 0, GL_RGBA, GL_FLOAT, row.as_ptr().cast())
            };
            assert_eq!(*SEEN.lock().unwrap(), [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.0, 0.0]);
        }

        #[test]
        fn a_loader_without_the_functions_is_answered_as_is() {
            let none: GetProcAddress = Box::new(|_| std::ptr::null_mut());
            assert!(wrap(&none).is_null());
        }
    }
}

unsafe extern "C" fn on_update(ctx: *mut c_void) {
    // SAFETY: `ctx` is the boxed callback the renderer keeps alive until it unsets it.
    unsafe {
        let callback = &*ctx.cast::<UpdateCallback>();
        callback();
    }
}

impl GlRenderer {
    /// A render context on `mpv`, using the GL context current on this thread.
    ///
    /// # Safety
    ///
    /// A GL context must be current on this thread, and every later call and the drop must be
    /// made on it with it current. A display in `display` must outlive the renderer.
    ///
    /// `advanced_control` turns on libmpv's direct rendering, at the price of its rules: the
    /// update callback must be answered with [`Self::frame_due`] on the GL thread every time,
    /// soon, and the GL thread must never wait on the core.
    pub unsafe fn new(
        mpv: &Mpv,
        loader: GetProcAddress,
        display: NativeDisplay,
        advanced_control: bool,
    ) -> Result<Self, MpvError> {
        let loader = Box::new(loader);
        let mut init = sys::mpv_opengl_init_params {
            get_proc_address: Some(get_proc_address),
            get_proc_address_ctx: opaque(&*loader),
        };
        let mut advanced: i32 = advanced_control.into();
        let mut params = vec![
            sys::mpv_render_param {
                type_: sys::MPV_RENDER_PARAM_API_TYPE,
                data: sys::MPV_RENDER_API_TYPE_OPENGL.as_ptr().cast_mut().cast(),
            },
            sys::mpv_render_param {
                type_: sys::MPV_RENDER_PARAM_OPENGL_INIT_PARAMS,
                data: (&raw mut init).cast(),
            },
            sys::mpv_render_param {
                type_: sys::MPV_RENDER_PARAM_ADVANCED_CONTROL,
                data: (&raw mut advanced).cast(),
            },
        ];
        match display {
            NativeDisplay::None => {}
            NativeDisplay::X11(ptr) => params.push(sys::mpv_render_param {
                type_: sys::MPV_RENDER_PARAM_X11_DISPLAY,
                data: ptr,
            }),
            NativeDisplay::Wayland(ptr) => params.push(sys::mpv_render_param {
                type_: sys::MPV_RENDER_PARAM_WL_DISPLAY,
                data: ptr,
            }),
        }
        params.push(sys::mpv_render_param {
            type_: sys::MPV_RENDER_PARAM_INVALID,
            data: std::ptr::null_mut(),
        });
        let mut ctx = std::ptr::null_mut();
        // SAFETY: the params are valid for the call, the loader for the context's life; the
        // caller vouches for the GL context.
        let code = unsafe {
            sys::mpv_render_context_create(&raw mut ctx, mpv.raw(), params.as_mut_ptr())
        };
        if code < 0 {
            return Err(MpvError { code, call: "mpv_render_context_create".into() });
        }
        Ok(Self { ctx, _loader: loader, update: None, _mpv: mpv.clone() })
    }

    /// Have `callback` called whenever a frame is due. It replaces any before it.
    pub fn set_update_callback(&mut self, callback: impl Fn() + Send + Sync + 'static) {
        let boxed: Box<UpdateCallback> = Box::new(Box::new(callback));
        // SAFETY: the box lives in `self.update` until it is replaced or the context is freed;
        // libmpv stops calling the old one before this returns.
        unsafe {
            sys::mpv_render_context_set_update_callback(
                self.ctx,
                Some(on_update),
                opaque(&*boxed),
            );
        }
        self.update = Some(boxed);
    }

    /// Whether a new frame is due: call it after each update callback, on the GL thread with the
    /// context current, and draw when it says so.
    pub fn frame_due(&self) -> bool {
        // SAFETY: live context, on its GL thread (the caller's promise at `new`).
        unsafe { sys::mpv_render_context_update(self.ctx) & sys::MPV_RENDER_UPDATE_FRAME != 0 }
    }

    /// Draw the current frame into framebuffer `fbo`, `width` by `height` pixels; flipped for
    /// a framebuffer whose origin is at the bottom left, as GL's own are.
    pub fn render(
        &self,
        fbo: i32,
        width: i32,
        height: i32,
        flip_y: bool,
    ) -> Result<(), MpvError> {
        let mut target = sys::mpv_opengl_fbo { fbo, w: width, h: height, internal_format: 0 };
        let mut flip: i32 = flip_y.into();
        let mut params = [
            sys::mpv_render_param {
                type_: sys::MPV_RENDER_PARAM_OPENGL_FBO,
                data: (&raw mut target).cast(),
            },
            sys::mpv_render_param {
                type_: sys::MPV_RENDER_PARAM_FLIP_Y,
                data: (&raw mut flip).cast(),
            },
            sys::mpv_render_param {
                type_: sys::MPV_RENDER_PARAM_INVALID,
                data: std::ptr::null_mut(),
            },
        ];
        // SAFETY: live context on its GL thread; params valid for the call.
        let code = unsafe { sys::mpv_render_context_render(self.ctx, params.as_mut_ptr()) };
        if code < 0 {
            Err(MpvError { code, call: "mpv_render_context_render".into() })
        } else {
            Ok(())
        }
    }

    /// The frame drawn has been put on screen: mpv's frame timing reads it.
    pub fn report_swap(&self) {
        // SAFETY: live context.
        unsafe { sys::mpv_render_context_report_swap(self.ctx) }
    }
}

impl Drop for GlRenderer {
    fn drop(&mut self) {
        // SAFETY: live context, freed once, on its GL thread (the caller's promise at `new`).
        // Freeing it stops the update callback first.
        unsafe { sys::mpv_render_context_free(self.ctx) }
    }
}
