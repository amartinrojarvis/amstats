//! Narrow FFI boundary. Every owned Mach/IOKit/CF handle is released by RAII.
#![allow(deprecated)] // libc Mach declarations are stable; avoid an extra binding crate.
use objc2_foundation::NSString;
use std::{
    ffi::{CStr, c_void},
    mem, ptr,
};

type Cf = *const c_void;
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(value: Cf);
    fn CFDataCreate(alloc: Cf, bytes: *const u8, length: isize) -> Cf;
    fn CFPropertyListCreateWithData(
        alloc: Cf,
        data: Cf,
        options: usize,
        format: *mut isize,
        error: *mut Cf,
    ) -> Cf;
    fn CFGetTypeID(value: Cf) -> usize;
    fn CFDictionaryGetTypeID() -> usize;
    fn CFNumberGetTypeID() -> usize;
    fn CFBooleanGetTypeID() -> usize;
    fn CFDictionaryGetValue(dict: Cf, key: Cf) -> Cf;
    fn CFNumberGetValue(number: Cf, kind: isize, value: *mut c_void) -> bool;
    fn CFBooleanGetValue(value: Cf) -> bool;
    fn CFEqual(a: Cf, b: Cf) -> bool;
    fn CFArrayGetCount(array: Cf) -> isize;
    fn CFArrayGetValueAtIndex(array: Cf, index: isize) -> Cf;
    fn CFArrayGetTypeID() -> usize;
    fn CFStringGetTypeID() -> usize;
    fn CFStringGetCString(value: Cf, buffer: *mut i8, size: isize, encoding: u32) -> bool;
    fn CFDictionaryGetCount(dict: Cf) -> isize;
    fn CFDictionaryGetKeysAndValues(dict: Cf, keys: *mut Cf, values: *mut Cf);
}
#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOServiceMatching(name: *const i8) -> Cf;
    fn IOServiceGetMatchingService(port: u32, matching: Cf) -> u32;
    fn IOObjectRelease(object: u32) -> i32;
    fn IORegistryEntryCreateCFProperty(entry: u32, key: Cf, alloc: Cf, options: u32) -> Cf;
    fn IOPSCopyPowerSourcesInfo() -> Cf;
    fn IOPSCopyPowerSourcesList(info: Cf) -> Cf;
    fn IOPSGetPowerSourceDescription(info: Cf, source: Cf) -> Cf;
    fn IORegistryEntryCreateIterator(
        entry: u32,
        plane: *const i8,
        options: u32,
        iterator: *mut u32,
    ) -> i32;
    fn IOIteratorNext(iterator: u32) -> u32;
    fn IORegistryEntryGetRegistryEntryID(entry: u32, id: *mut u64) -> i32;
    fn IORegistryEntryCreateCFProperties(
        entry: u32,
        properties: *mut Cf,
        alloc: Cf,
        options: u32,
    ) -> i32;
    fn IOPMCopyAssertionsByProcess(assertions: *mut Cf) -> i32;
}

struct OwnedCf(Cf);
impl OwnedCf {
    fn new(value: Cf) -> Option<Self> {
        (!value.is_null()).then_some(Self(value))
    }
}
impl Drop for OwnedCf {
    fn drop(&mut self) {
        unsafe {
            CFRelease(self.0);
        }
    }
}
fn key(value: &NSString) -> Cf {
    (value as *const NSString).cast()
}
fn get(dict: Cf, name: &NSString) -> Cf {
    if dict.is_null() || unsafe { CFGetTypeID(dict) != CFDictionaryGetTypeID() } {
        return ptr::null();
    }
    unsafe { CFDictionaryGetValue(dict, key(name)) }
}
fn number(value: Cf) -> Option<i64> {
    if value.is_null() || unsafe { CFGetTypeID(value) != CFNumberGetTypeID() } {
        return None;
    }
    let mut n = 0i64;
    unsafe { CFNumberGetValue(value, 4, (&mut n as *mut i64).cast()) }.then_some(n)
}
fn boolean(value: Cf) -> bool {
    !value.is_null()
        && unsafe { CFGetTypeID(value) == CFBooleanGetTypeID() && CFBooleanGetValue(value) }
}
fn equals(value: Cf, other: &NSString) -> bool {
    !value.is_null() && unsafe { CFEqual(value, key(other)) }
}

