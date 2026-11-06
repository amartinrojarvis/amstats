//! AppKit stays on the main thread. Cocoa owns subviews; App owns windows,
//! status items and the callback target. Only changed visible text is redrawn.
use crate::{
    config::{Config, NAMES},
    metrics::{self, Command, Shared, Snapshot, Worker},
};
use objc2::{
    class, msg_send,
    rc::{Allocated, Retained, autoreleasepool},
    runtime::{AnyObject, Bool, ClassBuilder, Sel},
    sel,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use std::{
    cell::RefCell,
    ptr,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {}
#[link(name = "ServiceManagement", kind = "framework")]
unsafe extern "C" {}
mod detail;
type Obj = *mut AnyObject;
thread_local! { static APP: RefCell<Option<App>> = const { RefCell::new(None) }; }
struct Row {
    value: Obj,
    detail: Obj,
    gauge: Obj,
}
struct Summary {
    window: Retained<AnyObject>,
    rows: Vec<Row>,
    footer: Obj,
}
struct App {
    // Join the worker before releasing the callback target.
    worker: Worker,
    details: Option<detail::Panel>,
    history: detail::History,
    shared: Arc<Shared>,
    config: Config,
    target: Retained<AnyObject>,
    status: Retained<AnyObject>,
    indicators: Vec<Indicator>,
    strip: Retained<AnyObject>,
    cells: Vec<BarCell>,
    styles: Styles,
    last_tooltip: String,
    preview: Obj,
    popover: Option<Retained<AnyObject>>,
    rows: Vec<Row>,
    footer: Obj,
    settings: Option<Retained<AnyObject>>,
    summary: Option<Summary>,
    about: Option<Retained<AnyObject>>,
    updater: Result<crate::updates::Updater, String>,
    update_checks: Obj,
    update_install: Obj,
    switches: Vec<Obj>,
    interval: Obj,
    saver: Obj,
    login: Obj,
    settings_status: Obj,
    _power_notification: Option<crate::platform::PowerNotification>,
}
struct Indicator {
    status: Retained<AnyObject>,
    strip: Retained<AnyObject>,
}
fn with_app(action: impl FnOnce(&mut App)) {
    APP.with(|cell| {
        if let Ok(mut guard) = cell.try_borrow_mut() {
            if let Some(app) = guard.as_mut() {
                action(app);
            }
        }
    });
}
fn rect(x: f64, y: f64, w: f64, h: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(w, h))
}
unsafe fn string(object: Obj, selector: Sel, text: &str) {
    // All uses are NSString-taking setters with the same Objective-C ABI.
    use objc2::runtime::MessageReceiver;
    unsafe {
        let _: () = MessageReceiver::send_message(object, selector, (&*NSString::from_str(text),));
    }
}
unsafe fn label(
    parent: Obj,
    frame: NSRect,
    text: &str,
    size: f64,
    bold: bool,
    secondary: bool,
) -> Obj {
    unsafe {
        let view: Retained<AnyObject> =
            msg_send![class!(AMStatsPassiveLabel), labelWithString: &*NSString::from_str(text)];
        let font: Obj = msg_send![class!(NSFont), systemFontOfSize: size, weight: if bold { 0.23f64 } else { 0f64 }];
        let _: () = msg_send![&view, setFrame: frame];
        let _: () = msg_send![&view, setFont: font];
        let color: Obj = if secondary {
            msg_send![class!(NSColor), secondaryLabelColor]
        } else {
            msg_send![class!(NSColor), labelColor]
        };
        let _: () = msg_send![&view, setTextColor: color];
        let _: () = msg_send![&view, setLineBreakMode: 4isize];
        let _: () = msg_send![parent, addSubview: &*view];
        Retained::as_ptr(&view) as Obj
    }
}
unsafe fn button(
    parent: Obj,
    frame: NSRect,
    title: &str,
    target: Obj,
    action: Sel,
    tag: isize,
    checkbox: bool,
) -> Obj {
    unsafe {
        let view: Allocated<AnyObject> = msg_send![class!(NSButton), alloc];
        let view: Retained<AnyObject> = msg_send![view, initWithFrame: frame];
        let _: () = msg_send![&view, setTitle: &*NSString::from_str(title)];
        let _: () = msg_send![&view, setTarget: target];
        let _: () = msg_send![&view, setAction: action];
        let _: () = msg_send![&view, setTag: tag];
        if checkbox {
            let _: () = msg_send![&view, setButtonType: 3usize];
        } else {
            let _: () = msg_send![&view, setBezelStyle: 1usize];
        }
        let _: () = msg_send![parent, addSubview: &*view];
        Retained::as_ptr(&view) as Obj
    }
}
#[derive(Clone, PartialEq)]
struct BarCell {
    metric: Option<usize>,
    top: String,
    bottom: String,
    width: f64,
    network: bool,
}
struct Styles {
    header: Retained<AnyObject>,
    value: Retained<AnyObject>,
    network: Retained<AnyObject>,
    percent: Retained<AnyObject>,
}
unsafe fn text_style(size: f64, weight: f64, secondary: bool) -> Retained<AnyObject> {
    unsafe {
        let attrs: Retained<AnyObject> = msg_send![class!(NSMutableDictionary), new];
        let font: Obj =
            msg_send![class!(NSFont), monospacedDigitSystemFontOfSize: size, weight: weight];
        let paragraph: Retained<AnyObject> = msg_send![class!(NSMutableParagraphStyle), new];
        let _: () = msg_send![&*paragraph, setAlignment: 1isize];
        let color: Obj = if secondary {
            msg_send![class!(NSColor), secondaryLabelColor]
        } else {
            msg_send![class!(NSColor), labelColor]
        };
        let _: () = msg_send![&*attrs, setObject: font, forKey: &*NSString::from_str("NSFont")];
        let _: () = msg_send![&*attrs, setObject: &*paragraph, forKey: &*NSString::from_str("NSParagraphStyle")];
        let _: () = msg_send![&*attrs, setObject: color, forKey: &*NSString::from_str("NSColor")];
        attrs
    }
}
fn compact_speed(value: f64) -> String {
    let (value, suffix) = if value >= 1e9 {
        (value / 1e9, "G")
    } else if value >= 1e6 {
        (value / 1e6, "M")
    } else {
        (value / 1e3, "K")
    };
    if value >= 10. || suffix == "K" {
        format!("{value:.0}{suffix}")
    } else {
        format!("{value:.1}{suffix}")
    }
}
fn bar_cells(config: &Config, snapshot: &Snapshot) -> Vec<BarCell> {
    let mut cells = Vec::with_capacity(6);
    for i in 0..6 {
        if !config.enabled[i] {
            continue;
        }
        let (value, _, _) = row_text(i, snapshot);
        let (top, bottom, width) = match i {
            4 => snapshot
                .network
                .map(|n| {
                    (
                        format!("↑ {}", compact_speed(n.up)),
                        format!("↓ {}", compact_speed(n.down)),
                        42.,
                    )
                })
                .unwrap_or(("↑ —".into(), "↓ —".into(), 42.)),
            5 => {
                let top = snapshot
                    .battery
                    .map(|b| {
                        if let Some(m) = b.minutes {
                            let time = if m < 60 {
                                format!("{m}m")
                            } else {
                                format!("{}h{:02}", m / 60, m % 60)
                            };
                            format!("{} {time}", if b.charging { "↗" } else { "↘" })
                        } else if b.plugged {
                            "BAT · AC".into()
                        } else {
                            "BAT".into()
                        }
                    })
                    .unwrap_or("BAT".into());
                (top, value, 44.)
            }
            _ => (["CPU", "RAM", "GPU", "SSD"][i].into(), value, 27.),
        };
        cells.push(BarCell {
            metric: Some(i),
            top,
            bottom,
            width,
            network: i == 4,
        });
    }
    if cells.is_empty() {
        cells.push(BarCell {
            metric: None,
            top: "AM".into(),
            bottom: "·".into(),
            width: 26.,
            network: false,
        });
    }
    cells
}
fn strip_width(cells: &[BarCell]) -> f64 {
    cells.iter().map(|c| c.width).sum::<f64>() + cells.len().saturating_sub(1) as f64 * 2. + 8.
}
fn indicator_width(cell: &BarCell) -> f64 {
    cell.width + 2.
}
extern "C-unwind" fn draw_strip(view: *mut AnyObject, _: Sel, _: NSRect) {
    APP.with(|cell| {
        let Ok(guard) = cell.try_borrow() else { return; };
        let Some(app) = guard.as_ref() else { return; };
        unsafe {
            let bounds: NSRect = msg_send![view, bounds];
            let metric = app.indicators.iter().position(|indicator| Retained::as_ptr(&indicator.strip) == view);
            let cells = app.cells.iter().filter(|cell| metric.is_none_or(|i| cell.metric == Some(i)));
            let width = cells.clone().map(|c| c.width).sum::<f64>() + cells.clone().count().saturating_sub(1) as f64 * 2.;
            let mut x = ((bounds.size.width - width) / 2.).floor();
            let y = ((bounds.size.height - 22.) / 2.).floor();
            for cell in cells {
                let top_style = if cell.network { &app.styles.network } else { &app.styles.header };
                let bottom_style = if cell.network { &app.styles.network } else { &app.styles.value };
                let top = NSString::from_str(&cell.top); let bottom = NSString::from_str(&cell.bottom);
                let _: () = msg_send![&*top, drawInRect: rect(x,y+11.,cell.width,10.), withAttributes: &**top_style];
                if let Some(number) = cell.bottom.strip_suffix('%') {
                    let number = NSString::from_str(number);
                    let percent = NSString::from_str("%");
                    let number_size: NSSize = msg_send![&*number, sizeWithAttributes: &*app.styles.value];
                    let percent_size: NSSize = msg_send![&*percent, sizeWithAttributes: &*app.styles.percent];
                    let start = x + (cell.width - number_size.width - percent_size.width) / 2.;
                    let _: () = msg_send![&*number, drawInRect: rect(start,y,number_size.width+0.25,13.), withAttributes: &*app.styles.value];
                    let _: () = msg_send![&*percent, drawInRect: rect(start+number_size.width,y+1.5,percent_size.width+0.25,9.), withAttributes: &*app.styles.percent];
                } else {
                    let _: () = msg_send![&*bottom, drawInRect: rect(x,y,cell.width,13.), withAttributes: &**bottom_style];
                }
                x += cell.width + 2.;
            }
        }
    });
}
// Let the native status-bar button receive mouse events and handle highlighting.
extern "C-unwind" fn strip_hit_test(_: *mut AnyObject, _: Sel, _: NSPoint) -> Obj {
    ptr::null_mut()
}
unsafe fn make_strip(frame: NSRect) -> Retained<AnyObject> {
    unsafe {
        let view: Allocated<AnyObject> = msg_send![class!(AMStatsStrip), alloc];
        msg_send![view, initWithFrame: frame]
    }
}
unsafe fn make_popover(target: Obj) -> (Retained<AnyObject>, Vec<Row>, Obj) {
    unsafe {
        let view: Allocated<AnyObject> = msg_send![class!(NSView), alloc];
        let view: Retained<AnyObject> = msg_send![view, initWithFrame: rect(0.,0.,368.,580.)];
        let root = Retained::as_ptr(&view) as Obj;
        label(
            root,
            rect(22., 535., 240., 30.),
            "AMStats",
            23.,
            true,
            false,
        );
        label(
            root,
            rect(23., 515., 320., 18.),
            "Tu Mac, de un vistazo.",
            12.,
            false,
            true,
        );
        let mut rows = Vec::new();
        for (i, name) in NAMES.iter().enumerate() {
            let y = 442. - i as f64 * 70.;
            let card: Allocated<AnyObject> = msg_send![class!(NSBox), alloc];
            let card: Retained<AnyObject> = msg_send![card, initWithFrame: rect(16.,y,336.,64.)];
            let _: () = msg_send![&card, setBoxType: 6usize];
            let _: () = msg_send![&card, setBorderType: 0usize];
            let _: () = msg_send![&card, setCornerRadius: 9f64];
            let color: Obj = msg_send![class!(NSColor), controlBackgroundColor];
            let _: () = msg_send![&card, setFillColor: color];
            let _: () = msg_send![root, addSubview: &*card];
            label(root, rect(28., y + 39., 95., 17.), name, 12., true, false);
            let value = label(root, rect(118., y + 36., 220., 22.), "—", 17., true, false);
            let _: () = msg_send![value, setAlignment: 2isize];
            if i == 4 {
                let font: Obj = msg_send![class!(NSFont), monospacedDigitSystemFontOfSize: 12f64, weight: 0.23f64];
                let _: () = msg_send![value, setFont: font];
            }
            let detail = label(
                root,
                rect(28., y + 18., 310., 16.),
                "Esperando la primera lectura…",
                10.5,
                false,
                true,
            );
            let gauge: Allocated<AnyObject> = msg_send![class!(NSLevelIndicator), alloc];
            let gauge: Retained<AnyObject> =
                msg_send![gauge, initWithFrame: rect(28.,y+7.,310.,6.)];
            let _: () = msg_send![&gauge, setLevelIndicatorStyle: 1usize];
            let _: () = msg_send![&gauge, setMinValue: 0f64];
            let _: () = msg_send![&gauge, setMaxValue: 100f64];
            let _: () = msg_send![&gauge, setWarningValue: if i == 5 {101f64} else {80f64}];
            let _: () = msg_send![&gauge, setCriticalValue: if i == 5 {102f64} else {95f64}];
            let _: () = msg_send![&gauge, setEditable: false];
            let _: () = msg_send![&gauge, setAccessibilityLabel: &*NSString::from_str(&format!("Porcentaje de {name}"))];
            let _: () = msg_send![root, addSubview: &*gauge];
            rows.push(Row {
                value,
                detail,
                gauge: Retained::as_ptr(&gauge) as Obj,
            });
        }
        button(
            root,
            rect(18., 44., 102., 30.),
            "Ajustes…",
            target,
            sel!(settings:),
            0,
            false,
        );
        button(
            root,
            rect(269., 44., 82., 30.),
            "Salir",
            target,
            sel!(quit:),
            0,
            false,
        );
        button(
            root,
            rect(127., 44., 134., 30.),
            "Acerca de…",
            target,
            sel!(about:),
            0,
            false,
        );
        let footer = label(
            root,
            rect(23., 17., 323., 22.),
            "Muestreo eficiente · Apple Silicon",
            10.,
            false,
            true,
        );
        let controller: Retained<AnyObject> = msg_send![class!(NSViewController), new];
        let _: () = msg_send![&controller, setView: &*view];
        let popover: Retained<AnyObject> = msg_send![class!(NSPopover), new];
        let _: () = msg_send![&popover, setContentViewController: &*controller];
        let _: () = msg_send![&popover, setContentSize: NSSize::new(368.,580.)];
        let _: () = msg_send![&popover, setBehavior: 1isize];
        let _: () = msg_send![&popover, setAnimates: false];
        (popover, rows, footer)
    }
}
struct Settings {
    window: Retained<AnyObject>,
    switches: Vec<Obj>,
    interval: Obj,
    saver: Obj,
    login: Obj,
    status: Obj,
    preview: Obj,
}
unsafe fn make_settings(target: Obj, config: &Config) -> Settings {
    unsafe {
        let window: Allocated<AnyObject> = msg_send![class!(NSWindow), alloc];
        let window: Retained<AnyObject> = msg_send![window, initWithContentRect: rect(0.,0.,460.,570.), styleMask: 3usize, backing: 2usize, defer: false];
        let _: () = msg_send![&window, setTitle: &*NSString::from_str("Ajustes de AMStats")];
        let _: () = msg_send![&window, setReleasedWhenClosed: false];
        let _: () = msg_send![&window, center];
        let root: Obj = msg_send![&window, contentView];
        label(
            root,
            rect(28., 520., 400., 30.),
            "Menos ruido. Más información.",
            21.,
            true,
            false,
        );
        let preview = make_strip(rect(28., 478., 404., 30.));
        let _: () = msg_send![root, addSubview: &*preview];
        let _: () = msg_send![&*preview, setAccessibilityLabel: &*NSString::from_str("Vista previa de la barra de menús")];
        label(
            root,
            rect(28., 455., 404., 20.),
            "INDICADORES",
            10.,
            true,
            true,
        );
        let mut switches = Vec::new();
        for (i, name) in NAMES.iter().enumerate() {
            let x = 26. + (i % 2) as f64 * 215.;
            let y = 414. - (i / 2) as f64 * 38.;
            let b = button(
                root,
                rect(x, y, 195., 28.),
                name,
                target,
                sel!(change:),
                i as isize,
                true,
            );
            let _: () = msg_send![b, setState: if config.enabled[i] { 1isize } else { 0isize }];
            switches.push(b);
        }
        label(
            root,
            rect(28., 302., 404., 20.),
            "Los indicadores ocultos dejan de medir su uso.",
            11.,
            false,
            true,
        );
        label(
            root,
            rect(28., 263., 404., 20.),
            "FRECUENCIA DE MUESTREO",
            10.,
            true,
            true,
        );
        let interval: Allocated<AnyObject> = msg_send![class!(NSPopUpButton), alloc];
        let interval: Retained<AnyObject> =
            msg_send![interval, initWithFrame: rect(25.,224.,407.,30.), pullsDown: false];
        for title in [
            "1 segundo · máxima respuesta",
            "2 segundos · equilibrado",
            "5 segundos · mínimo consumo",
        ] {
            let _: () = msg_send![&interval, addItemWithTitle: &*NSString::from_str(title)];
        }
        let _: () = msg_send![&interval, selectItemAtIndex: match config.interval { 1 => 0isize, 5 => 2, _ => 1 }];
        let _: () = msg_send![&interval, setTarget: target];
        let _: () = msg_send![&interval, setAction: sel!(change:)];
        let _: () = msg_send![&interval, setTag: 10isize];
        let _: () = msg_send![&interval, setAccessibilityLabel: &*NSString::from_str("Intervalo de actualización")];
        let _: () = msg_send![root, addSubview: &*interval];
        let saver = button(
            root,
            rect(26., 183., 405., 28.),
            "Reducir la frecuencia al usar batería",
            target,
            sel!(change:),
            11,
            true,
        );
        let _: () = msg_send![saver, setState: if config.battery_saver { 1isize } else { 0isize }];
        label(
            root,
            rect(28., 156., 404., 20.),
            "Duplica el intervalo. Disco y batería: cada 30 s.",
            11.,
            false,
            true,
        );
        let login = button(
            root,
            rect(26., 112., 405., 28.),
            "Abrir AMStats al iniciar sesión",
            target,
            sel!(change:),
            12,
            true,
        );
        button(
            root,
            rect(25., 66., 195., 29.),
            "Ítems de inicio de macOS…",
            target,
            sel!(loginSettings:),
            0,
            false,
        );
        button(
            root,
            rect(300., 66., 132., 29.),
            "Ver resumen",
            target,
            sel!(summary:),
            0,
            false,
        );
        let status = label(
            root,
            rect(28., 17., 404., 40.),
            "Los cambios se guardan automáticamente.",
            11.,
            false,
            true,
        );
        let _: () = msg_send![status, setMaximumNumberOfLines: 2isize];
        let _: () = msg_send![status, setLineBreakMode: 0isize];
        Settings {
            window,
            switches,
            interval: Retained::as_ptr(&interval) as Obj,
            saver,
            login,
            status,
            preview: Retained::as_ptr(&preview) as Obj,
        }
    }
}

impl App {
    unsafe fn new(target: Retained<AnyObject>) -> Self {
        unsafe {
            let config = Config::load();
            let target_ptr = Retained::as_ptr(&target) as Obj;
            let bar: Obj = msg_send![class!(NSStatusBar), systemStatusBar];
            let status: Retained<AnyObject> = msg_send![bar, statusItemWithLength: 212f64];
            let _: () =
                msg_send![&*status, setAutosaveName: &*NSString::from_str("AMStats.Compact")];
            let _: () = msg_send![&*status, setVisible: true];
            let button: Obj = msg_send![&*status, button];
            let _: () = msg_send![button, setTag: -1isize];
            let _: () = msg_send![button, setTarget: target_ptr];
            let _: () = msg_send![button, setAction: sel!(toggle:)];
            let _: isize = msg_send![button, sendActionOn: (1usize << 2) | (1usize << 4)];
            let _: () = msg_send![button, setAccessibilityLabel: &*NSString::from_str("AMStats · monitor del sistema")];
            let strip = make_strip(rect(0., 0., 212., 22.));
            let _: () = msg_send![button, addSubview: &*strip];
            let mut indicators = Vec::new();
            // macOS places new status items to the left. Each indicator owns its
            // native button and stable tag; selection never depends on currentEvent.x.
            for i in (0..6).rev() {
                let item: Retained<AnyObject> = msg_send![bar, statusItemWithLength: 29f64];
                let _: () = msg_send![&*item, setAutosaveName: &*NSString::from_str(&format!("AMStats.Metric.{i}"))];
                let button: Obj = msg_send![&*item, button];
                let _: () = msg_send![button, setTag: i as isize];
                let _: () = msg_send![button, setTarget: target_ptr];
                let _: () = msg_send![button, setAction: sel!(toggle:)];
                let _: isize = msg_send![button, sendActionOn: (1usize << 2) | (1usize << 4)];
                let cell: Obj = msg_send![button, cell];
                let _: () = msg_send![cell, setHighlightsBy: 0usize];
                let _: () = msg_send![button, setAccessibilityLabel: &*NSString::from_str(&format!("AMStats · {}", NAMES[i]))];
                let strip = make_strip(rect(0., 0., 29., 22.));
                let _: () = msg_send![button, addSubview: &*strip];
                indicators.push(Indicator {
                    status: item,
                    strip,
                });
            }
            indicators.reverse();
            let styles = Styles {
                header: text_style(7., 0.23, true),
                value: text_style(10.5, 0., false),
                network: text_style(9., 0., false),
                percent: text_style(6.5, 0., false),
            };
            let shared = Arc::new(Shared {
                snapshot: Mutex::new(Snapshot::default()),
                pending: AtomicBool::new(false),
            });
            let worker = Worker::start(config.clone(), shared.clone(), target_ptr as usize);
            let mut app = Self {
                worker,
                details: None,
                history: detail::History::default(),
                shared,
                config,
                target,
                status,
                indicators,
                strip,
                cells: Vec::new(),
                styles,
                last_tooltip: String::new(),
                preview: ptr::null_mut(),
                popover: None,
                rows: Vec::new(),
                footer: ptr::null_mut(),
                settings: None,
                summary: None,
                about: None,
                updater: crate::updates::Updater::new(),
                update_checks: ptr::null_mut(),
                update_install: ptr::null_mut(),
                switches: Vec::new(),
                interval: ptr::null_mut(),
                saver: ptr::null_mut(),
                login: ptr::null_mut(),
                settings_status: ptr::null_mut(),
                _power_notification: crate::platform::PowerNotification::new(power_changed),
            };
            app.refresh();
            app.observe();
            app
        }
    }
    fn prepare_dashboard(&mut self) {
        if self.popover.is_none() {
            let (popover, rows, footer) =
                unsafe { make_popover(Retained::as_ptr(&self.target) as Obj) };
            self.popover = Some(popover);
            self.rows = rows;
            self.footer = footer;
        }
    }
    fn popover_shown(&self) -> bool {
        self.popover
            .as_ref()
            .is_some_and(|p| unsafe { msg_send![&**p, isShown] })
    }
    unsafe fn show_dashboard(&mut self, anchor: Obj) {
        unsafe {
            self.prepare_dashboard();
            let snapshot = self.shared.snapshot.lock().unwrap().clone();
            self.refresh_rows(&snapshot);
            let app: Obj = msg_send![class!(NSApplication), sharedApplication];
            let _: () = msg_send![app, activateIgnoringOtherApps: true];
            let bounds: NSRect = msg_send![anchor, bounds];
            let _: () = msg_send![&**self.popover.as_ref().unwrap(), showRelativeToRect: bounds, ofView: anchor, preferredEdge: 1usize];
        }
    }
    fn show_summary(&mut self) {
        self.hide_detail();
        unsafe {
            if let Some(settings) = &self.settings {
                let _: () = msg_send![&**settings, orderOut: ptr::null::<AnyObject>()];
            }
            if self.summary.is_none() {
                let (popover, rows, footer) = make_popover(Retained::as_ptr(&self.target) as Obj);
                let controller: Obj = msg_send![&*popover, contentViewController];
                let view: Retained<AnyObject> = msg_send![controller, view];
                let window: Allocated<AnyObject> = msg_send![class!(NSWindow), alloc];
                let window: Retained<AnyObject> = msg_send![window, initWithContentRect: rect(0.,0.,368.,580.), styleMask: 3usize, backing: 2usize, defer: false];
                let _: () =
                    msg_send![&*window, setTitle: &*NSString::from_str("Resumen de AMStats")];
                let _: () = msg_send![&*window, setReleasedWhenClosed: false];
                let _: () = msg_send![&*window, setContentView: &*view];
                let _: () = msg_send![&*window, center];
                self.summary = Some(Summary {
                    window,
                    rows,
                    footer,
                });
            }
            let panel = self.summary.as_ref().unwrap();
            let snapshot = self.shared.snapshot.lock().unwrap().clone();
            self.refresh_content(&panel.rows, panel.footer, &snapshot);
            let _: () = msg_send![&*panel.window, makeKeyAndOrderFront: ptr::null::<AnyObject>()];
        }
    }
    fn show_settings(&mut self) {
        self.hide_detail();
        unsafe {
            if let Some(popover) = &self.popover {
                let _: () = msg_send![&**popover, close];
            }
            if self.settings.is_none() {
                let settings = make_settings(Retained::as_ptr(&self.target) as Obj, &self.config);
                self.settings = Some(settings.window);
                self.switches = settings.switches;
                self.interval = settings.interval;
                self.saver = settings.saver;
                self.login = settings.login;
                self.settings_status = settings.status;
                self.preview = settings.preview;
            }
            self.refresh_login();
            let app: Obj = msg_send![class!(NSApplication), sharedApplication];
            let _: () = msg_send![app, activateIgnoringOtherApps: true];
            let _: () = msg_send![&**self.settings.as_ref().unwrap(), makeKeyAndOrderFront: ptr::null::<AnyObject>()];
        }
    }
    fn show_about(&mut self) {
        self.hide_detail();
        unsafe {
            if let Some(popover) = &self.popover {
                let _: () = msg_send![&**popover, close];
            }
            if self.about.is_none() {
                let window: Allocated<AnyObject> = msg_send![class!(NSWindow), alloc];
                let window: Retained<AnyObject> = msg_send![window, initWithContentRect: rect(0.,0.,460.,420.), styleMask: 3usize, backing: 2usize, defer: false];
                let _: () =
                    msg_send![&*window, setTitle: &*NSString::from_str("Acerca de AMStats")];
                let _: () = msg_send![&*window, setReleasedWhenClosed: false];
                let _: () = msg_send![&*window, center];
                let root: Obj = msg_send![&*window, contentView];
                let target = Retained::as_ptr(&self.target) as Obj;
                label(
                    root,
                    rect(28., 363., 404., 32.),
                    "AMStats",
                    26.,
                    true,
                    false,
                );
                label(
                    root,
                    rect(28., 336., 404., 22.),
                    &format!(
                        "Versión {} · macOS · Apple Silicon",
                        crate::updates::VERSION
                    ),
                    12.,
                    false,
                    true,
                );
                label(
                    root,
                    rect(28., 296., 404., 24.),
                    "Creado por Alberto Martín",
                    17.,
                    true,
                    false,
                );
                button(
                    root,
                    rect(24., 253., 198., 30.),
                    "X · @amartinro",
                    target,
                    sel!(openLink:),
                    0,
                    false,
                );
                button(
                    root,
                    rect(234., 253., 202., 30.),
                    "Instagram · @amartinro",
                    target,
                    sel!(openLink:),
                    1,
                    false,
                );
                button(
                    root,
                    rect(24., 214., 412., 30.),
                    "Código fuente y novedades en GitHub",
                    target,
                    sel!(openLink:),
                    2,
                    false,
                );
                label(
                    root,
                    rect(28., 174., 404., 20.),
                    "ACTUALIZACIONES",
                    10.,
                    true,
                    true,
                );
                self.update_checks = button(
                    root,
                    rect(24., 139., 412., 28.),
                    "Buscar versiones nuevas automáticamente",
                    target,
                    sel!(updatePreference:),
                    0,
                    true,
                );
                self.update_install = button(
                    root,
                    rect(24., 106., 412., 28.),
                    "Descargar e instalar actualizaciones automáticamente",
                    target,
                    sel!(updatePreference:),
                    1,
                    true,
                );
                button(
                    root,
                    rect(24., 61., 412., 32.),
                    "Buscar actualizaciones…",
                    target,
                    sel!(checkUpdates:),
                    0,
                    false,
                );
                label(
                    root,
                    rect(28., 17., 404., 28.),
                    "© 2026 Alberto Martín · MIT · Actualizaciones con Sparkle",
                    10.,
                    false,
                    true,
                );
                self.about = Some(window);
            }
            self.refresh_update_preferences();
            let app: Obj = msg_send![class!(NSApplication), sharedApplication];
            let _: () = msg_send![app, activateIgnoringOtherApps: true];
            let _: () = msg_send![&**self.about.as_ref().unwrap(), makeKeyAndOrderFront: ptr::null::<AnyObject>()];
        }
    }
    fn refresh_update_preferences(&self) {
        if self.about.is_none() {
            return;
        }
        unsafe {
            let (checks, install) = self
                .updater
                .as_ref()
                .map(|u| u.preferences())
                .unwrap_or_default();
            let _: () = msg_send![self.update_checks, setState: isize::from(checks)];
            let _: () = msg_send![self.update_checks, setEnabled: self.updater.is_ok()];
            let _: () = msg_send![self.update_install, setState: isize::from(install)];
            let _: () = msg_send![self.update_install, setEnabled: self.updater.is_ok() && checks];
        }
    }
    fn check_updates(&self) {
        unsafe {
            if let Some(popover) = &self.popover {
                let _: () = msg_send![&**popover, close];
            }
            let app: Obj = msg_send![class!(NSApplication), sharedApplication];
            let _: () = msg_send![app, activateIgnoringOtherApps: true];
            match &self.updater {
                Ok(updater) => updater.check(),
                Err(message) => {
                    let alert: Retained<AnyObject> = msg_send![class!(NSAlert), new];
                    let _: () = msg_send![&*alert, setMessageText: &*NSString::from_str("Actualizador no disponible")];
                    let _: () =
                        msg_send![&*alert, setInformativeText: &*NSString::from_str(message)];
                    let _: isize = msg_send![&*alert, runModal];
                }
            }
        }
    }
    fn refresh(&mut self) {
        self.shared.pending.store(false, Ordering::Release);
        let snapshot = self.shared.snapshot.lock().unwrap().clone();
        self.history.push(&snapshot);
        self.refresh_detail(&snapshot);
        unsafe {
            let cells = bar_cells(&self.config, &snapshot);
            if cells != self.cells {
                let width = strip_width(&cells);
                let old_width = strip_width(&self.cells);
                let layout_changed = cells.len() != self.cells.len()
                    || cells
                        .iter()
                        .zip(&self.cells)
                        .any(|(a, b)| a.metric != b.metric || a.width != b.width);
                let previous = std::mem::replace(&mut self.cells, cells);
                if layout_changed {
                    let _: () = msg_send![&*self.status, setVisible: !self.config.any_enabled()];
                }
                for (i, indicator) in self.indicators.iter().enumerate() {
                    if layout_changed {
                        let _: () =
                            msg_send![&*indicator.status, setVisible: self.config.enabled[i]];
                    }
                    if let Some(cell) = self.cells.iter().find(|c| c.metric == Some(i)) {
                        if previous.iter().find(|c| c.metric == Some(i)) == Some(cell) {
                            continue;
                        }
                        let button: Obj = msg_send![&*indicator.status, button];
                        if layout_changed {
                            let width = indicator_width(cell);
                            let _: () = msg_send![&*indicator.status, setLength: width];
                            let bounds: NSRect = msg_send![button, bounds];
                            let _: () = msg_send![&*indicator.strip, setFrame: rect(0.,0.,width,bounds.size.height)];
                        }
                        let _: () = msg_send![&*indicator.strip, setNeedsDisplay: true];
                        let (value, detail, _) = row_text(i, &snapshot);
                        let tooltip =
                            NSString::from_str(&format!("{}: {} · {}", NAMES[i], value, detail));
                        let _: () = msg_send![button, setToolTip: &*tooltip];
                        let _: () = msg_send![button, setAccessibilityValue: &*tooltip];
                    }
                }
                if width != old_width {
                    let _: () = msg_send![&*self.status, setLength: width];
                    let button: Obj = msg_send![&*self.status, button];
                    let bounds: NSRect = msg_send![button, bounds];
                    let _: () =
                        msg_send![&*self.strip, setFrame: rect(0.,0.,width,bounds.size.height)];
                }
                let _: () = msg_send![&*self.strip, setNeedsDisplay: true];
                if !self.preview.is_null() {
                    let visible: bool = msg_send![&**self.settings.as_ref().unwrap(), isVisible];
                    if visible {
                        let _: () = msg_send![self.preview, setNeedsDisplay: true];
                    }
                }
            }
            let tooltip = (0..6)
                .filter(|i| self.config.enabled[*i])
                .map(|i| {
                    let (value, detail, _) = row_text(i, &snapshot);
                    format!("{}: {} · {}", NAMES[i], value, detail)
                })
                .collect::<Vec<_>>()
                .join("\n");
            if tooltip != self.last_tooltip {
                let button: Obj = msg_send![&*self.status, button];
                let _: () = msg_send![button, setToolTip: &*NSString::from_str(&tooltip)];
                let _: () =
                    msg_send![button, setAccessibilityValue: &*NSString::from_str(&tooltip)];
                self.last_tooltip = tooltip;
            }
            if self.popover_shown() {
                self.refresh_rows(&snapshot);
            }
            if let Some(panel) = &self.summary {
                let visible: bool = msg_send![&*panel.window, isVisible];
                if visible {
                    self.refresh_content(&panel.rows, panel.footer, &snapshot);
                }
            }
        }
    }
    unsafe fn refresh_rows(&self, s: &Snapshot) {
        unsafe {
            self.refresh_content(&self.rows, self.footer, s);
        }
    }
    unsafe fn refresh_content(&self, rows: &[Row], footer_view: Obj, s: &Snapshot) {
        unsafe {
            for (i, row) in rows.iter().enumerate() {
                let (value, detail, gauge) = if self.config.enabled[i] {
                    row_text(i, s)
                } else {
                    ("—".into(), "Indicador desactivado".into(), None)
                };
                string(row.value, sel!(setStringValue:), &value);
                string(row.detail, sel!(setStringValue:), &detail);
                let _: () = msg_send![row.gauge, setHidden: gauge.is_none()];
                if let Some(value) = gauge {
                    let _: () = msg_send![row.gauge, setDoubleValue: value];
                    if i == 5 {
                        let color: Obj = if value <= 10. {
                            msg_send![class!(NSColor), systemRedColor]
                        } else if value <= 20. {
                            msg_send![class!(NSColor), systemOrangeColor]
                        } else {
                            msg_send![class!(NSColor), systemGreenColor]
                        };
                        let _: () = msg_send![row.gauge, setFillColor: color];
                    }
                }
            }
            let footer = if self.config.any_enabled() {
                format!(
                    "Cada {} s · lectura {:.2} ms · Apple Silicon",
                    s.interval.max(self.config.interval),
                    s.sample_us as f64 / 1000.
                )
            } else {
                "Todos los indicadores están en pausa.".into()
            };
            string(footer_view, sel!(setStringValue:), &footer);
        }
    }
    unsafe fn observe(&self) {
        unsafe {
            let workspace: Obj = msg_send![class!(NSWorkspace), sharedWorkspace];
            let center: Obj = msg_send![workspace, notificationCenter];
            for (name, action) in [
                ("NSWorkspaceWillSleepNotification", sel!(sleep:)),
                ("NSWorkspaceDidWakeNotification", sel!(wake:)),
                ("NSWorkspaceScreensDidSleepNotification", sel!(screenSleep:)),
                ("NSWorkspaceScreensDidWakeNotification", sel!(screenWake:)),
            ] {
                let _: () = msg_send![center, addObserver: &*self.target, selector: action, name: &*NSString::from_str(name), object: ptr::null::<AnyObject>()];
            }
        }
    }
    fn refresh_login(&self) {
        if self.settings.is_none() {
            return;
        }
        unsafe {
            let service: Obj = msg_send![class!(SMAppService), mainAppService];
            let status: isize = msg_send![service, status];
            let _: () = msg_send![self.login, setState: if status == 1 || status == 2 { 1isize } else { 0isize }];
            if status == 2 {
                string(
                    self.settings_status,
                    sel!(setStringValue:),
                    "Autoriza AMStats en Ítems de inicio de macOS.",
                );
            }
        }
    }
    fn change(&mut self, sender: Obj) {
        unsafe {
            let tag: isize = msg_send![sender, tag];
            let state: isize = if tag != 10 {
                msg_send![sender, state]
            } else {
                0
            };
            match tag {
                0..=5 => self.config.enabled[tag as usize] = state == 1,
                10 => {
                    let selected: isize = msg_send![sender, indexOfSelectedItem];
                    self.config.interval = match selected {
                        0 => 1,
                        2 => 5,
                        _ => 2,
                    };
                }
                11 => self.config.battery_saver = state == 1,
                12 => {
                    let service: Obj = msg_send![class!(SMAppService), mainAppService];
                    let mut error: Obj = ptr::null_mut();
                    let success: bool = if state == 1 {
                        msg_send![service, registerAndReturnError: &mut error]
                    } else {
                        msg_send![service, unregisterAndReturnError: &mut error]
                    };
                    let message = if success {
                        "Preferencia de inicio actualizada.".to_string()
                    } else if !error.is_null() {
                        let description: Retained<NSString> =
                            msg_send![error, localizedDescription];
                        format!("No se pudo cambiar el inicio: {description}")
                    } else {
                        "No se pudo cambiar el inicio. Usa la app instalada en Aplicaciones.".into()
                    };
                    string(self.settings_status, sel!(setStringValue:), &message);
                    self.refresh_login();
                    return;
                }
                _ => return,
            }
            self.config.save();
            // Never show stale data from a previously disabled collector.
            *self.shared.snapshot.lock().unwrap() = Snapshot::default();
            let _ = self.worker.tx.send(Command::Configure(self.config.clone()));
            self.refresh();
            string(
                self.settings_status,
                sel!(setStringValue:),
                "Cambios guardados.",
            );
        }
    }
}
fn row_text(i: usize, s: &Snapshot) -> (String, String, Option<f64>) {
    match i {
        0 => (
            s.cpu.map(|p| format!("{p:.0}%")).unwrap_or("—".into()),
            "Uso total · todos los núcleos".into(),
            s.cpu,
        ),
        1 => s
            .memory
            .map(|m| {
                (
                    format!("{:.0}%", metrics::percent(m.used, m.total)),
                    format!(
                        "{} de {} · {} comprimidos",
                        metrics::bytes(m.used),
                        metrics::bytes(m.total),
                        metrics::bytes(m.compressed)
                    ),
                    Some(metrics::percent(m.used, m.total)),
                )
            })
            .unwrap_or(("—".into(), "Memoria no disponible".into(), None)),
        2 => (
            s.gpu.map(|p| format!("{p:.0}%")).unwrap_or("—".into()),
            if s.gpu.is_some() {
                "Uso del procesador gráfico"
            } else {
                "macOS no ha proporcionado esta métrica"
            }
            .into(),
            s.gpu,
        ),
        3 => s
            .disk
            .map(|d| {
                (
                    format!("{:.0}%", metrics::percent(d.used, d.total)),
                    format!(
                        "{} disponibles de {}",
                        metrics::bytes(d.available),
                        metrics::bytes(d.total)
                    ),
                    Some(metrics::percent(d.used, d.total)),
                )
            })
            .unwrap_or(("—".into(), "Almacenamiento no disponible".into(), None)),
        4 => s
            .network
            .map(|n| {
                (
                    format!("↓ {}  ↑ {}", metrics::speed(n.down), metrics::speed(n.up)),
                    "Wi-Fi y Ethernet · velocidad de transferencia".into(),
                    None,
                )
            })
            .unwrap_or(("—".into(), "Calculando velocidad de red…".into(), None)),
        5 => s
            .battery
            .map(|b| {
                (
                    format!("{:.0}%", b.percent),
                    metrics::battery_detail(b),
                    Some(b.percent),
                )
            })
            .unwrap_or((
                "—".into(),
                "Sin batería o sin datos disponibles".into(),
                None,
            )),
        _ => (
            "AM".into(),
            "AMStats · abrir resumen y ajustes".into(),
            None,
        ),
    }
}
extern "C-unwind" fn refresh(_: *mut AnyObject, _: Sel, _: Obj) {
    with_app(|a| a.refresh());
}
extern "C-unwind" fn change(_: *mut AnyObject, _: Sel, sender: Obj) {
    with_app(|a| a.change(sender));
}
extern "C-unwind" fn toggle(_: *mut AnyObject, _: Sel, sender: Obj) {
    with_app(|a| unsafe {
        a.toggle_detail(sender);
    });
}
extern "C-unwind" fn summary(_: *mut AnyObject, _: Sel, _: Obj) {
    with_app(|a| a.show_summary());
}
extern "C-unwind" fn settings(_: *mut AnyObject, _: Sel, _: Obj) {
    with_app(|a| a.show_settings());
}
extern "C-unwind" fn about(_: *mut AnyObject, _: Sel, _: Obj) {
    with_app(|a| a.show_about());
}
extern "C-unwind" fn check_updates(_: *mut AnyObject, _: Sel, _: Obj) {
    with_app(|a| a.check_updates());
}
extern "C-unwind" fn did_launch(_: *mut AnyObject, _: Sel, _: Obj) {
    with_app(|a| unsafe {
        if let Ok(updater) = &a.updater {
            if let Err(error) = updater.start() {
                a.updater = Err(error);
            }
        }
    });
}
extern "C-unwind" fn update_preference(_: *mut AnyObject, _: Sel, sender: Obj) {
    with_app(|a| unsafe {
        if let Ok(updater) = &a.updater {
            let tag: isize = msg_send![sender, tag];
            let state: isize = msg_send![sender, state];
            updater.set_preference(tag == 1, state == 1);
        }
        a.refresh_update_preferences();
    });
}
extern "C-unwind" fn open_link(_: *mut AnyObject, _: Sel, sender: Obj) {
    unsafe {
        let tag: isize = msg_send![sender, tag];
        let address = match tag {
            0 => "https://x.com/amartinro",
            1 => "https://www.instagram.com/amartinro/",
            _ => crate::updates::REPOSITORY,
        };
        let url: Obj = msg_send![class!(NSURL), URLWithString: &*NSString::from_str(address)];
        let workspace: Obj = msg_send![class!(NSWorkspace), sharedWorkspace];
        let _: bool = msg_send![workspace, openURL: url];
    }
}
extern "C-unwind" fn quit(_: *mut AnyObject, _: Sel, _: Obj) {
    unsafe {
        let app: Obj = msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![app, terminate: ptr::null::<AnyObject>()];
    }
}
extern "C-unwind" fn login_settings(_: *mut AnyObject, _: Sel, _: Obj) {
    unsafe {
        let _: () = msg_send![class!(SMAppService), openSystemSettingsLoginItems];
    }
}
extern "C-unwind" fn sleep(_: *mut AnyObject, _: Sel, _: Obj) {
    with_app(|a| {
        a.suspend_detail(1, true);
        let _ = a.worker.tx.send(Command::Suspend(1, true));
    });
}
extern "C-unwind" fn wake(_: *mut AnyObject, _: Sel, _: Obj) {
    with_app(|a| {
        a.suspend_detail(1, false);
        let _ = a.worker.tx.send(Command::Suspend(1, false));
    });
}
extern "C-unwind" fn screen_sleep(_: *mut AnyObject, _: Sel, _: Obj) {
    with_app(|a| {
        a.suspend_detail(2, true);
        let _ = a.worker.tx.send(Command::Suspend(2, true));
    });
}
extern "C-unwind" fn screen_wake(_: *mut AnyObject, _: Sel, _: Obj) {
    with_app(|a| {
        a.suspend_detail(2, false);
        let _ = a.worker.tx.send(Command::Suspend(2, false));
    });
}
extern "C" fn power_changed(_: *mut std::ffi::c_void) {
    with_app(|a| {
        let _ = a.worker.tx.send(Command::PowerChanged);
    });
}
extern "C-unwind" fn reopen(_: *mut AnyObject, _: Sel, _: Obj, visible: Bool) -> Bool {
    with_app(|a| {
        if !visible.as_bool() && !a.popover_shown() && !a.detail_visible() {
            a.show_settings();
        }
    });
    Bool::YES
}

unsafe fn make_menu(app: &AnyObject, target: &AnyObject) {
    unsafe {
        let menu: Retained<AnyObject> = msg_send![class!(NSMenu), new];
        let item: Retained<AnyObject> = msg_send![class!(NSMenuItem), new];
        let submenu: Retained<AnyObject> = msg_send![class!(NSMenu), new];
        for (title, selector, key) in [
            ("Acerca de AMStats…", sel!(about:), ""),
            ("Buscar actualizaciones…", sel!(checkUpdates:), "u"),
            ("Resumen", sel!(summary:), "r"),
            ("Ajustes…", sel!(settings:), ","),
            ("Salir de AMStats", sel!(quit:), "q"),
        ] {
            let command: Allocated<AnyObject> = msg_send![class!(NSMenuItem), alloc];
            let command: Retained<AnyObject> = msg_send![command, initWithTitle: &*NSString::from_str(title), action: selector, keyEquivalent: &*NSString::from_str(key)];
            let _: () = msg_send![&*command, setTarget: target];
            let _: () = msg_send![&*submenu, addItem: &*command];
        }
        let separator: Obj = msg_send![class!(NSMenuItem), separatorItem];
        let _: () = msg_send![&*submenu, addItem: separator];
        for (i, title) in NAMES.iter().enumerate() {
            let command: Allocated<AnyObject> = msg_send![class!(NSMenuItem), alloc];
            let command: Retained<AnyObject> = msg_send![command, initWithTitle: &*NSString::from_str(&format!("Detalle de {title}…")), action: sel!(detailTab:), keyEquivalent: &*NSString::from_str(&(i+1).to_string())];
            let _: () = msg_send![&*command, setTarget: target];
            let _: () = msg_send![&*command, setTag: i as isize];
            let _: () = msg_send![&*submenu, addItem: &*command];
        }
        let _: () = msg_send![&*item, setSubmenu: &*submenu];
        let _: () = msg_send![&*menu, addItem: &*item];
        let _: () = msg_send![app, setMainMenu: &*menu];
    }
}

pub fn run() {
    let app: Retained<AnyObject> = autoreleasepool(|_| unsafe {
        let app: Retained<AnyObject> = msg_send![class!(NSApplication), sharedApplication];
        let _: bool = msg_send![&*app, setActivationPolicy: 1isize];
        let mut strip_class = ClassBuilder::new(c"AMStatsStrip", class!(NSView)).unwrap();
        strip_class.add_method(sel!(drawRect:), draw_strip as extern "C-unwind" fn(_, _, _));
        strip_class.add_method(
            sel!(hitTest:),
            strip_hit_test as extern "C-unwind" fn(_, _, _) -> Obj,
        );
        strip_class.register();
        detail::register_views();
        let mut builder = ClassBuilder::new(c"AMStatsController", class!(NSObject)).unwrap();
        for (selector, method) in [
            (
                sel!(refresh:),
                refresh as extern "C-unwind" fn(*mut AnyObject, Sel, Obj),
            ),
            (sel!(toggle:), toggle),
            (sel!(summary:), summary),
            (sel!(settings:), settings),
            (sel!(about:), about),
            (sel!(checkUpdates:), check_updates),
            (sel!(openLink:), open_link),
            (sel!(updatePreference:), update_preference),
            (sel!(applicationDidFinishLaunching:), did_launch),
            (sel!(change:), change),
            (sel!(quit:), quit),
            (sel!(loginSettings:), login_settings),
            (sel!(sleep:), sleep),
            (sel!(wake:), wake),
            (sel!(screenSleep:), screen_sleep),
            (sel!(screenWake:), screen_wake),
        ] {
            builder.add_method(selector, method);
        }
        builder.add_method(
            sel!(applicationShouldHandleReopen:hasVisibleWindows:),
            reopen as extern "C-unwind" fn(_, _, _, _) -> Bool,
        );
        detail::register_actions(&mut builder);
        let target: Retained<AnyObject> = msg_send![builder.register(), new];
        let _: () = msg_send![&*app, setDelegate: &*target];
        make_menu(&app, &target);
        APP.with(|cell| *cell.borrow_mut() = Some(App::new(target)));
        app
    });
    autoreleasepool(|_| unsafe {
        let _: () = msg_send![&*app, run];
    });
    APP.with(|cell| {
        cell.borrow_mut().take();
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compact_layout_fits_worst_case_values() {
        autoreleasepool(|_| unsafe {
            let config = Config::default();
            let snapshot = Snapshot {
                cpu: Some(100.),
                gpu: Some(100.),
                memory: Some(crate::platform::Memory {
                    used: 1,
                    total: 1,
                    compressed: 0,
                }),
                disk: Some(crate::platform::Disk {
                    used: 1,
                    total: 1,
                    available: 0,
                }),
                network: Some(metrics::Rate {
                    down: 999_900_000.,
                    up: 999_900_000_000.,
                }),
                battery: Some(crate::platform::Battery {
                    percent: 100.,
                    minutes: Some(5999),
                    ..Default::default()
                }),
                ..Default::default()
            };
            let cells = bar_cells(&config, &snapshot);
            assert_eq!(strip_width(&cells), 212.);
            assert_eq!(cells.iter().map(indicator_width).sum::<f64>(), 206.);
            for cell in cells {
                for (text, font_size) in [
                    (&cell.top, if cell.network { 9. } else { 7. }),
                    (&cell.bottom, if cell.network { 9. } else { 10.5 }),
                ] {
                    let attributes = text_style(font_size, 0., false);
                    let string = NSString::from_str(text);
                    let mut size: NSSize = msg_send![&*string, sizeWithAttributes: &*attributes];
                    if let Some(number) = text.strip_suffix('%') {
                        let n = NSString::from_str(number);
                        let p = NSString::from_str("%");
                        let nsize: NSSize = msg_send![&*n, sizeWithAttributes: &*attributes];
                        let pstyle = text_style(6.5, 0., false);
                        let psize: NSSize = msg_send![&*p, sizeWithAttributes: &*pstyle];
                        size.width = nsize.width + psize.width;
                    }
                    assert!(
                        size.width <= cell.width,
                        "{text}: {} > {}",
                        size.width,
                        cell.width
                    );
                }
            }
        });
    }
    #[test]
    fn compact_layout_shrinks_and_keeps_settings_reachable() {
        let mut config = Config::default();
        let full = strip_width(&bar_cells(&config, &Snapshot::default()));
        config.enabled = [true, false, false, false, false, false];
        assert!(strip_width(&bar_cells(&config, &Snapshot::default())) < full);
        config.enabled = [false; 6];
        assert_eq!(bar_cells(&config, &Snapshot::default())[0].top, "AM");
    }
}
