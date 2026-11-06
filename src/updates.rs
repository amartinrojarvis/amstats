//! Sparkle owns version comparison, scheduling, signature verification and installation.
//! All methods must run on the AppKit main thread. Bare cargo binaries have no updater.
use objc2::{
    class, msg_send,
    rc::{Allocated, Retained},
    runtime::{AnyClass, AnyObject},
};
use objc2_foundation::NSString;
use std::ptr;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const REPOSITORY: &str = "https://github.com/amartinrojarvis/amstats";

pub struct Updater {
    controller: Retained<AnyObject>,
    _framework: Retained<AnyObject>,
}

impl Updater {
    pub unsafe fn new() -> Result<Self, String> {
        unsafe {
            let main: *mut AnyObject = msg_send![class!(NSBundle), mainBundle];
            let path: Retained<NSString> = msg_send![main, bundlePath];
            let path = format!("{path}/Contents/Frameworks/Sparkle.framework");
            let framework: Option<Retained<AnyObject>> =
                msg_send![class!(NSBundle), bundleWithPath: &*NSString::from_str(&path)];
            let framework = framework.ok_or("Usa AMStats.app para recibir actualizaciones.")?;
            let loaded: bool = msg_send![&*framework, load];
            if !loaded {
                return Err("No se pudo cargar Sparkle. Reinstala AMStats.app.".into());
            }
            let class = AnyClass::get(c"SPUStandardUpdaterController")
                .ok_or("No se encontró el actualizador Sparkle.")?;
            let controller: Allocated<AnyObject> = msg_send![class, alloc];
            let controller: Retained<AnyObject> = msg_send![controller,
                initWithStartingUpdater: false,
                updaterDelegate: ptr::null::<AnyObject>(),
                userDriverDelegate: ptr::null::<AnyObject>()];
            Ok(Self {
                controller,
                _framework: framework,
            })
        }
    }

    unsafe fn updater(&self) -> *mut AnyObject {
        unsafe { msg_send![&*self.controller, updater] }
    }

    pub unsafe fn start(&self) -> Result<(), String> {
        unsafe {
            let mut error: *mut AnyObject = ptr::null_mut();
            let started: bool = msg_send![self.updater(), startUpdater: &mut error];
            if started {
                return Ok(());
            }
            if !error.is_null() {
                let description: Retained<NSString> = msg_send![error, localizedDescription];
                return Err(description.to_string());
            }
            Err("No se pudo iniciar el actualizador.".into())
        }
    }

    pub unsafe fn check(&self) {
        unsafe {
            let can_check: bool = msg_send![self.updater(), canCheckForUpdates];
            if can_check {
                let _: () = msg_send![&*self.controller, checkForUpdates: ptr::null::<AnyObject>()];
            }
        }
    }

    pub unsafe fn preferences(&self) -> (bool, bool) {
        unsafe {
            (
                msg_send![self.updater(), automaticallyChecksForUpdates],
                msg_send![self.updater(), automaticallyDownloadsUpdates],
            )
        }
    }

    pub unsafe fn set_preference(&self, automatic_install: bool, enabled: bool) {
        unsafe {
            if automatic_install {
                let _: () = msg_send![self.updater(), setAutomaticallyDownloadsUpdates: enabled];
            } else {
                let _: () = msg_send![self.updater(), setAutomaticallyChecksForUpdates: enabled];
            }
        }
    }
}