pub struct Host {
    port: u32,
    pub total_memory: u64,
    page_size: u64,
}
impl Host {
    pub fn new() -> Self {
        Self {
            port: unsafe { libc::mach_host_self() },
            total_memory: sysctl_u64(c"hw.memsize").unwrap_or(0),
            page_size: unsafe { libc::sysconf(libc::_SC_PAGESIZE) }.max(1) as u64,
        }
    }
    pub fn cpu(&self) -> Option<[u32; 4]> {
        let mut ticks = [0u32; 4];
        let mut count = 4;
        let result = unsafe {
            libc::host_statistics(
                self.port,
                libc::HOST_CPU_LOAD_INFO,
                ticks.as_mut_ptr().cast(),
                &mut count,
            )
        };
        (result == 0 && count == 4).then_some(ticks)
    }
    pub fn memory(&self) -> Option<Memory> {
        let mut vm: libc::vm_statistics64 = unsafe { mem::zeroed() };
        // REV1 fields through total_uncompressed_pages_in_compressor; supported on macOS 13+.
        // Do not request the newer struct size when running against an older kernel.
        let mut count = (mem::offset_of!(libc::vm_statistics64, swapped_count) / 4) as u32;
        let result = unsafe {
            libc::host_statistics64(
                self.port,
                libc::HOST_VM_INFO64,
                (&mut vm as *mut libc::vm_statistics64).cast(),
                &mut count,
            )
        };
        if result != 0 || self.total_memory == 0 {
            return None;
        }
        let pages = u64::from(vm.active_count)
            + u64::from(vm.inactive_count)
            + u64::from(vm.speculative_count)
            + u64::from(vm.wire_count)
            + u64::from(vm.compressor_page_count);
        let reclaimable = u64::from(vm.purgeable_count) + u64::from(vm.external_page_count);
        Some(Memory {
            used: (pages.saturating_sub(reclaimable) * self.page_size).min(self.total_memory),
            total: self.total_memory,
            compressed: u64::from(vm.compressor_page_count) * self.page_size,
        })
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        unsafe {
            mach_port_deallocate(libc::mach_task_self(), self.port);
        }
    }
}
fn sysctl_u64(name: &CStr) -> Option<u64> {
    let mut value = 0u64;
    let mut size = mem::size_of::<u64>();
    (unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            (&mut value as *mut u64).cast(),
            &mut size,
            ptr::null_mut(),
            0,
        )
    } == 0)
        .then_some(value)
}
#[derive(Clone, Copy, Debug)]
pub struct Memory {
    pub used: u64,
    pub total: u64,
    pub compressed: u64,
}
#[derive(Clone, Copy, Debug)]
pub struct Disk {
    pub used: u64,
    pub total: u64,
    pub available: u64,
}
pub fn disk() -> Option<Disk> {
    let mut stat: libc::statfs = unsafe { mem::zeroed() };
    // Data volume shares APFS container space with the system volume.
    if unsafe { libc::statfs(c"/System/Volumes/Data".as_ptr(), &mut stat) } != 0 {
        return None;
    }
    let block = u64::from(stat.f_bsize);
    let total = stat.f_blocks.checked_mul(block)?;
    if total == 0 {
        return None;
    }
    let available = stat.f_bavail.saturating_mul(block).min(total);
    Some(Disk {
        total,
        available,
        used: total - available,
    })
}

