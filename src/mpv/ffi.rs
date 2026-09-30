#![allow(non_camel_case_types, dead_code)]

use std::ffi::{c_char, c_double, c_int, c_uint, c_void};

pub enum mpv_handle {}
pub enum mpv_render_context {}

pub const MPV_FORMAT_NONE: c_int = 0;
pub const MPV_FORMAT_STRING: c_int = 1;
pub const MPV_FORMAT_OSD_STRING: c_int = 2;
pub const MPV_FORMAT_FLAG: c_int = 3;
pub const MPV_FORMAT_INT64: c_int = 4;
pub const MPV_FORMAT_DOUBLE: c_int = 5;
pub const MPV_FORMAT_NODE: c_int = 6;
pub const MPV_FORMAT_NODE_ARRAY: c_int = 7;
pub const MPV_FORMAT_NODE_MAP: c_int = 8;
pub const MPV_FORMAT_BYTE_ARRAY: c_int = 9;

pub const MPV_EVENT_NONE: c_int = 0;
pub const MPV_EVENT_SHUTDOWN: c_int = 1;
pub const MPV_EVENT_LOG_MESSAGE: c_int = 2;
pub const MPV_EVENT_GET_PROPERTY_REPLY: c_int = 3;
pub const MPV_EVENT_SET_PROPERTY_REPLY: c_int = 4;
pub const MPV_EVENT_COMMAND_REPLY: c_int = 5;
pub const MPV_EVENT_START_FILE: c_int = 6;
pub const MPV_EVENT_END_FILE: c_int = 7;
pub const MPV_EVENT_FILE_LOADED: c_int = 8;
pub const MPV_EVENT_VIDEO_RECONFIG: c_int = 17;
pub const MPV_EVENT_SEEK: c_int = 20;
pub const MPV_EVENT_PLAYBACK_RESTART: c_int = 21;
pub const MPV_EVENT_PROPERTY_CHANGE: c_int = 22;

pub const MPV_END_FILE_REASON_EOF: c_int = 0;
pub const MPV_END_FILE_REASON_STOP: c_int = 2;
pub const MPV_END_FILE_REASON_QUIT: c_int = 3;
pub const MPV_END_FILE_REASON_ERROR: c_int = 4;
pub const MPV_END_FILE_REASON_REDIRECT: c_int = 5;

pub const MPV_RENDER_PARAM_INVALID: c_int = 0;
pub const MPV_RENDER_PARAM_API_TYPE: c_int = 1;
pub const MPV_RENDER_PARAM_OPENGL_INIT_PARAMS: c_int = 2;
pub const MPV_RENDER_PARAM_OPENGL_FBO: c_int = 3;
pub const MPV_RENDER_PARAM_FLIP_Y: c_int = 4;
pub const MPV_RENDER_PARAM_WL_DISPLAY: c_int = 9;
pub const MPV_RENDER_PARAM_ADVANCED_CONTROL: c_int = 10;
pub const MPV_RENDER_PARAM_BLOCK_FOR_TARGET_TIME: c_int = 12;
pub const MPV_RENDER_PARAM_SW_SIZE: c_int = 17;
pub const MPV_RENDER_PARAM_SW_FORMAT: c_int = 18;
pub const MPV_RENDER_PARAM_SW_STRIDE: c_int = 19;
pub const MPV_RENDER_PARAM_SW_POINTER: c_int = 20;

pub const MPV_RENDER_UPDATE_FRAME: u64 = 1 << 0;

