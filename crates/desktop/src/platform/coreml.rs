#![allow(unsafe_code)]

use ort::{AsPointer, execution_providers::ExecutionProvider, session::builder::SessionBuilder};
use std::ffi::CStr;

#[derive(Default)]
pub struct CoreMlProgram;

pub fn configure(mut builder: SessionBuilder) -> ort::Result<SessionBuilder> {
    CoreMlProgram.register(&mut builder)?;
    Ok(builder)
}

impl ExecutionProvider for CoreMlProgram {
    fn as_str(&self) -> &'static str {
        "CoreMLExecutionProvider"
    }

    fn register(&self, builder: &mut SessionBuilder) -> ort::Result<()> {
        let api = ort::api();
        let get_message = api
            .GetErrorMessage
            .ok_or_else(|| ort::Error::new("ORT error API unavailable"))?;
        let release = api
            .ReleaseStatus
            .ok_or_else(|| ort::Error::new("ORT status API unavailable"))?;
        // ONNX Runtime 1.20: MLProgram=0x10, CPUAndGPU=0x20. Keep ANE excluded for this backend.
        let flags = 0x10 | 0x20;
        // SAFETY: builder owns valid session options. The pinned CoreML C API copies the flags synchronously and returns an owned status or null.
        let status =
            unsafe { OrtSessionOptionsAppendExecutionProvider_CoreML(builder.ptr_mut(), flags) };
        if status.is_null() {
            return Ok(());
        }
        // SAFETY: a non-null status is owned by this call and its error string remains valid until ReleaseStatus.
        let message = unsafe { CStr::from_ptr(get_message(status)) }
            .to_string_lossy()
            .into_owned();
        // SAFETY: status is released exactly once after copying its message; no references to the status remain.
        unsafe { release(status) };
        Err(ort::Error::new(message))
    }
}

unsafe extern "C" {
    fn OrtSessionOptionsAppendExecutionProvider_CoreML(
        options: *mut ort::sys::OrtSessionOptions,
        flags: u32,
    ) -> *mut ort::sys::OrtStatus;
}