pub struct Gpu {
    service: u32,
    property: objc2::rc::Retained<NSString>,
    utilization: [objc2::rc::Retained<NSString>; 2],
}
impl Gpu {
    pub fn new() -> Self {
        Self {
            service: 0,
            property: NSString::from_str("PerformanceStatistics"),
            utilization: [
                NSString::from_str("Device Utilization %"),
                NSString::from_str("GPU Activity(%)"),
            ],
        }
    }
    pub fn read(&mut self) -> Option<f64> {
        if self.service == 0 {
            self.service = unsafe {
                IOServiceGetMatchingService(0, IOServiceMatching(c"IOAccelerator".as_ptr()))
            };
        }
        if self.service == 0 {
            return None;
        }
        let property = OwnedCf::new(unsafe {
            IORegistryEntryCreateCFProperty(self.service, key(&self.property), ptr::null(), 0)
        });
        let Some(property) = property else {
            self.reset();
            return None;
        };
        self.utilization
            .iter()
            .find_map(|name| number(get(property.0, name)))
            .map(|n| (n as f64).clamp(0., 100.))
    }
    pub fn reset(&mut self) {
        if self.service != 0 {
            unsafe {
                IOObjectRelease(self.service);
            }
            self.service = 0;
        }
    }
}
impl Drop for Gpu {
    fn drop(&mut self) {
        self.reset();
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Battery {
    pub percent: f64,
    pub plugged: bool,
    pub charging: bool,
    pub full: bool,
    pub minutes: Option<u32>,
}
pub struct Power {
    keys: Vec<objc2::rc::Retained<NSString>>,
}
impl Power {
    pub fn new() -> Self {
        Self {
            keys: [
                "Type",
                "InternalBattery",
                "Current Capacity",
                "Max Capacity",
                "Power Source State",
                "AC Power",
                "Is Charging",
                "Is Charged",
                "Time to Full Charge",
                "Time to Empty",
                "Is Present",
            ]
            .into_iter()
            .map(NSString::from_str)
            .collect(),
        }
    }
    pub fn read(&self) -> Option<Battery> {
        let info = OwnedCf::new(unsafe { IOPSCopyPowerSourcesInfo() })?;
        let list = OwnedCf::new(unsafe { IOPSCopyPowerSourcesList(info.0) })?;
        for index in 0..unsafe { CFArrayGetCount(list.0) } {
            let description = unsafe {
                IOPSGetPowerSourceDescription(info.0, CFArrayGetValueAtIndex(list.0, index))
            };
            let field = |n: usize| get(description, &self.keys[n]);
            if !equals(field(0), &self.keys[1]) || !boolean(field(10)) {
                continue;
            }
            let capacity = number(field(3))?;
            if capacity <= 0 {
                return None;
            }
            let percent = (number(field(2))? as f64 * 100. / capacity as f64).clamp(0., 100.);
            let plugged = equals(field(4), &self.keys[5]);
            let charging = boolean(field(6));
            let full = boolean(field(7));
            // Use one powerd snapshot for state and time, like pmset -g batt.
            // The aggregate estimate can also include an attached UPS.
            let minutes =
                battery_minutes(plugged, charging, full, number(field(9)), number(field(8)));
            return Some(Battery {
                percent,
                plugged,
                charging,
                full,
                minutes,
            });
        }
        None
    }
}

// Both per-source time fields are already in minutes. Do not extrapolate or
// smooth powerd's estimate a second time. 0xffff means unknown, not 45 days.
fn battery_minutes(
    plugged: bool,
    charging: bool,
    full: bool,
    to_empty: Option<i64>,
    to_full: Option<i64>,
) -> Option<u32> {
    if full && plugged {
        return None;
    }
    let minutes = if charging {
        to_full
    } else if !plugged {
        to_empty
    } else {
        None
    };
    minutes.filter(|m| (1..65535).contains(m)).map(|m| m as u32)
}

#[cfg(test)]
mod power_tests {
    use super::*;
    #[test]
    fn remaining_time_uses_the_matching_system_field_without_conversion() {
        assert_eq!(
            battery_minutes(false, false, false, Some(91), Some(60)),
            Some(91)
        );
        assert_eq!(
            battery_minutes(true, true, false, Some(91), Some(60)),
            Some(60)
        );
        for value in [None, Some(-1), Some(0), Some(65535), Some(65536)] {
            assert_eq!(battery_minutes(false, false, false, value, Some(120)), None);
            assert_eq!(battery_minutes(true, true, false, Some(120), value), None);
        }
        assert_eq!(
            battery_minutes(true, false, false, Some(91), Some(60)),
            None
        );
        assert_eq!(battery_minutes(true, true, true, Some(91), Some(60)), None);
    }
    #[test]
    fn system_health_parses_percentage_without_recomputing_raw_capacity() {
        assert_eq!(parse_health_percent("97\u{a0}%"), Some(97.));
        assert_eq!(parse_health_percent("100%"), Some(100.));
        for value in ["", "—", "NaN%", "101%", "-1%", "97", "0%"] {
            assert_eq!(parse_health_percent(value), None);
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NetCounter {
    pub index: u16,
    pub received: u64,
    pub sent: u64,
}
#[derive(Default)]
pub struct Network {
    buffer: Vec<u8>,
}
impl Network {
    pub fn read(&mut self, out: &mut Vec<NetCounter>) -> bool {
        let mut mib = [libc::CTL_NET, libc::PF_ROUTE, 0, 0, libc::NET_RT_IFLIST2, 0];
        out.clear();
        // Reuse the routing buffer. Re-query its size only on first use or interface changes.
        for _ in 0..2 {
            if self.buffer.is_empty() {
                let mut size = 0;
                if unsafe {
                    libc::sysctl(
                        mib.as_mut_ptr(),
                        6,
                        ptr::null_mut(),
                        &mut size,
                        ptr::null_mut(),
                        0,
                    )
                } != 0
                {
                    return false;
                }
                self.buffer.resize(size + 4096, 0);
            }
            let mut len = self.buffer.len();
            if unsafe {
                libc::sysctl(
                    mib.as_mut_ptr(),
                    6,
                    self.buffer.as_mut_ptr().cast(),
                    &mut len,
                    ptr::null_mut(),
                    0,
                )
            } != 0
            {
                if std::io::Error::last_os_error().raw_os_error() == Some(libc::ENOMEM) {
                    self.buffer.clear();
                    continue;
                }
                return false;
            }
            let mut offset = 0;
            while offset + 4 <= len {
                let size =
                    u16::from_ne_bytes([self.buffer[offset], self.buffer[offset + 1]]) as usize;
                if size < 4 || offset + size > len {
                    return false;
                }
                if self.buffer[offset + 3] == libc::RTM_IFINFO2 as u8
                    && size >= mem::size_of::<libc::if_msghdr2>()
                {
                    // Routing records are not guaranteed to have Rust's struct alignment.
                    let header = unsafe {
                        ptr::read_unaligned(
                            self.buffer.as_ptr().add(offset).cast::<libc::if_msghdr2>(),
                        )
                    };
                    let mut name = [0i8; libc::IF_NAMESIZE];
                    if header.ifm_flags & (libc::IFF_UP | libc::IFF_RUNNING)
                        == (libc::IFF_UP | libc::IFF_RUNNING)
                        && !unsafe {
                            libc::if_indextoname(header.ifm_index as u32, name.as_mut_ptr())
                        }
                        .is_null()
                    {
                        let name = unsafe { CStr::from_ptr(name.as_ptr()) }.to_bytes();
                        // Count the physical transport once: exclude utun, awdl, lo0 and bridges.
                        if name.starts_with(b"en") {
                            out.push(NetCounter {
                                index: header.ifm_index,
                                received: header.ifm_data.ifi_ibytes,
                                sent: header.ifm_data.ifi_obytes,
                            });
                        }
                    }
                }
                offset += size;
            }
            return true;
        }
        false
    }
}

pub fn process_cpu_seconds() -> f64 {
    let mut usage: libc::rusage = unsafe { mem::zeroed() };
    unsafe {
        libc::getrusage(libc::RUSAGE_SELF, &mut usage);
    }
    usage.ru_utime.tv_sec as f64
        + usage.ru_stime.tv_sec as f64
        + (usage.ru_utime.tv_usec + usage.ru_stime.tv_usec) as f64 / 1e6
}
pub fn resident_bytes() -> u64 {
    let mut info: libc::mach_task_basic_info = unsafe { mem::zeroed() };
    let mut count = libc::MACH_TASK_BASIC_INFO_COUNT;
    let result = unsafe {
        libc::task_info(
            libc::mach_task_self(),
            libc::MACH_TASK_BASIC_INFO,
            (&mut info as *mut libc::mach_task_basic_info).cast(),
            &mut count,
        )
    };
    if result == 0 { info.resident_size } else { 0 }
}

unsafe extern "C" {
    fn mach_port_deallocate(task: u32, name: u32) -> i32;
}
#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOPSNotificationCreateRunLoopSource(
        callback: extern "C" fn(*mut c_void),
        context: *mut c_void,
    ) -> Cf;
}
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRunLoopGetMain() -> Cf;
    fn CFRunLoopAddSource(run_loop: Cf, source: Cf, mode: Cf);
    fn CFRunLoopRemoveSource(run_loop: Cf, source: Cf, mode: Cf);
    static kCFRunLoopCommonModes: Cf;
}
/// Owned by the main-thread App. Delivers plug/unplug and battery changes without polling.
pub struct PowerNotification(OwnedCf);
impl PowerNotification {
    pub fn new(callback: extern "C" fn(*mut c_void)) -> Option<Self> {
        let source = OwnedCf::new(unsafe {
            IOPSNotificationCreateRunLoopSource(callback, ptr::null_mut())
        })?;
        unsafe {
            CFRunLoopAddSource(CFRunLoopGetMain(), source.0, kCFRunLoopCommonModes);
        }
        Some(Self(source))
    }
}
impl Drop for PowerNotification {
    fn drop(&mut self) {
        unsafe {
            CFRunLoopRemoveSource(CFRunLoopGetMain(), self.0.0, kCFRunLoopCommonModes);
        }
    }
}

// Detail-only collectors: never called by the menu-bar sampler.
struct IoHandle(u32);
impl Drop for IoHandle {
    fn drop(&mut self) {
        if self.0 != 0 {
            unsafe {
                IOObjectRelease(self.0);
            }
        }
    }
}
fn registry_properties(service: u32) -> Option<OwnedCf> {
    let mut properties = ptr::null();
    (unsafe { IORegistryEntryCreateCFProperties(service, &mut properties, ptr::null(), 0) } == 0)
        .then(|| OwnedCf::new(properties))
        .flatten()
}
fn cf_text(value: Cf) -> Option<String> {
    if value.is_null() || unsafe { CFGetTypeID(value) != CFStringGetTypeID() } {
        return None;
    }
    let mut buffer = [0i8; 1024];
    unsafe {
        CFStringGetCString(
            value,
            buffer.as_mut_ptr(),
            buffer.len() as isize,
            0x08000100,
        )
    }
    .then(|| {
        unsafe { CStr::from_ptr(buffer.as_ptr()) }
            .to_string_lossy()
            .into_owned()
    })
}
fn field_number(dict: Cf, name: &str) -> Option<i64> {
    number(get(dict, &NSString::from_str(name)))
}

/// Per-client GPU counters. Registry IDs prevent spikes when a Metal client exits.
/// These driver properties are optional and not a stable public Apple API.
pub fn gpu_clients() -> Vec<(u64, i32, u64, String)> {
    let accelerator = IoHandle(unsafe {
        IOServiceGetMatchingService(0, IOServiceMatching(c"IOAccelerator".as_ptr()))
    });
    let mut iterator = 0;
    if accelerator.0 == 0
        || unsafe {
            IORegistryEntryCreateIterator(accelerator.0, c"IOService".as_ptr(), 1, &mut iterator)
        } != 0
    {
        return Vec::new();
    }
    let iterator = IoHandle(iterator);
    let creator = NSString::from_str("IOUserClientCreator");
    let usage = NSString::from_str("AppUsage");
    let time = NSString::from_str("accumulatedGPUTime");
    let mut out = Vec::new();
    loop {
        let entry = IoHandle(unsafe { IOIteratorNext(iterator.0) });
        if entry.0 == 0 {
            break;
        }
        let Some(properties) = registry_properties(entry.0) else {
            continue;
        };
        let Some(owner) = cf_text(get(properties.0, &creator)) else {
            continue;
        };
        let Some(pid) = owner
            .strip_prefix("pid ")
            .and_then(|s| s.split(',').next())
            .and_then(|s| s.parse::<i32>().ok())
        else {
            continue;
        };
        let array = get(properties.0, &usage);
        if array.is_null() || unsafe { CFGetTypeID(array) != CFArrayGetTypeID() } {
            continue;
        }
        let mut total = 0u64;
        for index in 0..unsafe { CFArrayGetCount(array) } {
            let item = unsafe { CFArrayGetValueAtIndex(array, index) };
            total = total.saturating_add(number(get(item, &time)).unwrap_or(0).max(0) as u64);
        }
        let mut id = 0;
        if unsafe { IORegistryEntryGetRegistryEntryID(entry.0, &mut id) } == 0 {
            out.push((
                id,
                pid,
                total,
                owner
                    .split_once(", ")
                    .map_or("", |(_, name)| name)
                    .to_string(),
            ));
        }
    }
    out
}

#[derive(Clone, Debug, Default)]
pub struct BatteryDetails {
    pub adapter_watts: Option<f64>,
    pub battery_watts: Option<f64>,
    pub cycles: Option<u64>,
    pub capacity_mah: Option<u64>,
    pub connected: bool,
}
pub fn battery_details() -> BatteryDetails {
    let service = IoHandle(unsafe {
        IOServiceGetMatchingService(0, IOServiceMatching(c"AppleSmartBattery".as_ptr()))
    });
    let Some(properties) = registry_properties(service.0) else {
        return BatteryDetails::default();
    };
    let dict = properties.0;
    let connected = boolean(get(dict, &NSString::from_str("ExternalConnected")));
    let adapter = get(dict, &NSString::from_str("AdapterDetails"));
    let battery = get(dict, &NSString::from_str("BatteryData"));
    let capacity = field_number(battery, "AppleRawMaxCapacity").filter(|v| *v > 0);
    let volts = field_number(dict, "Voltage").filter(|v| (1000..30000).contains(v));
    let amps = field_number(dict, "InstantAmperage").or_else(|| field_number(dict, "Amperage"));
    BatteryDetails {
        connected,
        adapter_watts: connected
            .then(|| {
                field_number(adapter, "Watts")
                    .filter(|v| (1..1000).contains(v))
                    .map(|v| v as f64)
            })
            .flatten(),
        battery_watts: volts
            .zip(amps)
            .map(|(v, a)| v as f64 * a as f64 / 1e6)
            .filter(|w| w.abs() < 500.),
        cycles: field_number(dict, "CycleCount")
            .filter(|v| *v >= 0)
            .map(|v| v as u64),
        capacity_mah: capacity.map(|v| v as u64),
    }
}

// powerd's calibrated health is not the instantaneous raw FCC/design ratio.
// Read the system report only on the detail worker, and cache it there.
pub fn system_battery_health() -> Option<f64> {
    let output = std::process::Command::new("/usr/sbin/system_profiler")
        .args([
            "SPPowerDataType",
            "-xml",
            "-detailLevel",
            "mini",
            "-timeout",
            "5",
        ])
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let data = OwnedCf::new(unsafe {
        CFDataCreate(
            ptr::null(),
            output.stdout.as_ptr(),
            output.stdout.len().try_into().ok()?,
        )
    })?;
    let report = OwnedCf::new(unsafe {
        CFPropertyListCreateWithData(ptr::null(), data.0, 0, ptr::null_mut(), ptr::null_mut())
    })?;
    system_health_from_report(report.0)
}
fn system_health_from_report(report: Cf) -> Option<f64> {
    if report.is_null() || unsafe { CFGetTypeID(report) != CFArrayGetTypeID() } {
        return None;
    }
    for i in 0..unsafe { CFArrayGetCount(report) } {
        let section = unsafe { CFArrayGetValueAtIndex(report, i) };
        let items = get(section, &NSString::from_str("_items"));
        if items.is_null() || unsafe { CFGetTypeID(items) != CFArrayGetTypeID() } {
            continue;
        }
        for j in 0..unsafe { CFArrayGetCount(items) } {
            let item = unsafe { CFArrayGetValueAtIndex(items, j) };
            let health = get(item, &NSString::from_str("sppower_battery_health_info"));
            let value = cf_text(get(
                health,
                &NSString::from_str("sppower_battery_health_maximum_capacity"),
            ));
            if let Some(percent) = value.as_deref().and_then(parse_health_percent) {
                return Some(percent);
            }
        }
    }
    None
}
fn parse_health_percent(text: &str) -> Option<f64> {
    let value: f64 = text.trim().strip_suffix('%')?.trim().parse().ok()?;
    (value.is_finite() && value > 0. && value <= 100.).then_some(value)
}

pub fn sleep_blockers() -> Vec<i32> {
    let mut dict = ptr::null();
    if unsafe { IOPMCopyAssertionsByProcess(&mut dict) } != 0 {
        return Vec::new();
    }
    let Some(dict) = OwnedCf::new(dict) else {
        return Vec::new();
    };
    let count = unsafe { CFDictionaryGetCount(dict.0) }.max(0) as usize;
    let mut keys = vec![ptr::null(); count];
    let mut values = vec![ptr::null(); count];
    unsafe {
        CFDictionaryGetKeysAndValues(dict.0, keys.as_mut_ptr(), values.as_mut_ptr());
    }
    let mut out = Vec::new();
    for (pid, assertions) in keys.into_iter().zip(values) {
        if assertions.is_null() || unsafe { CFGetTypeID(assertions) != CFArrayGetTypeID() } {
            continue;
        }
        for i in 0..unsafe { CFArrayGetCount(assertions) } {
            let assertion = unsafe { CFArrayGetValueAtIndex(assertions, i) };
            let kind =
                cf_text(get(assertion, &NSString::from_str("AssertType"))).unwrap_or_default();
            if field_number(assertion, "AssertLevel").unwrap_or(0) != 0
                && matches!(
                    kind.as_str(),
                    "PreventUserIdleSystemSleep"
                        | "PreventSystemSleep"
                        | "PreventUserIdleDisplaySleep"
                        | "NoIdleSleepAssertion"
                        | "NoDisplaySleepAssertion"
                )
            {
                if let Some(pid) = number(pid) {
                    out.push(pid as i32);
                }
                break;
            }
        }
    }
    out
}

pub fn memory_details() -> (Option<u64>, Option<u32>) {
    #[repr(C)]
    #[derive(Default)]
    struct Swap {
        total: u64,
        available: u64,
        used: u64,
        pagesize: i32,
        encrypted: i32,
    }
    let mut swap = Swap::default();
    let mut size = mem::size_of::<Swap>();
    let valid = unsafe {
        libc::sysctlbyname(
            c"vm.swapusage".as_ptr(),
            (&mut swap as *mut Swap).cast(),
            &mut size,
            ptr::null_mut(),
            0,
        )
    } == 0;
    let mut pressure = 0u32;
    let mut size = mem::size_of::<u32>();
    let has_pressure = unsafe {
        libc::sysctlbyname(
            c"kern.memorystatus_vm_pressure_level".as_ptr(),
            (&mut pressure as *mut u32).cast(),
            &mut size,
            ptr::null_mut(),
            0,
        )
    } == 0;
    (valid.then_some(swap.used), has_pressure.then_some(pressure))
}

pub fn active_interfaces() -> String {
    let mut addresses = ptr::null_mut();
    if unsafe { libc::getifaddrs(&mut addresses) } != 0 {
        return "Interfaz no disponible".into();
    }
    let mut names = Vec::new();
    let mut item = addresses;
    while !item.is_null() {
        let address = unsafe { &*item };
        let name = unsafe { CStr::from_ptr(address.ifa_name) }.to_string_lossy();
        if name.starts_with("en")
            && address.ifa_flags & libc::IFF_RUNNING as u32 != 0
            && !names.iter().any(|n| n == &name)
        {
            names.push(name.into_owned());
        }
        item = address.ifa_next;
    }
    unsafe {
        libc::freeifaddrs(addresses);
    }
    if names.is_empty() {
        "Sin interfaz física activa".into()
    } else {
        names.join(" · ")
    }
}