pub const GL_DRAW_FRAMEBUFFER_BINDING: c_uint = 0x8CA6;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct mpv_event {
    pub event_id: c_int,
    pub error: c_int,
    pub reply_userdata: u64,
    pub data: *mut c_void,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct mpv_event_property {
    pub name: *const c_char,
    pub format: c_int,
    pub data: *mut c_void,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct mpv_event_end_file {
    pub reason: c_int,
    pub error: c_int,
    pub playlist_entry_id: i64,
    pub playlist_insert_id: i64,
    pub playlist_insert_num_entries: c_int,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct mpv_event_log_message {
    pub prefix: *const c_char,
    pub level: *const c_char,
    pub text: *const c_char,
    pub log_level: c_int,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct mpv_render_param {
    pub type_: c_int,
    pub data: *mut c_void,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct mpv_opengl_init_params {
    pub get_proc_address: Option<unsafe extern "C" fn(ctx: *mut c_void, name: *const c_char) -> *mut c_void>,
    pub get_proc_address_ctx: *mut c_void,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct mpv_opengl_fbo {
    pub fbo: c_int,
    pub w: c_int,
    pub h: c_int,
    pub internal_format: c_int,
}

unsafe extern "C" {
    pub fn mpv_create() -> *mut mpv_handle;
    pub fn mpv_initialize(ctx: *mut mpv_handle) -> c_int;
    pub fn mpv_terminate_destroy(ctx: *mut mpv_handle);
    pub fn mpv_error_string(error: c_int) -> *const c_char;
    pub fn mpv_free(data: *mut c_void);

    pub fn mpv_set_option_string(
        ctx: *mut mpv_handle,
        name: *const c_char,
        data: *const c_char,
    ) -> c_int;

    pub fn mpv_command(ctx: *mut mpv_handle, args: *const *const c_char) -> c_int;
    pub fn mpv_command_async(
        ctx: *mut mpv_handle,
        reply_userdata: u64,
        args: *const *const c_char,
    ) -> c_int;

    pub fn mpv_set_property(
        ctx: *mut mpv_handle,
        name: *const c_char,
        format: c_int,
        data: *const c_void,
    ) -> c_int;
    pub fn mpv_set_property_string(
        ctx: *mut mpv_handle,
        name: *const c_char,
        data: *const c_char,
    ) -> c_int;
    pub fn mpv_set_property_async(
        ctx: *mut mpv_handle,
        reply_userdata: u64,
        name: *const c_char,
        format: c_int,
        data: *const c_void,
    ) -> c_int;

    pub fn mpv_get_property(
        ctx: *mut mpv_handle,
        name: *const c_char,
        format: c_int,
        data: *mut c_void,
    ) -> c_int;
    pub fn mpv_get_property_string(ctx: *mut mpv_handle, name: *const c_char) -> *mut c_char;

    pub fn mpv_observe_property(
        mpv: *mut mpv_handle,
        reply_userdata: u64,
        name: *const c_char,
        format: c_int,
    ) -> c_int;

    pub fn mpv_wait_event(ctx: *mut mpv_handle, timeout: c_double) -> *mut mpv_event;
    pub fn mpv_set_wakeup_callback(
        ctx: *mut mpv_handle,
        cb: unsafe extern "C" fn(*mut c_void),
        d: *mut c_void,
    );

    pub fn mpv_render_context_create(
        res: *mut *mut mpv_render_context,
        mpv: *mut mpv_handle,
        params: *mut mpv_render_param,
    ) -> c_int;
    pub fn mpv_render_context_set_update_callback(
        ctx: *mut mpv_render_context,
        callback: unsafe extern "C" fn(*mut c_void),
        callback_ctx: *mut c_void,
    );
    pub fn mpv_render_context_update(ctx: *mut mpv_render_context) -> u64;
    pub fn mpv_render_context_render(
        ctx: *mut mpv_render_context,
        params: *mut mpv_render_param,
    ) -> c_int;
    pub fn mpv_render_context_report_swap(ctx: *mut mpv_render_context);
    pub fn mpv_render_context_free(ctx: *mut mpv_render_context);

    pub fn eglGetProcAddress(procname: *const c_char) -> *mut c_void;
    pub fn glXGetProcAddressARB(procname: *const u8) -> *mut c_void;
    pub fn glGetIntegerv(pname: c_uint, data: *mut c_int);
    pub fn gdk_display_get_default() -> *mut c_void;
    pub fn gdk_wayland_display_get_wl_display(display: *mut c_void) -> *mut c_void;
}

pub unsafe extern "C" fn get_proc_address(
    _ctx: *mut c_void,
    name: *const c_char,
) -> *mut c_void {
    if name.is_null() {
        return std::ptr::null_mut();
    }
    let sym = unsafe { eglGetProcAddress(name) };
    if !sym.is_null() {
        return sym;
    }
    let sym_dlsym = unsafe { libc::dlsym(libc::RTLD_DEFAULT, name) };
    if !sym_dlsym.is_null() {
        return sym_dlsym;
    }
    unsafe { glXGetProcAddressARB(name as *const u8) }
}
