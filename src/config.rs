use objc2::{class, msg_send, runtime::AnyObject};
use objc2_foundation::NSString;

pub const NAMES: [&str; 6] = ["CPU", "RAM", "GPU", "Disco", "Red", "Batería"];
const KEYS: [&str; 6] = [
    "showCPU",
    "showRAM",
    "showGPU",
    "showDisk",
    "showNetwork",
    "showBattery",
];
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub enabled: [bool; 6],
    pub interval: u64,
    pub battery_saver: bool,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: [true; 6],
            interval: 2,
            battery_saver: true,
        }
    }
}
impl Config {
    pub fn load() -> Self {
        let mut config = Self::default();
        unsafe {
            let defaults: *mut AnyObject = msg_send![class!(NSUserDefaults), standardUserDefaults];
            for (i, name) in KEYS.iter().enumerate() {
                let key = NSString::from_str(name);
                let object: *mut AnyObject = msg_send![defaults, objectForKey: &*key];
                if !object.is_null() {
                    config.enabled[i] = msg_send![defaults, boolForKey: &*key];
                }
            }
            let interval: isize =
                msg_send![defaults, integerForKey: &*NSString::from_str("interval")];
            if [1, 2, 5].contains(&interval) {
                config.interval = interval as u64;
            }
            let key = NSString::from_str("batterySaver");
            let object: *mut AnyObject = msg_send![defaults, objectForKey: &*key];
            if !object.is_null() {
                config.battery_saver = msg_send![defaults, boolForKey: &*key];
            }
        }
        config
    }
    pub fn save(&self) {
        unsafe {
            let defaults: *mut AnyObject = msg_send![class!(NSUserDefaults), standardUserDefaults];
            for (i, name) in KEYS.iter().enumerate() {
                let _: () = msg_send![defaults, setBool: self.enabled[i], forKey: &*NSString::from_str(name)];
            }
            let _: () = msg_send![defaults, setInteger: self.interval as isize, forKey: &*NSString::from_str("interval")];
            let _: () = msg_send![defaults, setBool: self.battery_saver, forKey: &*NSString::from_str("batterySaver")];
        }
    }
    pub fn any_enabled(&self) -> bool {
        self.enabled.iter().any(|v| *v)
    }
    pub fn effective_interval(&self, on_battery: bool) -> u64 {
        self.interval
            * if self.battery_saver && on_battery {
                2
            } else {
                1
            }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn battery_policy() {
        let mut c = Config::default();
        assert_eq!(c.effective_interval(false), 2);
        assert_eq!(c.effective_interval(true), 4);
        c.battery_saver = false;
        assert_eq!(c.effective_interval(true), 2);
        c.enabled = [false; 6];
        assert!(!c.any_enabled());
    }
}
