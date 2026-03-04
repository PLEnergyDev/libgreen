pub mod bundles;
pub mod measurements;

use measurements::MeasurementContext;
use std::os::raw::c_char;

#[unsafe(no_mangle)]
pub extern "C" fn measure_start(metrics: *const c_char) -> *mut MeasurementContext {
    measurements::measure_start(metrics)
}

#[unsafe(no_mangle)]
pub extern "C" fn measure_stop(context: *mut MeasurementContext) {
    measurements::measure_stop(context)
}

#[cfg(all(target_os = "linux", any(target_arch = "x86", target_arch = "x86_64")))]
pub mod jni {
    use crate::measurements::MeasurementContext;
    use jni::objects::{JClass, JString};
    use jni::sys::jlong;
    use jni::JNIEnv;

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_Green_measureStart(
        mut env: JNIEnv,
        _class: JClass,
        metrics: JString,
    ) -> jlong {
        let s: String = env.get_string(&metrics).unwrap().into();
        let cstr = std::ffi::CString::new(s).unwrap();
        crate::measurements::measure_start(cstr.as_ptr()) as jlong
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_Green_measureStop(_env: JNIEnv, _class: JClass, context: jlong) {
        crate::measurements::measure_stop(context as *mut MeasurementContext);
    }
}

