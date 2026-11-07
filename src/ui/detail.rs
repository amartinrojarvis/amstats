//! One reusable native detail panel; bounded history and lazy, visible-only work.
use super::*;
use crate::processes::{self, DetailData, Group, Request};
use std::{
    collections::{HashMap, VecDeque},
    time::{Duration, Instant},
};
const WIDTH: f64 = 456.;
const HEIGHT: f64 = 722.;

#[derive(Default)]
pub(super) struct History {
    points: VecDeque<(Instant, [Option<f64>; 6], Option<f64>)>,
}
impl History {
    pub(super) fn push(&mut self, s: &Snapshot) {
        let Some(at) = s.sampled_at else {
            return;
        };
        if self.points.back().is_some_and(|p| p.0 == at) {
            return;
        }
        self.points.push_back((
            at,
            [
                s.cpu,
                s.memory.map(|m| metrics::percent(m.used, m.total)),
                s.gpu,
                s.disk.map(|d| metrics::percent(d.used, d.total)),
                s.network.map(|n| n.down),
                s.battery.map(|b| b.percent),
            ],
            s.network.map(|n| n.up),
        ));
        while self.points.len() > 61
            || self
                .points
                .front()
                .is_some_and(|p| at.duration_since(p.0) > Duration::from_secs(60))
        {
            self.points.pop_front();
        }
    }
}
struct ProcessView {
    button: Obj,
    icon: Obj,
    name: Obj,
    value: Obj,
    subtitle: Obj,
    last: String,
}
pub(super) struct Panel {
    // Worker must finish before Cocoa target is released by App.
    worker: processes::Worker,
    shared: Arc<processes::Shared>,
    view: Retained<AnyObject>,
    popover: Retained<AnyObject>,
    window: Option<Retained<AnyObject>>,
    title: Obj,
    value: Obj,
    subtitle: Obj,
    graph: Obj,
    graph_label: Obj,
    facts: Vec<(Obj, Obj)>,
    rows: Vec<ProcessView>,
    note: Obj,
    status: Obj,
    group_button: Obj,
    pause_button: Obj,
    pin_button: Obj,
    back_button: Obj,
    list_heading: Obj,
    order: Obj,
    tabs: Vec<Obj>,
    kind: usize,
    sort: usize,
    grouped: bool,
    expanded: Option<String>,
    paused: bool,
    hovering: bool,
    suspended: u8,
    data: Arc<DetailData>,
    displayed: Vec<Group>,
    icons: HashMap<String, Retained<AnyObject>>,
    request: Option<Request>,
    graph_sample: Option<Instant>,
}
unsafe fn tint(kind: usize) -> Obj {
    unsafe {
        match kind {
            0 => msg_send![class!(NSColor), systemTealColor],
            1 => msg_send![class!(NSColor), systemPurpleColor],
            2 => msg_send![class!(NSColor), systemOrangeColor],
            3 => msg_send![class!(NSColor), systemBlueColor],
            4 => msg_send![class!(NSColor), systemCyanColor],
            _ => msg_send![class!(NSColor), systemGreenColor],
        }
    }
}
unsafe fn set_text(view: Obj, text: &str) {
    unsafe {
        let previous: Retained<NSString> = msg_send![view, stringValue];
        if previous.to_string() != text {
            string(view, sel!(setStringValue:), text);
        }
    }
}
unsafe fn card(root: Obj, frame: NSRect) {
    unsafe {
        let view: Allocated<AnyObject> = msg_send![class!(NSBox), alloc];
        let view: Retained<AnyObject> = msg_send![view, initWithFrame: frame];
        let _: () = msg_send![&*view, setBoxType: 4usize];
        let _: () = msg_send![&*view, setBorderType: 0usize];
        let _: () = msg_send![&*view, setCornerRadius: 12f64];
        let base: Obj = msg_send![class!(NSColor), labelColor];
        let color: Obj = msg_send![base, colorWithAlphaComponent: 0.045f64];
        let _: () = msg_send![&*view, setFillColor: color];
        let _: () = msg_send![root, addSubview: &*view];
    }
}
impl Panel {
    unsafe fn new(target: Obj) -> Self {
        unsafe {
            let view: Allocated<AnyObject> = msg_send![class!(NSView), alloc];
            let view: Retained<AnyObject> =
                msg_send![view, initWithFrame: rect(0.,0.,WIDTH,HEIGHT)];
            // Keep the full status line above a separate row of navigation buttons.
            let body: Allocated<AnyObject> = msg_send![class!(NSView), alloc];
            let body: Retained<AnyObject> =
                msg_send![body, initWithFrame: rect(0.,32.,WIDTH,HEIGHT-32.)];
            let _: () = msg_send![&*view, addSubview: &*body];
            let root = Retained::as_ptr(&body) as Obj;
            let mut tabs = Vec::new();
            for (i, name) in ["CPU", "RAM", "GPU", "SSD", "Red", "Batería"]
                .iter()
                .enumerate()
            {
                let tab = button(
                    root,
                    rect(15. + i as f64 * 71., 648., 69., 28.),
                    name,
                    target,
                    sel!(detailTab:),
                    i as isize,
                    false,
                );
                let _: () = msg_send![tab, setButtonType: 6usize];
                tabs.push(tab);
            }
            let title = label(
                root,
                rect(24., 611., 265., 23.),
                "Procesador",
                15.,
                true,
                false,
            );
            let pin_button = button(
                root,
                rect(339., 608., 94., 28.),
                "Separar ↗",
                target,
                sel!(detailPin:),
                0,
                false,
            );
            let _: () = msg_send![pin_button, setToolTip: &*NSString::from_str("Alternar entre ventana independiente y panel en la barra de menús")];
            let value = label(root, rect(22., 562., 410., 48.), "—", 35., true, false);
            let subtitle = label(
                root,
                rect(24., 540., 410., 20.),
                "Preparando lecturas…",
                11.,
                false,
                true,
            );
            card(root, rect(20., 452., 416., 80.));
            let graph: Allocated<AnyObject> = msg_send![class!(AMStatsGraph), alloc];
            let graph: Retained<AnyObject> =
                msg_send![graph, initWithFrame: rect(30.,474.,396.,49.)];
            let _: () = msg_send![root, addSubview: &*graph];
            let graph_label = label(
                root,
                rect(32., 455., 390., 16.),
                "ÚLTIMO MINUTO",
                9.,
                false,
                true,
            );
            let mut facts = Vec::new();
            for i in 0..4 {
                let x = 20. + (i % 2) as f64 * 212.;
                let y = 398. - (i / 2) as f64 * 51.;
                card(root, rect(x, y, 204., 47.));
                let heading = label(root, rect(x + 10., y + 28., 184., 13.), "", 8., true, true);
                let number = label(
                    root,
                    rect(x + 10., y + 5., 184., 24.),
                    "—",
                    15.,
                    true,
                    false,
                );
                facts.push((heading, number));
            }
            let group_button = button(
                root,
                rect(17., 310., 147., 28.),
                "Agrupar apps",
                target,
                sel!(detailGroup:),
                0,
                true,
            );
            let pause_button = button(
                root,
                rect(168., 310., 82., 28.),
                "Pausar",
                target,
                sel!(detailPause:),
                0,
                false,
            );
            let _: () = msg_send![pause_button, setToolTip: &*NSString::from_str("Pausar o reanudar las consultas de procesos de este panel")];
            let order: Allocated<AnyObject> = msg_send![class!(NSPopUpButton), alloc];
            let order: Retained<AnyObject> =
                msg_send![order, initWithFrame: rect(260.,310.,178.,28.), pullsDown: false];
            let _: () = msg_send![&*order, setTarget: target];
            let _: () = msg_send![&*order, setAction: sel!(detailSort:)];
            let _: () =
                msg_send![&*order, setAccessibilityLabel: &*NSString::from_str("Ordenar procesos")];
            let _: () = msg_send![root, addSubview: &*order];
            let list_heading = label(
                root,
                rect(25., 282., 405., 18.),
                "PROCESOS · MAYOR CONSUMO",
                9.,
                true,
                true,
            );
            let back_button = button(
                root,
                rect(18., 282., 420., 24.),
                "‹ Todas las aplicaciones",
                target,
                sel!(detailBack:),
                0,
                false,
            );
            let mut rows = Vec::new();
            for i in 0..10 {
                let y = 258. - i as f64 * 19.;
                let hit = button(
                    root,
                    rect(19., y, 418., 20.),
                    "",
                    target,
                    sel!(detailProcess:),
                    i,
                    false,
                );
                let _: () = msg_send![hit, setBordered: false];
                let icon: Allocated<AnyObject> = msg_send![class!(AMStatsPassiveImage), alloc];
                let icon: Retained<AnyObject> =
                    msg_send![icon, initWithFrame: rect(27.,y+1.,17.,17.)];
                let _: () = msg_send![root, addSubview: &*icon];
                let name = label(root, rect(51., y + 1., 205., 18.), "", 11., i == 0, false);
                let sub = label(root, rect(260., y + 1., 40., 18.), "", 9., false, true);
                let val = label(root, rect(300., y + 1., 127., 18.), "", 11., true, false);
                let _: () = msg_send![val, setAlignment: 2isize];
                rows.push(ProcessView {
                    button: hit,
                    icon: Retained::as_ptr(&icon) as Obj,
                    name,
                    value: val,
                    subtitle: sub,
                    last: String::new(),
                });
            }
            let tracking: Allocated<AnyObject> = msg_send![class!(NSTrackingArea), alloc];
            let tracking: Retained<AnyObject> = msg_send![tracking, initWithRect: rect(19.,86.,418.,193.), options: 129usize, owner: target, userInfo: ptr::null::<AnyObject>()];
            let _: () = msg_send![root, addTrackingArea: &*tracking];
            let note = label(root, rect(24., 48., 408., 34.), "", 10., false, true);
            let _: () = msg_send![note, setMaximumNumberOfLines: 2isize];
            let _: () = msg_send![note, setLineBreakMode: 0isize];
            let status = label(
                root,
                rect(24., 16., 408., 25.),
                "Calculando actividad…",
                10.,
                false,
                true,
            );
            let footer_root = Retained::as_ptr(&view) as Obj;
            button(
                footer_root,
                rect(20., 13., 230., 28.),
                "Buscar actualizaciones…",
                target,
                sel!(checkUpdates:),
                0,
                false,
            );
            button(
                footer_root,
                rect(340., 13., 96., 28.),
                "Ajustes…",
                target,
                sel!(settings:),
                0,
                false,
            );
            let controller: Retained<AnyObject> = msg_send![class!(NSViewController), new];
            let _: () = msg_send![&*controller, setView: &*view];
            let popover: Retained<AnyObject> = msg_send![class!(NSPopover), new];
            let _: () = msg_send![&*popover, setContentViewController: &*controller];
            let _: () = msg_send![&*popover, setContentSize: NSSize::new(WIDTH,HEIGHT)];
            let _: () = msg_send![&*popover, setBehavior: 1isize];
            let _: () = msg_send![&*popover, setAnimates: false];
            let _: () = msg_send![&*popover, setDelegate: target];
            let shared = Arc::new(processes::Shared {
                data: Mutex::new(Arc::new(DetailData::default())),
                pending: AtomicBool::new(false),
            });
            let worker = processes::Worker::start(shared.clone(), target as usize);
            Self {
                worker,
                shared,
                view,
                popover,
                window: None,
                title,
                value,
                subtitle,
                graph: Retained::as_ptr(&graph) as Obj,
                graph_label,
                facts,
                rows,
                note,
                status,
                group_button,
                pause_button,
                pin_button,
                back_button,
                list_heading,
                order: Retained::as_ptr(&order) as Obj,
                tabs,
                kind: 0,
                sort: 0,
                grouped: true,
                expanded: None,
                paused: false,
                hovering: false,
                suspended: 0,
                data: Arc::new(DetailData::default()),
                displayed: Vec::new(),
                icons: HashMap::new(),
                request: None,
                graph_sample: None,
            }
        }
    }
    fn visible(&self) -> bool {
        unsafe {
            let shown: bool = msg_send![&*self.popover, isShown];
            shown
                || self
                    .window
                    .as_ref()
                    .is_some_and(|w| msg_send![&**w, isVisible])
        }
    }
    unsafe fn select(&mut self, kind: usize, sort: usize) {
        unsafe {
            self.kind = kind;
            self.graph_sample = None;
            self.sort = sort;
            self.expanded = None;
            self.hovering = false;
            self.paused = false;
            self.data = Arc::new(DetailData {
                kind,
                ..Default::default()
            });
            self.displayed.clear();
            for (index, tab) in self.tabs.iter().enumerate() {
                let _: () = msg_send![*tab, setState: isize::from(index == kind)];
            }
            let _: () = msg_send![self.order, removeAllItems];
            let titles: &[&str] = match kind {
                3 => &["Actividad total", "Lectura ↓", "Escritura ↑"],
                4 => &["Tráfico total", "Descarga ↓", "Subida ↑"],
                5 => &["Potencia atribuida"],
                _ => &["Mayor consumo"],
            };
            for title in titles {
                let _: () = msg_send![self.order, addItemWithTitle: &*NSString::from_str(title)];
            }
            let _: () = msg_send![self.order, selectItemAtIndex: sort as isize];
            string(self.pause_button, sel!(setTitle:), "Pausar");
            self.render_rows();
        }
    }
    unsafe fn render_rows(&mut self) {
        unsafe {
            if self.hovering || self.paused {
                return;
            }
            self.displayed = processes::ranked(
                &self.data.rows,
                self.grouped,
                self.expanded.as_deref(),
                self.sort,
            );
            if !self.data.ready {
                self.displayed.clear();
            }
            self.displayed.retain(|g| {
                g.primary + g.secondary > 0. || g.members.iter().any(|p| p.prevents_sleep)
            });
            let _: () = msg_send![self.list_heading, setHidden: self.expanded.is_some()];
            let _: () = msg_send![self.group_button, setState: isize::from(self.grouped)];
            let _: () = msg_send![self.back_button, setHidden: self.expanded.is_none()];
            if let Some(app) = &self.expanded {
                string(
                    self.back_button,
                    sel!(setTitle:),
                    &format!("‹ Todas las apps   ·   {}", processes::app_name(app)),
                );
            }
            for (i, row) in self.rows.iter_mut().enumerate() {
                let group = self.displayed.get(i);
                for view in [row.button, row.icon, row.name, row.value, row.subtitle] {
                    let _: () = msg_send![view, setHidden: group.is_none()];
                }
                let Some(group) = group else {
                    row.last.clear();
                    continue;
                };
                let value = match self.kind {
                    0 => {
                        if group.primary > 0. && group.primary < 0.1 {
                            "<0.1%".into()
                        } else {
                            format!("{:.1}%", group.primary)
                        }
                    }
                    1 => processes::memory(group.primary as u64),
                    2 => format!("{:.1} ms/s", group.primary),
                    3 | 4 => metrics::speed(match self.sort {
                        1 => group.primary,
                        2 => group.secondary,
                        _ => group.primary + group.secondary,
                    }),
                    _ => {
                        if group.primary < 1. {
                            format!("{:.1} mW", group.primary * 1000.)
                        } else {
                            format!("{:.2} W", group.primary)
                        }
                    }
                };
                let value = if !self.data.ready && self.kind != 1 {
                    "…".to_string()
                } else {
                    value
                };
                let mark = if group.members.iter().any(|p| p.prevents_sleep) {
                    "☾".to_string()
                } else if group.members.len() > 1 {
                    format!("{} ›", group.members.len())
                } else {
                    String::new()
                };
                let signature = format!("{}|{}|{}|{}", group.name, group.app, value, mark);
                if row.last != signature {
                    set_text(row.name, &group.name);
                    set_text(row.value, &value);
                    set_text(row.subtitle, &mark);
                    let _: () = msg_send![row.button, setAccessibilityLabel: &*NSString::from_str(&format!("{}: {}, {} procesos. Ver detalle", group.name, value, group.members.len()))];
                    let tooltip = if matches!(self.kind, 3 | 4) {
                        format!(
                            "↓ {} · ↑ {}",
                            metrics::speed(group.primary),
                            metrics::speed(group.secondary)
                        )
                    } else {
                        "Pulsa para ver los procesos y su identidad".into()
                    };
                    let _: () = msg_send![row.button, setToolTip: &*NSString::from_str(&tooltip)];
                    if !group.app.is_empty() && !self.icons.contains_key(&group.app) {
                        if self.icons.len() >= 16 {
                            self.icons.clear();
                        }
                        let workspace: Obj = msg_send![class!(NSWorkspace), sharedWorkspace];
                        let icon: Retained<AnyObject> =
                            msg_send![workspace, iconForFile: &*NSString::from_str(&group.app)];
                        self.icons.insert(group.app.clone(), icon);
                    }
                    let icon = self
                        .icons
                        .get(&group.app)
                        .map(|v| Retained::as_ptr(v) as Obj)
                        .unwrap_or(ptr::null_mut());
                    let _: () = msg_send![row.icon, setImage: icon];
                    row.last = signature;
                }
            }
            let status = if self.data.unavailable {
                "Datos por proceso no disponibles".into()
            } else if !self.data.ready {
                "Calculando… esperando dos lecturas".into()
            } else if self.displayed.is_empty() {
                "Sin procesos con datos disponibles".into()
            } else {
                format!(
                    "Top {} · lectura {:.1} ms",
                    self.displayed.len(),
                    self.data.sample_us as f64 / 1000.
                )
            };
            set_text(self.status, &status);
            set_text(self.note, &self.data.note);
        }
    }
}

