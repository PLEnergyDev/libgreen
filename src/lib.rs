pub mod measurements;
pub mod bundles;

use std::os::raw::c_char;
use measurements::MeasurementState;

#[unsafe(no_mangle)]
pub extern "C" fn measure_start(events: *const c_char) -> *mut MeasurementState {
    measurements::measure_start(events)
}

#[unsafe(no_mangle)]
pub extern "C" fn measure_stop(state: *mut MeasurementState) {
    measurements::measure_stop(state)
}

#[cfg(all(target_os = "linux", any(target_arch = "x86", target_arch = "x86_64")))]
pub mod jni {
    use jni::objects::{JClass, JString};
    use jni::sys::jlong;
    use jni::JNIEnv;
    use crate::measurements::MeasurementState;

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_Green_measureStart(
        mut env: JNIEnv,
        _class: JClass,
        events: JString,
    ) -> jlong {
        let s: String = env.get_string(&events).unwrap().into();
        let cstr = std::ffi::CString::new(s).unwrap();
        crate::measurements::measure_start(cstr.as_ptr()) as jlong
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_Green_measureStop(
        _env: JNIEnv,
        _class: JClass,
        handle: jlong,
    ) {
        crate::measurements::measure_stop(handle as *mut MeasurementState);
    }
}

