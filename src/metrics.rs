use crate::{
    config::Config,
    platform::{self, Battery, Disk, Gpu, Host, Memory, NetCounter, Network, Power},
};
use objc2::{msg_send, rc::autoreleasepool, runtime::AnyObject, sel};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, Default)]
pub struct Rate {
    pub down: f64,
    pub up: f64,
}
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub cpu: Option<f64>,
    pub memory: Option<Memory>,
    pub gpu: Option<f64>,
    pub disk: Option<Disk>,
    pub network: Option<Rate>,
    pub battery: Option<Battery>,
    pub sampled_at: Option<Instant>,
    pub network_total: Rate,
    pub sample_us: u64,
    pub interval: u64,
}
pub enum Command {
    Configure(Config),
    Suspend(u8, bool),
    PowerChanged,
    Stop,
}
pub struct Shared {
    pub snapshot: Mutex<Snapshot>,
    pub pending: AtomicBool,
}
pub struct Worker {
    pub tx: Sender<Command>,
    join: Option<thread::JoinHandle<()>>,
}
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.tx.send(Command::Stop);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}
impl Worker {
    /// The target is owned by App until this worker has been joined. Only the
    /// thread-safe NSObject main-thread scheduling method is sent off-main.
    pub fn start(config: Config, shared: Arc<Shared>, target: usize) -> Self {
        let (tx, rx) = mpsc::channel();
        let join = thread::Builder::new().name("amstats.sampler".into()).spawn(move || {
            // Utility QoS: yield to foreground work without holding an activity assertion.
            unsafe { pthread_set_qos_class_self_np(0x11, 0); }
            let mut sampler = autoreleasepool(|_| Sampler::new());
            worker_loop(&mut sampler, config, &rx, |snapshot| {
                *shared.snapshot.lock().unwrap() = snapshot;
                if !shared.pending.swap(true, Ordering::AcqRel) {
                    autoreleasepool(|_| unsafe {
                        let _: () = msg_send![target as *mut AnyObject, performSelectorOnMainThread: sel!(refresh:), withObject: std::ptr::null::<AnyObject>(), waitUntilDone: false];
                    });
                }
            });
        }).expect("No se pudo iniciar el muestreador");
        Self {
            tx,
            join: Some(join),
        }
    }
}
unsafe extern "C" {
    fn pthread_set_qos_class_self_np(class: u32, relative_priority: i32) -> i32;
}
fn worker_loop(
    sampler: &mut Sampler,
    mut config: Config,
    rx: &Receiver<Command>,
    mut publish: impl FnMut(Snapshot),
) {
    let mut suspended = 0u8;
    let mut next = Instant::now();
    loop {
        let waiting = suspended != 0 || !config.any_enabled();
        let command = if waiting {
            rx.recv().map_err(|_| mpsc::RecvTimeoutError::Disconnected)
        } else {
            rx.recv_timeout(next.saturating_duration_since(Instant::now()))
        };
        match command {
            Ok(Command::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Ok(Command::Suspend(reason, value)) => {
                if value {
                    suspended |= reason;
                } else {
                    suspended &= !reason;
                }
                sampler.reset();
                next = Instant::now();
            }
            Ok(Command::Configure(new)) => {
                config = new;
                sampler.reset();
                next = Instant::now();
                if !config.any_enabled() {
                    publish(Snapshot::default());
                }
            }
            Ok(Command::PowerChanged) => {
                sampler.power_at = None;
                next = Instant::now();
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let snapshot = autoreleasepool(|_| sampler.sample(&config));
                let interval = snapshot.interval;
                publish(snapshot);
                next = Instant::now() + Duration::from_secs(interval);
            }
        }
    }
}

pub struct Sampler {
    host: Host,
    gpu: Gpu,
    power: Power,
    net: Network,
    ticks: Option<[u32; 4]>,
    net_previous: Vec<NetCounter>,
    net_current: Vec<NetCounter>,
    net_at: Option<Instant>,
    disk_at: Option<Instant>,
    power_at: Option<Instant>,
    gpu_retry_at: Option<Instant>,
    snapshot: Snapshot,
    network_total: Rate,
}
impl Sampler {
    pub fn new() -> Self {
        Self {
            host: Host::new(),
            gpu: Gpu::new(),
            power: Power::new(),
            net: Network::default(),
            ticks: None,
            net_previous: Vec::with_capacity(8),
            net_current: Vec::with_capacity(8),
            net_at: None,
            disk_at: None,
            power_at: None,
            gpu_retry_at: None,
            snapshot: Snapshot::default(),
            network_total: Rate::default(),
        }
    }
    pub fn reset(&mut self) {
        self.ticks = None;
        self.net_at = None;
        self.net_previous.clear();
        self.disk_at = None;
        self.power_at = None;
        self.gpu_retry_at = None;
        self.gpu.reset();
        self.snapshot = Snapshot::default();
    }
    pub fn sample(&mut self, config: &Config) -> Snapshot {
        let start = Instant::now();
        if config.enabled[0] {
            let current = self.host.cpu();
            self.snapshot.cpu = self
                .ticks
                .zip(current)
                .and_then(|(old, new)| cpu_percent(old, new));
            self.ticks = current;
        } else {
            self.snapshot.cpu = None;
            self.ticks = None;
        }
        self.snapshot.memory = if config.enabled[1] {
            self.host.memory()
        } else {
            None
        };
        if config.enabled[2] && due(self.gpu_retry_at, start, 30) {
            self.snapshot.gpu = self.gpu.read();
            self.gpu_retry_at = if self.snapshot.gpu.is_none() {
                Some(start)
            } else {
                None
            };
        } else if !config.enabled[2] {
            self.snapshot.gpu = None;
        }
        if config.enabled[3] && due(self.disk_at, start, 30) {
            self.snapshot.disk = platform::disk();
            self.disk_at = Some(start);
        } else if !config.enabled[3] {
            self.snapshot.disk = None;
        }
        if config.enabled[4] {
            if self.net.read(&mut self.net_current) {
                self.snapshot.network = self.net_at.and_then(|then| {
                    network_rate(
                        &self.net_previous,
                        &self.net_current,
                        start.duration_since(then).as_secs_f64(),
                    )
                });
                if let (Some(rate), Some(then)) = (self.snapshot.network, self.net_at) {
                    let seconds = start.duration_since(then).as_secs_f64();
                    self.network_total.down += rate.down * seconds;
                    self.network_total.up += rate.up * seconds;
                }
                std::mem::swap(&mut self.net_previous, &mut self.net_current);
                self.net_at = Some(start);
            } else {
                self.snapshot.network = None;
                self.net_at = None;
                self.net_previous.clear();
            }
        } else {
            self.snapshot.network = None;
            self.net_at = None;
        }
        // A hidden battery is read only if needed by the optional energy-saving policy.
        if (config.enabled[5] || config.battery_saver) && due(self.power_at, start, 30) {
            self.snapshot.battery = self.power.read();
            self.power_at = Some(start);
        } else if !config.enabled[5] && !config.battery_saver {
            self.snapshot.battery = None;
        }
        self.snapshot.interval =
            config.effective_interval(self.snapshot.battery.is_some_and(|b| !b.plugged));
        self.snapshot.sampled_at = Some(start);
        self.snapshot.network_total = self.network_total;
        self.snapshot.sample_us = start.elapsed().as_micros() as u64;
        self.snapshot.clone()
    }
}
fn due(last: Option<Instant>, now: Instant, seconds: u64) -> bool {
    last.is_none_or(|then| now.duration_since(then) >= Duration::from_secs(seconds))
}
pub fn cpu_percent(previous: [u32; 4], current: [u32; 4]) -> Option<f64> {
    let delta: [u64; 4] = std::array::from_fn(|i| u64::from(current[i].wrapping_sub(previous[i])));
    let total: u64 = delta.iter().sum();
    (total > 0).then(|| 100. * (total - delta[2]) as f64 / total as f64)
}
pub fn network_rate(previous: &[NetCounter], current: &[NetCounter], elapsed: f64) -> Option<Rate> {
    if !elapsed.is_finite() || elapsed <= 0. {
        return None;
    }
    let mut rate = Rate::default();
    for new in current {
        if let Some(old) = previous.iter().find(|old| old.index == new.index) {
            // New interfaces, resets, and reconnects cannot create artificial traffic spikes.
            rate.down += new.received.saturating_sub(old.received) as f64 / elapsed;
            rate.up += new.sent.saturating_sub(old.sent) as f64 / elapsed;
        }
    }
    Some(rate)
}
pub fn percent(used: u64, total: u64) -> f64 {
    if total == 0 {
        0.
    } else {
        (100. * used as f64 / total as f64).clamp(0., 100.)
    }
}
pub fn bytes(value: u64) -> String {
    format!("{:.1} GB", value as f64 / 1_000_000_000.)
}
pub fn speed(value: f64) -> String {
    if value >= 1_000_000_000. {
        format!("{:.1} GB/s", value / 1_000_000_000.)
    } else if value >= 1_000_000. {
        format!("{:.1} MB/s", value / 1_000_000.)
    } else if value >= 1_000. {
        format!("{:.0} KB/s", value / 1_000.)
    } else {
        format!("{value:.0} B/s")
    }
}
pub fn time(minutes: u32) -> String {
    if minutes < 60 {
        format!("{minutes} min")
    } else {
        format!("{} h {:02} min", minutes / 60, minutes % 60)
    }
}
pub fn battery_detail(battery: Battery) -> String {
    if battery.full && battery.plugged {
        "Carga completa · conectado".into()
    } else if battery.charging {
        battery
            .minutes
            .map(|m| format!("{} para completar la carga", time(m)))
            .unwrap_or_else(|| "Cargando · calculando tiempo…".into())
    } else if battery.plugged {
        "Conectado · carga en pausa".into()
    } else {
        battery
            .minutes
            .map(|m| format!("{} de autonomía estimada", time(m)))
            .unwrap_or_else(|| "En batería · calculando autonomía…".into())
    }
}

pub fn diagnose() {
    let args: Vec<String> = std::env::args().collect();
    let samples = args
        .windows(2)
        .find(|a| a[0] == "--samples")
        .and_then(|a| a[1].parse::<usize>().ok())
        .unwrap_or(5)
        .clamp(2, 3600);
    let mut sampler = autoreleasepool(|_| Sampler::new());
    let config = Config {
        interval: 1,
        battery_saver: false,
        ..Config::default()
    };
    let start = Instant::now();
    let cpu = platform::process_cpu_seconds();
    let mut cost = 0;
    for n in 0..samples {
        let s = autoreleasepool(|_| sampler.sample(&config));
        cost += s.sample_us;
        println!(
            "Muestra {}: CPU={:?}% RAM={:?} GPU={:?}% disco={:?} red={:?} batería={:?} lectura={} µs",
            n + 1,
            s.cpu.map(|p| (p * 10.).round() / 10.),
            s.memory,
            s.gpu,
            s.disk,
            s.network,
            s.battery,
            s.sample_us
        );
        if n + 1 < samples {
            thread::sleep(Duration::from_secs(1));
        }
    }
    println!(
        "RESUMEN: {} muestras; lectura media={} µs; CPU del proceso={:.3}% de un núcleo; RSS={:.1} MiB; duración={:.1} s",
        samples,
        cost / samples as u64,
        (platform::process_cpu_seconds() - cpu) / start.elapsed().as_secs_f64() * 100.,
        platform::resident_bytes() as f64 / 1_048_576.,
        start.elapsed().as_secs_f64()
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cpu_uses_deltas_and_idle() {
        assert_eq!(cpu_percent([10, 20, 30, 40], [20, 30, 100, 50]), Some(30.));
        assert_eq!(cpu_percent([0; 4], [0; 4]), None);
    }
    #[test]
    fn cpu_handles_tick_wrap() {
        assert_eq!(
            cpu_percent([u32::MAX - 1, 0, 0, 0], [2, 0, 4, 0]),
            Some(50.)
        );
    }
    #[test]
    fn network_handles_reset_new_interface_and_elapsed() {
        let before = [
            NetCounter {
                index: 1,
                received: 1000,
                sent: 500,
            },
            NetCounter {
                index: 2,
                received: 10000,
                sent: 10000,
            },
        ];
        let after = [
            NetCounter {
                index: 1,
                received: 3000,
                sent: 1500,
            },
            NetCounter {
                index: 2,
                received: 10,
                sent: 10,
            },
            NetCounter {
                index: 3,
                received: 999999,
                sent: 999999,
            },
        ];
        let r = network_rate(&before, &after, 2.).unwrap();
        assert_eq!(r.down, 1000.);
        assert_eq!(r.up, 500.);
        assert!(network_rate(&before, &after, 0.).is_none());
        assert!(network_rate(&before, &after, f64::NAN).is_none());
    }
    #[test]
    fn battery_states_do_not_invent_times() {
        let b = Battery {
            percent: 80.,
            plugged: true,
            ..Battery::default()
        };
        assert!(battery_detail(b).contains("pausa"));
        assert!(
            battery_detail(Battery {
                charging: true,
                ..b
            })
            .contains("calculando")
        );
        assert!(
            battery_detail(Battery {
                charging: true,
                minutes: Some(90),
                ..b
            })
            .contains("1 h 30 min")
        );
        assert!(battery_detail(Battery { full: true, ..b }).contains("completa"));
        assert!(
            battery_detail(Battery {
                plugged: false,
                minutes: Some(60),
                ..b
            })
            .contains("autonomía")
        );
    }
    #[test]
    fn units_and_boundaries() {
        assert_eq!(percent(5, 0), 0.);
        assert_eq!(percent(120, 100), 100.);
        assert_eq!(speed(1_500_000.), "1.5 MB/s");
        assert_eq!(speed(0.), "0 B/s");
        assert_eq!(time(45), "45 min");
    }
    #[test]
    fn disabling_collectors_clears_values() {
        autoreleasepool(|_| {
            let mut s = Sampler::new();
            s.sample(&Config::default());
            let disabled = Config {
                enabled: [false; 6],
                battery_saver: false,
                ..Config::default()
            };
            let snapshot = s.sample(&disabled);
            assert!(
                snapshot.cpu.is_none()
                    && snapshot.memory.is_none()
                    && snapshot.gpu.is_none()
                    && snapshot.disk.is_none()
                    && snapshot.network.is_none()
                    && snapshot.battery.is_none()
            );
        });
    }
    #[test]
    fn worker_stays_asleep_with_all_indicators_off() {
        let (tx, rx) = mpsc::channel();
        tx.send(Command::Stop).unwrap();
        autoreleasepool(|_| {
            worker_loop(
                &mut Sampler::new(),
                Config {
                    enabled: [false; 6],
                    ..Config::default()
                },
                &rx,
                |_| panic!("No debe muestrear"),
            )
        });
    }
}