impl App {
    unsafe fn detail_anchor(&self, kind: usize) -> Obj {
        unsafe {
            let index = if self.config.enabled[kind] {
                kind
            } else {
                self.config
                    .enabled
                    .iter()
                    .position(|&on| on)
                    .unwrap_or(kind)
            };
            if self.config.any_enabled() {
                msg_send![&*self.indicators[index].status, button]
            } else {
                msg_send![&*self.status, button]
            }
        }
    }
    unsafe fn detach_detail_window(&mut self) {
        unsafe {
            if let Some(p) = &mut self.details {
                if let Some(window) = p.window.take() {
                    let _: () = msg_send![&*window, orderOut: ptr::null::<AnyObject>()];
                    let blank: Retained<AnyObject> = msg_send![class!(NSView), new];
                    let _: () = msg_send![&*window, setContentView: &*blank];
                    let controller: Obj = msg_send![&*p.popover, contentViewController];
                    let _: () = msg_send![controller, setView: &*p.view];
                    string(p.pin_button, sel!(setTitle:), "Separar ↗");
                }
            }
        }
    }
    pub(super) fn detail_visible(&self) -> bool {
        self.details.as_ref().is_some_and(Panel::visible)
    }
    pub(super) fn hide_detail(&mut self) {
        if let Some(panel) = &mut self.details {
            unsafe {
                let _: () = msg_send![&*panel.popover, close];
                if let Some(window) = &panel.window {
                    let _: () = msg_send![&**window, orderOut: ptr::null::<AnyObject>()];
                }
            }
        }
        self.sync_detail();
    }
    pub(super) fn suspend_detail(&mut self, reason: u8, suspended: bool) {
        if let Some(panel) = &mut self.details {
            if suspended {
                panel.suspended |= reason;
            } else {
                panel.suspended &= !reason;
            }
        }
        self.sync_detail();
    }
    fn sync_detail(&mut self) {
        if let Some(panel) = &mut self.details {
            let on_battery = self
                .shared
                .snapshot
                .lock()
                .unwrap()
                .battery
                .is_some_and(|b| !b.plugged);
            let request =
                (panel.visible() && !panel.paused && panel.suspended == 0).then_some(Request {
                    kind: panel.kind,
                    interval: self.config.effective_interval(on_battery).max(2),
                });
            if panel.request != request {
                panel.request = request;
                let _ = panel.worker.tx.send(request);
            }
        }
    }
    pub(super) unsafe fn toggle_detail(&mut self, anchor: Obj) {
        unsafe {
            let tag: isize = msg_send![anchor, tag];
            let Some(kind) = indicator_kind(tag) else {
                self.show_dashboard(anchor);
                return;
            };
            let sort = 0;
            // Clicking the menu bar always opens its attached popover. A separate
            // window is only created by the explicit Separar button.
            self.detach_detail_window();
            let same = self
                .details
                .as_ref()
                .is_some_and(|p| p.kind == kind && (kind != 4 || p.sort == sort) && p.visible());
            if same {
                self.hide_detail();
                return;
            }
            self.open_detail(kind, sort, anchor);
        }
    }
    unsafe fn open_detail(&mut self, kind: usize, sort: usize, anchor: Obj) {
        unsafe {
            if let Some(popover) = &self.popover {
                let _: () = msg_send![&**popover, close];
            }
            if let Some(window) = &self.settings {
                let _: () = msg_send![&**window, orderOut: ptr::null::<AnyObject>()];
            }
            if let Some(summary) = &self.summary {
                let _: () = msg_send![&*summary.window, orderOut: ptr::null::<AnyObject>()];
            }
            if let Some(window) = &self.about {
                let _: () = msg_send![&**window, orderOut: ptr::null::<AnyObject>()];
            }
            if self.details.is_none() {
                self.details = Some(Panel::new(Retained::as_ptr(&self.target) as Obj));
            }
            let panel = self.details.as_mut().unwrap();
            panel.select(kind, sort);
            let application: Obj = msg_send![class!(NSApplication), sharedApplication];
            let _: () = msg_send![application, activateIgnoringOtherApps: true];
            if let Some(window) = &panel.window {
                let _: () = msg_send![&**window, makeKeyAndOrderFront: ptr::null::<AnyObject>()];
            } else {
                let shown: bool = msg_send![&*panel.popover, isShown];
                if shown {
                    let _: () = msg_send![&*panel.popover, close];
                }
                let bounds: NSRect = msg_send![anchor, bounds];
                let _: () = msg_send![&*panel.popover, showRelativeToRect: bounds, ofView: anchor, preferredEdge: 1usize];
                let window: Obj = msg_send![&*panel.view, window];
                let _: () = msg_send![window, makeKeyWindow];
            }
            let snapshot = self.shared.snapshot.lock().unwrap().clone();
            self.refresh_detail(&snapshot);
            self.sync_detail();
        }
    }
    pub(super) fn refresh_detail(&mut self, snapshot: &Snapshot) {
        self.sync_detail();
        let Some(panel) = &mut self.details else {
            return;
        };
        if !panel.visible() || panel.paused || panel.suspended != 0 {
            return;
        }
        unsafe {
            let kind = panel.kind;
            let (value, subtitle, _) = row_text(kind, snapshot);
            set_text(
                panel.title,
                [
                    "Procesador",
                    "Memoria",
                    "Gráficos",
                    "Almacenamiento",
                    "Actividad de red",
                    "Batería y alimentación",
                ][kind],
            );
            let value = if kind == 4 {
                snapshot
                    .network
                    .map(|n| format!("↑ {}   ↓ {}", metrics::speed(n.up), metrics::speed(n.down)))
                    .unwrap_or("↑ —   ↓ —".into())
            } else {
                value
            };
            set_text(panel.value, &value);
            let font: Obj = msg_send![class!(NSFont), monospacedDigitSystemFontOfSize: if kind == 4 { 23f64 } else { 35f64 }, weight: 0.23f64];
            let _: () = msg_send![panel.value, setFont: font];
            let _: () = msg_send![panel.value, setTextColor: tint(kind)];
            set_text(panel.subtitle, &subtitle);
            let mut facts = panel.data.facts.clone();
            match kind {
                1 => {
                    if let Some(m) = snapshot.memory {
                        facts.extend([
                            ("COMPRIMIDA".into(), processes::memory(m.compressed)),
                            (
                                "DISPONIBLE".into(),
                                processes::memory(m.total.saturating_sub(m.used)),
                            ),
                        ]);
                    }
                }
                2 => facts.extend([
                    ("CHIP".into(), "Apple Silicon".into()),
                    ("LECTURA".into(), "Bajo demanda".into()),
                ]),
                3 => {
                    if let Some(d) = snapshot.disk {
                        facts.extend([
                            ("LIBRE".into(), metrics::bytes(d.available)),
                            ("CAPACIDAD".into(), metrics::bytes(d.total)),
                        ]);
                    }
                }
                4 => facts.extend([
                    (
                        "RECIBIDO · SESIÓN".into(),
                        processes::memory(snapshot.network_total.down as u64),
                    ),
                    (
                        "ENVIADO · SESIÓN".into(),
                        processes::memory(snapshot.network_total.up as u64),
                    ),
                ]),
                _ => {}
            }
            for (i, (heading, number)) in panel.facts.iter().enumerate() {
                let fact = facts.get(i);
                set_text(*heading, fact.map_or("", |v| v.0.as_str()));
                set_text(*number, fact.map_or("—", |v| v.1.as_str()));
            }
            set_text(
                panel.graph_label,
                if kind == 4 {
                    "60 S   ·   ↑ SUBIDA ARRIBA   ·   ↓ BAJADA ABAJO"
                } else if kind == 3 {
                    "ÚLTIMO MINUTO   ·   ESPACIO OCUPADO"
                } else {
                    "ÚLTIMO MINUTO   ·   HISTORIAL LOCAL"
                },
            );
            if panel.graph_sample != snapshot.sampled_at {
                panel.graph_sample = snapshot.sampled_at;
                let _: () = msg_send![panel.graph, setNeedsDisplay: true];
            }
        }
    }
}
fn indicator_kind(tag: isize) -> Option<usize> {
    usize::try_from(tag).ok().filter(|&i| i < 6)
}
// Fixed dot matrix. Three batched paths, no animation timer or per-dot views.
extern "C-unwind" fn draw_graph(view: Obj, _: Sel, _: NSRect) {
    APP.with(|app| {
        let Ok(guard) = app.try_borrow() else {
            return;
        };
        let Some(app) = guard.as_ref() else {
            return;
        };
        let Some(panel) = &app.details else {
            return;
        };
        unsafe {
            let bounds: NSRect = msg_send![view, bounds];
            let points = &app.history.points;
            let Some(latest) = points.back() else {
                return;
            };
            let network = panel.kind == 4;
            let maximum = if network {
                points
                    .iter()
                    .flat_map(|(_, values, up)| [values[4].unwrap_or(0.), up.unwrap_or(0.)])
                    .fold(1024f64, f64::max)
            } else {
                100.
            };
            let columns = 40usize;
            let rows = 8usize;
            let dx = bounds.size.width / columns as f64;
            let dy = bounds.size.height / rows as f64;
            let dim: Retained<AnyObject> = msg_send![class!(NSBezierPath), bezierPath];
            let primary: Retained<AnyObject> = msg_send![class!(NSBezierPath), bezierPath];
            let secondary: Retained<AnyObject> = msg_send![class!(NSBezierPath), bezierPath];
            for col in 0..columns {
                let age = (columns - 1 - col) as f64 * 60. / columns as f64;
                let sample = points.iter().rev().find(|(at, _, _)| {
                    let delta = latest.0.duration_since(*at).as_secs_f64() - age;
                    (0. ..=12.).contains(&delta)
                });
                for row in 0..rows {
                    let cell = rect(
                        col as f64 * dx + 1.,
                        row as f64 * dy + 1.,
                        dx - 2.5,
                        dy - 2.,
                    );
                    let (value, level, levels, upload) = if network {
                        if row >= rows / 2 {
                            (sample.and_then(|p| p.2), row - rows / 2, rows / 2, true)
                        } else {
                            (
                                sample.and_then(|p| p.1[4]),
                                rows / 2 - 1 - row,
                                rows / 2,
                                false,
                            )
                        }
                    } else {
                        (sample.and_then(|p| p.1[panel.kind]), row, rows, false)
                    };
                    let lit = value.is_some_and(|v| {
                        v > 0.
                            && level < ((v / maximum).clamp(0., 1.) * levels as f64).ceil() as usize
                    });
                    let path = if !lit {
                        &dim
                    } else if upload {
                        &secondary
                    } else {
                        &primary
                    };
                    let _: () = msg_send![&**path, appendBezierPathWithRect: cell];
                }
            }
            let color: Obj = msg_send![class!(NSColor), tertiaryLabelColor];
            let dim_color: Obj = msg_send![color, colorWithAlphaComponent: 0.12f64];
            let _: () = msg_send![dim_color, setFill];
            let _: () = msg_send![&*dim, fill];
            let _: () = msg_send![tint(panel.kind), setFill];
            let _: () = msg_send![&*primary, fill];
            if network {
                let color: Obj = msg_send![class!(NSColor), systemPurpleColor];
                let _: () = msg_send![color, setFill];
                let _: () = msg_send![&*secondary, fill];
            }
        }
    });
}
extern "C-unwind" fn detail_refresh(_: Obj, _: Sel, _: Obj) {
    with_app(|app| {
        if let Some(panel) = &mut app.details {
            panel.shared.pending.store(false, Ordering::Release);
            let data = panel.shared.data.lock().unwrap().clone();
            if data.kind == panel.kind && panel.visible() && !panel.paused {
                panel.data = data;
                unsafe {
                    panel.render_rows();
                }
            }
        }
        let snapshot = app.shared.snapshot.lock().unwrap().clone();
        app.refresh_detail(&snapshot);
    });
}
extern "C-unwind" fn tab(_: Obj, _: Sel, sender: Obj) {
    with_app(|a| unsafe {
        let index: isize = msg_send![sender, tag];
        let from_menu: bool = msg_send![sender, isKindOfClass: class!(NSMenuItem)];
        if from_menu {
            a.detach_detail_window();
        }
        let anchor = a.detail_anchor(index as usize);
        a.open_detail(index as usize, 0, anchor);
    });
}
extern "C-unwind" fn closed(_: Obj, _: Sel, _: Obj) {
    with_app(|a| {
        if let Some(p) = &mut a.details {
            p.hovering = false;
            p.request = None;
            let _ = p.worker.tx.send(None);
        }
    });
}
extern "C-unwind" fn hovered(_: Obj, _: Sel, _: Obj) {
    with_app(|a| {
        if let Some(p) = &mut a.details {
            p.hovering = true;
            unsafe {
                set_text(p.status, "Lista fija mientras el cursor está encima");
            }
        }
    });
}
extern "C-unwind" fn exited(_: Obj, _: Sel, _: Obj) {
    with_app(|a| {
        if let Some(p) = &mut a.details {
            p.hovering = false;
            unsafe {
                p.render_rows();
            }
        }
    });
}
extern "C-unwind" fn group(_: Obj, _: Sel, _: Obj) {
    with_app(|a| {
        if let Some(p) = &mut a.details {
            p.grouped = !p.grouped;
            p.expanded = None;
            unsafe {
                p.render_rows();
            }
        }
    });
}
extern "C-unwind" fn back(_: Obj, _: Sel, _: Obj) {
    with_app(|a| {
        if let Some(p) = &mut a.details {
            p.expanded = None;
            p.hovering = false;
            unsafe {
                p.render_rows();
            }
        }
    });
}
extern "C-unwind" fn sort(_: Obj, _: Sel, sender: Obj) {
    with_app(|a| {
        if let Some(p) = &mut a.details {
            unsafe {
                let index: isize = msg_send![sender, indexOfSelectedItem];
                p.sort = index as usize;
                p.render_rows();
            }
        }
    });
}
extern "C-unwind" fn pause(_: Obj, _: Sel, _: Obj) {
    with_app(|a| {
        if let Some(p) = &mut a.details {
            p.paused = !p.paused;
            unsafe {
                string(
                    p.pause_button,
                    sel!(setTitle:),
                    if p.paused { "Reanudar" } else { "Pausar" },
                );
                if p.paused {
                    set_text(p.status, "Pausado · sin consultas de procesos");
                }
            }
        }
        a.sync_detail();
    });
}
impl App {
    unsafe fn pin_detail(&mut self) {
        unsafe {
            let anchor = self.details.as_ref().map(|p| self.detail_anchor(p.kind));
            let Some(p) = &mut self.details else {
                return;
            };
            if p.window.is_none() {
                let window: Allocated<AnyObject> = msg_send![class!(NSWindow), alloc];
                let window: Retained<AnyObject> = msg_send![window, initWithContentRect: rect(0.,0.,WIDTH,HEIGHT), styleMask: 3usize, backing: 2usize, defer: false];
                let _: () = msg_send![&*window, setReleasedWhenClosed: false];
                let _: () =
                    msg_send![&*window, setTitle: &*NSString::from_str("Detalle de AMStats")];
                let _: () = msg_send![&*window, setDelegate: &*self.target];
                let _: () = msg_send![&*p.popover, close];
                let controller: Obj = msg_send![&*p.popover, contentViewController];
                let blank: Retained<AnyObject> = msg_send![class!(NSView), new];
                let _: () = msg_send![controller, setView: &*blank];
                let _: () = msg_send![&*window, setContentView: &*p.view];
                let _: () = msg_send![&*window, center];
                let _: () = msg_send![&*window, makeKeyAndOrderFront: ptr::null::<AnyObject>()];
                p.window = Some(window);
                string(p.pin_button, sel!(setTitle:), "A la barra ↙");
            } else {
                let window = p.window.take().unwrap();
                let _: () = msg_send![&*window, orderOut: ptr::null::<AnyObject>()];
                let controller: Obj = msg_send![&*p.popover, contentViewController];
                let blank: Retained<AnyObject> = msg_send![class!(NSView), new];
                let _: () = msg_send![&*window, setContentView: &*blank];
                let _: () = msg_send![controller, setView: &*p.view];
                string(p.pin_button, sel!(setTitle:), "Separar ↗");
                let anchor = anchor.unwrap();
                let bounds: NSRect = msg_send![anchor, bounds];
                let _: () = msg_send![&*p.popover, showRelativeToRect: bounds, ofView: anchor, preferredEdge: 1usize];
                let window: Obj = msg_send![&*p.view, window];
                let _: () = msg_send![window, makeKeyWindow];
            }
            p.hovering = false;
            self.sync_detail();
        }
    }
}
extern "C-unwind" fn pin(_: Obj, _: Sel, _: Obj) {
    with_app(|a| unsafe {
        a.pin_detail();
    });
}
extern "C-unwind" fn process(_: Obj, _: Sel, sender: Obj) {
    let mut prepared = None;
    with_app(|a| unsafe {
        let Some(p) = &mut a.details else {
            return;
        };
        let index: isize = msg_send![sender, tag];
        let Some(group) = p.displayed.get(index as usize).cloned() else {
            return;
        };
        if group.members.len() > 1 && !group.app.is_empty() {
            p.expanded = Some(group.app);
            p.hovering = false;
            p.render_rows();
            return;
        }
        let Some(row) = group.members.first() else {
            return;
        };
        let identity = &row.identity;
        let text = format!(
            "PID {}\nMemoria: {}\n{}\n\n{}",
            identity.pid,
            row.memory
                .map(processes::memory)
                .unwrap_or("No disponible".into()),
            if row.prevents_sleep {
                "Impide el reposo de macOS"
            } else {
                ""
            },
            identity.path
        );
        let alert: Retained<AnyObject> = msg_send![class!(NSAlert), new];
        let _: () = msg_send![&*alert, setMessageText: &*NSString::from_str(&identity.name)];
        let _: () = msg_send![&*alert, setInformativeText: &*NSString::from_str(&text)];
        let _: Obj = msg_send![&*alert, addButtonWithTitle: &*NSString::from_str("Cerrar")];
        prepared = Some(alert);
    });
    // A modal runs a nested event loop. Release App's RefCell borrow before entering
    // it so sampler callbacks and their coalescing flags continue to be serviced.
    if let Some(alert) = prepared {
        unsafe {
            let _: isize = msg_send![&*alert, runModal];
        }
        with_app(|a| {
            if let Some(p) = &mut a.details {
                p.hovering = false;
                unsafe {
                    p.render_rows();
                }
            }
        });
    }
}
pub(super) unsafe fn register_views() {
    unsafe {
        let mut graph = ClassBuilder::new(c"AMStatsGraph", class!(NSView)).unwrap();
        graph.add_method(sel!(drawRect:), draw_graph as extern "C-unwind" fn(_, _, _));
        graph.register();
        for (name, base) in [
            (c"AMStatsPassiveLabel", class!(NSTextField)),
            (c"AMStatsPassiveImage", class!(NSImageView)),
        ] {
            let mut passive = ClassBuilder::new(name, base).unwrap();
            passive.add_method(
                sel!(hitTest:),
                strip_hit_test as extern "C-unwind" fn(_, _, _) -> Obj,
            );
            passive.register();
        }
    }
}
pub(super) unsafe fn register_actions(builder: &mut ClassBuilder) {
    unsafe {
        for (selector, action) in [
            (
                sel!(detailRefresh:),
                detail_refresh as extern "C-unwind" fn(Obj, Sel, Obj),
            ),
            (sel!(detailTab:), tab),
            (sel!(detailGroup:), group),
            (sel!(detailSort:), sort),
            (sel!(detailBack:), back),
            (sel!(detailPause:), pause),
            (sel!(detailPin:), pin),
            (sel!(detailProcess:), process),
            (sel!(popoverDidClose:), closed),
            (sel!(windowWillClose:), closed),
            (sel!(mouseEntered:), hovered),
            (sel!(mouseExited:), exited),
        ] {
            builder.add_method(selector, action);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn indicator_tags_are_stable_for_every_visibility_combination() {
        assert_eq!(indicator_kind(-1), None);
        assert_eq!(indicator_kind(6), None);
        for mask in 1..64 {
            let config = Config {
                enabled: std::array::from_fn(|i| mask & (1 << i) != 0),
                ..Default::default()
            };
            for cell in bar_cells(&config, &Snapshot::default()) {
                let metric = cell.metric.unwrap();
                assert_eq!(indicator_kind(metric as isize), Some(metric));
                if metric == 4 {
                    assert!(cell.top.starts_with('↑'));
                    assert!(cell.bottom.starts_with('↓'));
                }
            }
        }
    }
    #[test]
    fn history_is_bounded_and_ignores_duplicate_refreshes() {
        let mut history = History::default();
        let start = Instant::now();
        for i in 0..120 {
            let s = Snapshot {
                sampled_at: Some(start + Duration::from_secs(i)),
                cpu: Some(1.),
                ..Default::default()
            };
            history.push(&s);
            history.push(&s);
        }
        assert_eq!(history.points.len(), 61);
    }
}
