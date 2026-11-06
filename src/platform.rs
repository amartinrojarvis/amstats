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
            let minutes = if charging {
                number(field(8))
            } else if !plugged {
                number(field(9))
            } else {
                None
            };
            return Some(Battery {
                percent,
                plugged,
                charging,
                full,
                minutes: minutes.filter(|n| *n > 0 && *n < 100_000).map(|n| n as u32),
            });
        }
        None
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
