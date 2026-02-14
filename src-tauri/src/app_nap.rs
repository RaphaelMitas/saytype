use objc2::msg_send;
use objc2::runtime::{AnyClass, AnyObject};
use std::ffi::CStr;
use std::sync::OnceLock;

/// Wrapper around a raw NSObject pointer to make it Send + Sync.
/// Safety: The NSObject returned by beginActivityWithOptions is retained by the
/// system and we never mutate it. We only store it to prevent it from being released.
#[allow(dead_code)]
struct ActivityTokenWrapper(*mut AnyObject);

// Safety: The underlying NSObject is thread-safe and we only store the pointer
// without mutating the object.
unsafe impl Send for ActivityTokenWrapper {}
unsafe impl Sync for ActivityTokenWrapper {}

static ACTIVITY_TOKEN: OnceLock<ActivityTokenWrapper> = OnceLock::new();

pub fn disable_app_nap() -> Result<(), String> {
    unsafe {
        let process_info_class =
            AnyClass::get(c"NSProcessInfo").ok_or("NSProcessInfo class not found")?;

        let process_info: *mut AnyObject = msg_send![process_info_class, processInfo];
        if process_info.is_null() {
            return Err("Failed to get processInfo".to_string());
        }

        let ns_string_class = AnyClass::get(c"NSString").ok_or("NSString class not found")?;
        let reason_cstr = CStr::from_bytes_with_nul(b"Listening for global hotkey events\0").unwrap();
        let reason: *mut AnyObject = msg_send![ns_string_class,
            stringWithUTF8String: reason_cstr.as_ptr()];

        // NSActivityUserInitiatedAllowingIdleSystemSleep = 0x00FFFFFF
        // This prevents App Nap while still allowing system sleep
        let options: u64 = 0x00FFFFFF;

        let activity: *mut AnyObject =
            msg_send![process_info, beginActivityWithOptions: options, reason: reason];

        if activity.is_null() {
            return Err("Failed to begin activity".to_string());
        }

        ACTIVITY_TOKEN
            .set(ActivityTokenWrapper(activity))
            .map_err(|_| "Activity already set")?;
        println!("[APP_NAP] Disabled App Nap for hotkey listening");
        Ok(())
    }
}
