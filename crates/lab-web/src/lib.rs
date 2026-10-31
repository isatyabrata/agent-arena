//! WASM front end. No wasm-bindgen: a handful of C-ABI exports and ~60 lines of JS glue.
//! The Rust side owns the world, the demo feed (when standalone) and the renderer.

pub mod font;
pub mod render;

use lab_core::{Config, DemoFeed, Event, World};
use render::{Fb, View};
use std::cell::RefCell;

struct App {
    world: World,
    demo: Option<DemoFeed>,
    fb: Fb,
    selected: Option<String>,
    hovered: Option<String>,
    t: f32,
    live: bool,
    connected: bool,
    panel: String,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

fn with<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    APP.with(|a| a.borrow_mut().as_mut().map(f))
}

/// standalone = 1 runs the built-in demo feed in the browser (no server needed).
#[no_mangle]
pub extern "C" fn lab_init(standalone: u32, seed: u32) {
    let mut world = World::new();
    world.demo = standalone == 1;
    let app = App {
        world,
        demo: (standalone == 1).then(|| DemoFeed::new(seed as u64 + 1)),
        fb: Fb::default(),
        selected: None,
        hovered: None,
        t: 0.0,
        live: standalone == 0,
        connected: false,
        panel: String::new(),
    };
    APP.with(|a| *a.borrow_mut() = Some(app));
}

#[no_mangle]
pub extern "C" fn lab_alloc(len: usize) -> *mut u8 {
    let mut v = Vec::<u8>::with_capacity(len);
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

/// # Safety
/// `ptr` must come from `lab_alloc(len)`.
#[no_mangle]
pub unsafe extern "C" fn lab_free(ptr: *mut u8, len: usize) {
    drop(Vec::from_raw_parts(ptr, 0, len));
}

/// Feed one JSON message: an Event, or {"snapshot": World} from the server.
/// Returns 1 on success, 0 if the JSON was not understood.
///
/// # Safety
/// `ptr..ptr+len` must be a valid UTF-8 buffer.
#[no_mangle]
pub unsafe extern "C" fn lab_ingest(ptr: *const u8, len: usize) -> u32 {
    let bytes = std::slice::from_raw_parts(ptr, len);
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(bytes) else { return 0 };
    with(|app| {
        app.connected = true;
        if let Some(snap) = v.get("snapshot") {
            if let Ok(w) = serde_json::from_value::<World>(snap.clone()) {
                app.world = w;
                return 1;
            }
            return 0;
        }
        if let Some(cfg) = v.get("config") {
            if let Ok(c) = serde_json::from_value::<Config>(cfg.clone()) {
                app.world.set_config(c);
                return 1;
            }
            return 0;
        }
        if let Some(ext) = v.get("external").and_then(|x| x.as_str()) {
            let id = ext.to_string();
            if let Some(a) = app.world.agents.iter_mut().find(|a| a.id == id) {
                a.external = true;
                a.online = true;
            }
            return 1;
        }
        match serde_json::from_value::<Event>(v) {
            Ok(e) => {
                app.world.apply(&e);
                1
            }
            Err(_) => 0,
        }
    })
    .unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn lab_set_connected(c: u32) {
    with(|app| app.connected = c == 1);
}

/// Advance by dt milliseconds, render, and return a pointer to RGBA pixels.
#[no_mangle]
pub extern "C" fn lab_frame(dt_ms: f32) -> *const u8 {
    with(|app| {
        let dt = (dt_ms / 1000.0).clamp(0.0, 0.1);
        app.t += dt;
        if let Some(d) = app.demo.as_mut() {
            for e in d.step(&app.world, dt) {
                app.world.apply(&e);
            }
        }
        app.world.tick(dt);
        let view = View {
            world: &app.world,
            t: app.t,
            selected: app.selected.as_deref(),
            hovered: app.hovered.as_deref(),
            live: app.live,
            connected: app.connected,
        };
        render::draw(&mut app.fb, &view);
        app.fb.px.as_ptr() as *const u8
    })
    .unwrap_or(std::ptr::null())
}

#[no_mangle]
pub extern "C" fn lab_width() -> u32 {
    with(|app| render::fb_size(&app.world).0 as u32).unwrap_or(640)
}
#[no_mangle]
pub extern "C" fn lab_height() -> u32 {
    with(|app| render::fb_size(&app.world).1 as u32).unwrap_or(420)
}

/// Build the side-panel JSON (roster, selected agent, arena). Read it with
/// lab_panel_ptr / lab_panel_len right after calling this.
#[no_mangle]
pub extern "C" fn lab_panel() -> u32 {
    with(|app| {
        app.panel = app.world.panel_json(app.selected.as_deref());
        app.panel.len() as u32
    })
    .unwrap_or(0)
}
#[no_mangle]
pub extern "C" fn lab_panel_ptr() -> *const u8 {
    with(|app| app.panel.as_ptr()).unwrap_or(std::ptr::null())
}

/// Select an agent by id (from the roster list). Empty id clears the selection.
///
/// # Safety
/// `ptr..ptr+len` must be valid UTF-8.
#[no_mangle]
pub unsafe extern "C" fn lab_select(ptr: *const u8, len: usize) {
    let id = std::str::from_utf8(std::slice::from_raw_parts(ptr, len)).unwrap_or("").to_string();
    with(|app| app.selected = if id.is_empty() { None } else { Some(id) });
}

#[no_mangle]
pub extern "C" fn lab_click(x: i32, y: i32) {
    with(|app| {
        let h = render::hit(&app.world, x, y);
        app.selected = if h.is_some() && h != app.selected { h } else { None };
    });
}

#[no_mangle]
pub extern "C" fn lab_hover(x: i32, y: i32) -> u32 {
    with(|app| {
        app.hovered = render::hit(&app.world, x, y);
        app.hovered.is_some() as u32
    })
    .unwrap_or(0)
}
