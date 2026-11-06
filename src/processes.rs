//! On-demand process sampling. No polling, children or process enumeration while closed.
use crate::platform;
use objc2::{msg_send, rc::autoreleasepool, runtime::AnyObject, sel};
use std::{
    collections::HashMap,
    ffi::CStr,
    io::Read,
    mem,
    os::fd::{AsRawFd, FromRawFd},
    process::{Child, Command, Stdio},
    ptr,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Sender},
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Debug)]
pub struct Identity {
    pub pid: i32,
    pub start: u64,
    pub name: String,
    pub path: String,
    pub app: String,
}
#[derive(Clone, Debug)]
pub struct Process {
    pub identity: Arc<Identity>,
    pub primary: f64,
    pub secondary: f64,
    pub memory: Option<u64>,
    pub prevents_sleep: bool,
}
#[derive(Clone, Debug, Default)]
pub struct DetailData {
    pub kind: usize,
    pub rows: Vec<Process>,
    pub ready: bool,
    pub unavailable: bool,
    pub note: String,
    pub facts: Vec<(String, String)>,
    pub sample_us: u64,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Request {
    pub kind: usize,
    pub interval: u64,
}
pub struct Shared {
    pub data: Mutex<Arc<DetailData>>,
    pub pending: AtomicBool,
}
pub struct Worker {
    pub tx: Sender<Option<Request>>,
    join: Option<thread::JoinHandle<()>>,
}
impl Worker {
    pub fn start(shared: Arc<Shared>, target: usize) -> Self {
        let (tx, rx) = mpsc::channel::<Option<Request>>();
        let join = thread::Builder::new().name("amstats.detail".into()).spawn(move || {
            unsafe { pthread_set_qos_class_self_np(0x11, 0); }
            let mut active = None;
            let mut collector = Collector::default();
            let mut next = Instant::now();
            loop {
                let event = match active { Some(_) => rx.recv_timeout(next.saturating_duration_since(Instant::now())), None => rx.recv().map_err(|_| mpsc::RecvTimeoutError::Disconnected) };
                match event {
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Ok(request) => {
                        if request != active { collector = Collector::default(); }
                        active = request;
                        next = Instant::now();
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        let request = active.unwrap();
                        let data = autoreleasepool(|_| collector.sample(request));
                        *shared.data.lock().unwrap() = Arc::new(data);
                        if !shared.pending.swap(true, Ordering::AcqRel) {
                            autoreleasepool(|_| unsafe { let _: () = msg_send![target as *mut AnyObject, performSelectorOnMainThread: sel!(detailRefresh:), withObject: ptr::null::<AnyObject>(), waitUntilDone: false]; });
                        }
                        next = Instant::now() + Duration::from_secs(request.interval.max(2));
                    }
                }
            }
        }).expect("No se pudo iniciar el detalle");
        Self {
            tx,
            join: Some(join),
        }
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        let (dummy, _) = mpsc::channel();
        drop(mem::replace(&mut self.tx, dummy));
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}
unsafe extern "C" {
    fn pthread_set_qos_class_self_np(class: u32, priority: i32) -> i32;
}

#[derive(Clone, Copy, Default)]
struct Counters {
    start: u64,
    cpu: u64,
    read: u64,
    write: u64,
    energy: Option<u64>,
}
#[repr(C)]
struct Usage {
    base: libc::rusage_info_v4,
    tail: [u64; 24],
}
fn usage(pid: i32, energy: bool) -> Option<(Counters, u64)> {
    let mut data: Usage = unsafe { mem::zeroed() };
    let p = (&mut data as *mut Usage).cast();
    let has_energy = energy && unsafe { libc::proc_pid_rusage(pid, 6, p) } == 0;
    if !has_energy && unsafe { libc::proc_pid_rusage(pid, 2, p) } != 0 {
        return None;
    }
    Some((
        Counters {
            start: data.base.ri_proc_start_abstime,
            cpu: data
                .base
                .ri_user_time
                .saturating_add(data.base.ri_system_time),
            read: data.base.ri_diskio_bytesread,
            write: data.base.ri_diskio_byteswritten,
            energy: has_energy.then_some(data.tail[5]),
        },
        data.base.ri_phys_footprint,
    ))
}
fn identity(pid: i32, start: u64, fallback: &str) -> Arc<Identity> {
    let mut buffer = [0i8; 4096];
    let path = if unsafe {
        libc::proc_pidpath(pid, buffer.as_mut_ptr().cast(), buffer.len() as u32)
    } > 0
    {
        unsafe { CStr::from_ptr(buffer.as_ptr()) }
            .to_string_lossy()
            .into_owned()
    } else {
        String::new()
    };
    let app = app_path(&path).unwrap_or_default();
    let name =
        if unsafe { libc::proc_name(pid, buffer.as_mut_ptr().cast(), buffer.len() as u32) } > 0 {
            unsafe { CStr::from_ptr(buffer.as_ptr()) }
                .to_string_lossy()
                .into_owned()
        } else if fallback.is_empty() {
            format!("Proceso {pid}")
        } else {
            fallback.to_string()
        };
    Arc::new(Identity {
        pid,
        start,
        name,
        path,
        app,
    })
}
pub fn app_path(path: &str) -> Option<String> {
    path.find(".app/").map(|end| path[..end + 4].to_string())
}
pub fn app_name(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}
pub fn rate(old: u64, new: u64, elapsed: f64) -> Option<f64> {
    (elapsed.is_finite() && elapsed > 0.)
        .then(|| new.checked_sub(old).map(|n| n as f64 / elapsed))
        .flatten()
}
pub fn memory(value: u64) -> String {
    if value >= 1_000_000_000 {
        format!("{:.2} GB", value as f64 / 1e9)
    } else {
        format!("{:.1} MB", value as f64 / 1e6)
    }
}

#[derive(Default)]
struct Collector {
    pids: Vec<i32>,
    previous: HashMap<i32, Counters>,
    names: HashMap<i32, Arc<Identity>>,
    at: Option<Instant>,
    gpu: HashMap<u64, (i32, u64)>,
    network: Option<NetReader>,
    network_failed: bool,
    facts: Vec<(String, String)>,
    facts_at: Option<Instant>,
    blockers: Vec<i32>,
}
impl Collector {
    fn sample(&mut self, request: Request) -> DetailData {
        let now = Instant::now();
        let elapsed = self.at.map(|then| now.duration_since(then).as_secs_f64());
        let mut result = DetailData {
            kind: request.kind,
            ready: elapsed.is_some() || request.kind == 1,
            ..Default::default()
        };
        let mut gpu_rates: HashMap<i32, f64> = HashMap::new();
        let mut gpu_names = HashMap::new();
        if request.kind == 2 {
            let clients = platform::gpu_clients();
            result.ready = elapsed.is_some() && !clients.is_empty();
            let mut current = HashMap::with_capacity(clients.len());
            for (id, pid, time, name) in clients {
                gpu_names.insert(pid, name);
                if let Some((old_pid, old_time)) = self.gpu.get(&id) {
                    if *old_pid == pid {
                        if let Some(value) = elapsed.and_then(|dt| rate(*old_time, time, dt)) {
                            *gpu_rates.entry(pid).or_default() += value / 1e6;
                        }
                    }
                }
                current.insert(id, (pid, time));
            }
            self.gpu = current;
            result.unavailable = self.gpu.is_empty();
            result.note = if self.gpu.is_empty() {
                "El controlador no publica actividad GPU por proceso."
            } else {
                "Tiempo GPU en ms/s. Puede solaparse entre tareas; no es un % del chip."
            }
            .into();
        }
        let net = if request.kind == 4 {
            if self.network.is_none() && !self.network_failed {
                match NetReader::new(request.interval) {
                    Ok(reader) => self.network = Some(reader),
                    Err(_) => self.network_failed = true,
                }
            }
            self.network
                .as_mut()
                .map(|reader| reader.poll())
                .unwrap_or_default()
        } else {
            HashMap::new()
        };
        if request.kind == 4 {
            result.ready = self.network.as_ref().is_some_and(|n| n.ready);
            result.unavailable =
                self.network_failed || self.network.as_ref().is_some_and(|n| n.failed);
            result.note = if result.unavailable {
                "macOS no ha permitido leer el tráfico por proceso."
            } else {
                "Tráfico TCP/UDP por proceso. VPN y tráfico local pueden diferir del total físico."
            }
            .into();
        }
        let count = unsafe { libc::proc_listallpids(ptr::null_mut(), 0) }.max(0) as usize;
        if self.pids.len() < count + 64 {
            self.pids.resize(count + 64, 0);
        }
        let len = unsafe {
            libc::proc_listallpids(
                self.pids.as_mut_ptr().cast(),
                (self.pids.len() * mem::size_of::<i32>()) as i32,
            )
        }
        .max(0) as usize;
        let cores = thread::available_parallelism().map_or(1., |n| n.get() as f64);
        let mut current = HashMap::with_capacity(len);
        let mut observed_energy = false;
        for &pid in &self.pids[..len.min(self.pids.len())] {
            if pid <= 0 {
                continue;
            }
            if request.kind == 2 && !self.gpu.values().any(|(owner, _)| *owner == pid) {
                continue;
            }
            if request.kind == 4 && !net.contains_key(&pid) {
                continue;
            }
            let usage = usage(pid, request.kind == 5);
            if usage.is_none() && !matches!(request.kind, 2 | 4) {
                continue;
            }
            let (counters, footprint) = usage.unwrap_or_default();
            observed_energy |= counters.energy.is_some_and(|v| v > 0);
            let old = self
                .previous
                .get(&pid)
                .filter(|c| c.start == counters.start);
            let delta =
                |before, after| elapsed.and_then(|dt| rate(before, after, dt)).unwrap_or(0.);
            let (primary, secondary) = match request.kind {
                0 => (
                    old.map_or(0., |o| delta(o.cpu, counters.cpu) / 1e9 * 100. / cores),
                    0.,
                ),
                1 => (footprint as f64, 0.),
                2 => (gpu_rates.get(&pid).copied().unwrap_or(0.), 0.),
                3 => (
                    old.map_or(0., |o| delta(o.read, counters.read)),
                    old.map_or(0., |o| delta(o.write, counters.write)),
                ),
                4 => net.get(&pid).copied().unwrap_or_default(),
                5 => (
                    old.and_then(|o| o.energy.zip(counters.energy))
                        .map_or(0., |(a, b)| delta(a, b) / 1e9),
                    0.,
                ),
                _ => (0., 0.),
            };
            let fallback = gpu_names
                .get(&pid)
                .cloned()
                .or_else(|| {
                    self.network
                        .as_ref()
                        .and_then(|n| n.names.get(&pid).cloned())
                })
                .unwrap_or_default();
            let metadata = self
                .names
                .entry(pid)
                .or_insert_with(|| identity(pid, counters.start, &fallback));
            if metadata.start != counters.start {
                *metadata = identity(pid, counters.start, &fallback);
            }
            result.rows.push(Process {
                identity: metadata.clone(),
                primary,
                secondary,
                memory: usage.map(|_| footprint),
                prevents_sleep: false,
            });
            current.insert(pid, counters);
        }
        self.names.retain(|pid, _| current.contains_key(pid));
        self.previous = current;
        if self
            .facts_at
            .is_none_or(|at| now.duration_since(at) >= Duration::from_secs(10))
        {
            self.facts.clear();
            match request.kind {
                0 => {
                    let mut load = [0.; 3];
                    unsafe {
                        libc::getloadavg(load.as_mut_ptr(), 3);
                    }
                    self.facts = vec![
                        ("NÚCLEOS LÓGICOS".into(), format!("{cores:.0}")),
                        ("CARGA · 1 MIN".into(), format!("{:.2}", load[0])),
                        ("PROCESOS LEGIBLES".into(), result.rows.len().to_string()),
                        ("INTERVALO".into(), format!("{} s", request.interval)),
                    ];
                }
                1 => {
                    let (swap, pressure) = platform::memory_details();
                    self.facts = vec![
                        (
                            "PRESIÓN".into(),
                            match pressure {
                                Some(1) => "Normal",
                                Some(2) => "Elevada",
                                Some(4) => "Crítica",
                                _ => "No disponible",
                            }
                            .into(),
                        ),
                        ("SWAP".into(), swap.map(memory).unwrap_or("—".into())),
                    ];
                }
                2 => {
                    self.facts = vec![
                        ("CLIENTES GRÁFICOS".into(), self.gpu.len().to_string()),
                        ("MEDICIÓN".into(), "Metal · ms/s".into()),
                    ]
                }
                3 => {
                    self.facts = vec![
                        (
                            "LECTURA".into(),
                            crate::metrics::speed(result.rows.iter().map(|p| p.primary).sum()),
                        ),
                        (
                            "ESCRITURA".into(),
                            crate::metrics::speed(result.rows.iter().map(|p| p.secondary).sum()),
                        ),
                    ]
                }
                4 => {
                    self.facts = vec![
                        ("INTERFACES".into(), platform::active_interfaces()),
                        ("ÁMBITO".into(), "TCP + UDP".into()),
                    ]
                }
                5 => {
                    let b = platform::battery_details();
                    self.blockers = platform::sleep_blockers();
                    self.facts = vec![
                        (
                            "CARGADOR · POTENCIA DECLARADA".into(),
                            if b.connected {
                                b.adapter_watts
                                    .map(|w| format!("{w:.0} W"))
                                    .unwrap_or("No disponible".into())
                            } else {
                                "Desconectado".into()
                            },
                        ),
                        (
                            "BATERÍA · CARGA / DESCARGA".into(),
                            b.battery_watts
                                .map(|w| format!("{w:+.1} W"))
                                .unwrap_or("—".into()),
                        ),
                        (
                            "SALUD · CAPACIDAD ESTIMADA".into(),
                            b.health.map(|v| format!("{v:.0}%")).unwrap_or("—".into()),
                        ),
                        (
                            "CICLOS · CAPACIDAD".into(),
                            format!(
                                "{} · {} mAh",
                                b.cycles.map(|v| v.to_string()).unwrap_or("—".into()),
                                b.capacity_mah.map(|v| v.to_string()).unwrap_or("—".into())
                            ),
                        ),
                    ];
                }
                _ => {}
            }
            self.facts_at = Some(now);
        }
        if request.kind == 5 {
            for row in &mut result.rows {
                row.prevents_sleep = self.blockers.contains(&row.identity.pid);
            }
            result.note = if observed_energy {
                "Potencia atribuida por el kernel; no mide el enchufe. ☾ impide reposo."
            } else {
                "Energía por proceso no disponible. Se muestran apps que impiden el reposo (☾)."
            }
            .into();
            if !observed_energy {
                result.rows.retain(|p| p.prevents_sleep);
            }
        }
        if request.kind == 0 {
            result.note = "CPU normalizada a la capacidad total del Mac (0–100%).".into();
        }
        if request.kind == 1 {
            result.note =
                "Huella física atribuida. La memoria compartida no se suma como RAM exclusiva."
                    .into();
        }
        if request.kind == 3 {
            result.note =
                "E/S de procesos legibles. Espacio ocupado y actividad son medidas distintas."
                    .into();
        }
        result.facts = self.facts.clone();
        result.sample_us = now.elapsed().as_micros() as u64;
        self.at = Some(now);
        result
    }
}

// A single system reader, only while the network panel is visible. Nonblocking stdout
// is drained by the detail worker; no extra reader thread and no repeated subprocesses.
struct NetReader {
    child: Child,
    output: std::fs::File,
    buffer: String,
    frame: HashMap<i32, (u64, u64)>,
    previous: HashMap<i32, (u64, u64)>,
    rates: HashMap<i32, (f64, f64)>,
    interval: f64,
    frames: usize,
    ready: bool,
    failed: bool,
    names: HashMap<i32, String>,
}
impl NetReader {
    fn new(interval: u64) -> std::io::Result<Self> {
        let (mut master, mut slave) = (0, 0);
        if unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
            )
        } != 0
        {
            return Err(std::io::Error::last_os_error());
        }
        // A PTY makes libc flush each CSV line instead of buffering several samples.
        let output = unsafe { std::fs::File::from_raw_fd(master) };
        let terminal = unsafe { std::fs::File::from_raw_fd(slave) };
        let child = Command::new("/usr/bin/nettop")
            .args([
                "-P",
                "-L",
                "0",
                "-n",
                "-x",
                "-J",
                "bytes_in,bytes_out",
                "-s",
                &interval.max(2).to_string(),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::from(terminal))
            .stderr(Stdio::null())
            .spawn()?;
        let fd = output.as_raw_fd();
        unsafe {
            libc::fcntl(
                fd,
                libc::F_SETFL,
                libc::fcntl(fd, libc::F_GETFL) | libc::O_NONBLOCK,
            );
        }
        Ok(Self {
            child,
            output,
            buffer: String::new(),
            frame: HashMap::new(),
            previous: HashMap::new(),
            rates: HashMap::new(),
            interval: interval.max(2) as f64,
            frames: 0,
            ready: false,
            failed: false,
            names: HashMap::new(),
        })
    }
    fn poll(&mut self) -> HashMap<i32, (f64, f64)> {
        let mut buffer = [0u8; 16384];
        // Bounded work even if the producer unexpectedly floods stdout.
        for _ in 0..64 {
            match self.output.read(&mut buffer) {
                Ok(0) => {
                    self.failed = true;
                    break;
                }
                Ok(len) => self
                    .buffer
                    .push_str(&String::from_utf8_lossy(&buffer[..len])),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(_) => {
                    self.failed = true;
                    break;
                }
            }
        }
        while let Some(end) = self.buffer.find('\n') {
            let line: String = self.buffer.drain(..=end).collect();
            if line.starts_with(",bytes_in,bytes_out") {
                if self.frames > 0 {
                    self.rates.clear();
                    for (pid, (down, up)) in &self.frame {
                        if let Some((a, b)) = self.previous.get(pid) {
                            self.rates.insert(
                                *pid,
                                (
                                    rate(*a, *down, self.interval).unwrap_or(0.),
                                    rate(*b, *up, self.interval).unwrap_or(0.),
                                ),
                            );
                        }
                    }
                    self.ready = self.frames > 1;
                    self.names.retain(|pid, _| self.frame.contains_key(pid));
                    self.previous = mem::take(&mut self.frame);
                }
                self.frames += 1;
            } else if let Some((pid, down, up)) = parse_net_line(&line) {
                self.frame.insert(pid, (down, up));
                if let Some(owner) = line
                    .trim()
                    .trim_end_matches(',')
                    .rsplitn(3, ',')
                    .nth(2)
                    .and_then(|v| v.rsplit_once('.'))
                    .map(|v| v.0)
                {
                    self.names.insert(pid, owner.to_string());
                }
            }
        }
        if self.buffer.len() > 1024 * 1024 {
            self.buffer.clear();
            self.failed = true;
        }
        if self.child.try_wait().ok().flatten().is_some() {
            self.failed = true;
        }
        self.rates.clone()
    }
}
impl Drop for NetReader {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
fn parse_net_line(line: &str) -> Option<(i32, u64, u64)> {
    let fields: Vec<&str> = line.trim().trim_end_matches(',').rsplitn(3, ',').collect();
    if fields.len() != 3 {
        return None;
    }
    let pid = fields[2].rsplit_once('.')?.1.parse::<i32>().ok()?;
    Some((pid, fields[1].parse().ok()?, fields[0].parse().ok()?))
}

#[derive(Clone, Debug)]
pub struct Group {
    pub name: String,
    pub app: String,
    pub primary: f64,
    pub secondary: f64,
    pub members: Vec<Process>,
}
pub fn ranked(rows: &[Process], grouped: bool, expanded: Option<&str>, order: usize) -> Vec<Group> {
    let mut groups: HashMap<String, Group> = HashMap::new();
    for row in rows {
        if expanded.is_some_and(|app| row.identity.app != app) {
            continue;
        }
        let app = &row.identity.app;
        let combine = grouped && expanded.is_none() && !app.is_empty();
        let key = if combine {
            app.clone()
        } else {
            format!("pid:{}:{}", row.identity.pid, row.identity.start)
        };
        let group = groups.entry(key).or_insert_with(|| Group {
            name: if combine {
                app_name(app)
            } else {
                row.identity.name.clone()
            },
            app: app.clone(),
            primary: 0.,
            secondary: 0.,
            members: Vec::new(),
        });
        group.primary += row.primary;
        group.secondary += row.secondary;
        group.members.push(row.clone());
    }
    let score = |g: &Group| match order {
        1 => g.primary,
        2 => g.secondary,
        _ => g.primary + g.secondary,
    };
    let mut groups: Vec<_> = groups.into_values().collect();
    groups.sort_by(|a, b| score(b).total_cmp(&score(a)).then(a.name.cmp(&b.name)));
    groups.truncate(10);
    groups
}

/// Exercise the real detail collectors without AppKit; not a full-app benchmark.
pub fn diagnose() {
    let args: Vec<String> = std::env::args().collect();
    let kind = args
        .windows(2)
        .find(|p| p[0] == "--diagnose-detail")
        .and_then(|p| p[1].parse::<usize>().ok())
        .unwrap_or(0)
        .min(5);
    let mut collector = Collector::default();
    for index in 0..if kind == 4 { 5 } else { 2 } {
        let data = autoreleasepool(|_| collector.sample(Request { kind, interval: 2 }));
        println!(
            "detail={kind} sample={index} ready={} rows={} cost_us={} facts={:?} note={}",
            data.ready,
            data.rows.len(),
            data.sample_us,
            data.facts,
            data.note
        );
        for row in ranked(&data.rows, true, None, 0).iter().take(5) {
            println!(
                "  {}: {:.3} / {:.3} [{} procesos]",
                row.name,
                row.primary,
                row.secondary,
                row.members.len()
            );
        }
        if index + 1 < if kind == 4 { 5 } else { 2 } {
            thread::sleep(Duration::from_secs(2));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn counters_reject_resets_and_invalid_time() {
        assert_eq!(rate(10, 9, 2.), None);
        assert_eq!(rate(10, 30, 2.), Some(10.));
        assert_eq!(rate(0, 10, 0.), None);
        assert_eq!(rate(0, 10, f64::NAN), None);
    }
    #[test]
    fn network_names_can_contain_dots_and_commas() {
        assert_eq!(
            parse_net_line("com.example, app.123,120,45,\n"),
            Some((123, 120, 45))
        );
        assert_eq!(parse_net_line(",bytes_in,bytes_out,"), None);
    }
    #[test]
    fn nested_helpers_belong_to_outer_application() {
        assert_eq!(
            app_path(
                "/Applications/Browser.app/Contents/Frameworks/Helper.app/Contents/MacOS/Helper"
            ),
            Some("/Applications/Browser.app".into())
        );
        assert_eq!(app_path("/usr/bin/tool"), None);
    }
    #[test]
    fn grouping_sums_and_orders_selected_metric() {
        let process = |pid, app: &str, a, b| Process {
            identity: Arc::new(Identity {
                pid,
                start: 1,
                name: format!("p{pid}"),
                path: String::new(),
                app: app.into(),
            }),
            primary: a,
            secondary: b,
            memory: Some(0),
            prevents_sleep: false,
        };
        let rows = vec![
            process(1, "/A.app", 3., 20.),
            process(2, "/A.app", 4., 0.),
            process(3, "/B.app", 10., 1.),
        ];
        assert_eq!(ranked(&rows, true, None, 1)[0].name, "B");
        let groups = ranked(&rows, true, None, 2);
        assert_eq!(groups[0].primary, 7.);
        assert_eq!(groups[0].members.len(), 2);
        assert_eq!(ranked(&rows, true, Some("/A.app"), 1).len(), 2);
    }
}
