//! Software pixel renderer: draws the whole lab into a small RGBA framebuffer.
//! 640x420 pixels (about 1 MB), scaled up by the browser with crisp pixels.

use crate::font::glyph;
use lab_core::layout::*;
use lab_core::rng::hash;
use lab_core::*;

pub const HUD_H: i32 = 60;

pub const fn rgb(r: u32, g: u32, b: u32) -> u32 {
    0xFF00_0000 | (b << 16) | (g << 8) | r
}

const BG: u32 = rgb(18, 20, 28);
const WALL: u32 = rgb(38, 40, 54);
const WALL_HI: u32 = rgb(58, 61, 80);
const WHITE: u32 = rgb(232, 234, 242);
const DIM: u32 = rgb(140, 146, 165);
const DARK: u32 = rgb(16, 18, 24);
const GREEN: u32 = rgb(110, 220, 130);
const RED: u32 = rgb(236, 90, 90);
const YELLOW: u32 = rgb(242, 208, 90);
const CYAN: u32 = rgb(100, 205, 235);
const ORANGE: u32 = rgb(245, 150, 70);
const SCREEN_BG: u32 = rgb(10, 16, 24);

const SKINS: [u32; 5] = [rgb(255, 219, 172), rgb(241, 194, 125), rgb(224, 172, 105), rgb(198, 134, 66), rgb(141, 85, 36)];
const HAIRS: [u32; 6] = [rgb(40, 30, 25), rgb(95, 62, 30), rgb(205, 165, 85), rgb(22, 22, 26), rgb(150, 60, 40), rgb(185, 185, 195)];
pub const BLOCKS: [u32; 5] = [rgb(0, 0, 0), rgb(222, 72, 72), rgb(72, 122, 232), rgb(242, 202, 62), rgb(82, 192, 92)];
const ROBOTS: [u32; 8] = [
    rgb(205, 214, 226),
    rgb(255, 142, 62),
    rgb(140, 222, 255),
    rgb(222, 122, 222),
    rgb(162, 232, 122),
    rgb(250, 232, 122),
    rgb(182, 162, 255),
    rgb(255, 122, 122),
];

fn role_color(r: Role) -> u32 {
    match r {
        Role::Captain => rgb(232, 182, 62),
        Role::Researcher => rgb(152, 102, 212),
        Role::Scientist => rgb(238, 238, 242),
        Role::Engineer => rgb(72, 132, 222),
        Role::Tester => rgb(82, 182, 112),
        Role::Auditor => rgb(212, 72, 72),
        Role::Scribe => rgb(62, 182, 182),
        Role::Guest => rgb(150, 150, 150),
    }
}

fn floor_color(k: Option<BoxKind>) -> (u32, u32) {
    use RoomKind::*;
    match k {
        Some(BoxKind::Room(Research)) => (rgb(78, 70, 96), rgb(72, 64, 90)),
        Some(BoxKind::Room(Science)) => (rgb(70, 92, 98), rgb(64, 86, 92)),
        Some(BoxKind::Room(Engineering)) => (rgb(68, 76, 94), rgb(62, 70, 88)),
        Some(BoxKind::Room(Integrity)) => (rgb(92, 66, 70), rgb(86, 60, 64)),
        Some(BoxKind::Room(Testing)) => (rgb(66, 90, 72), rgb(60, 84, 66)),
        Some(BoxKind::Room(Command)) => (rgb(92, 84, 64), rgb(86, 78, 58)),
        Some(BoxKind::Room(Lounge)) => (rgb(84, 74, 70), rgb(78, 68, 64)),
        Some(BoxKind::Arena) => (rgb(36, 40, 58), rgb(32, 36, 52)),
        None => (rgb(98, 98, 104), rgb(92, 92, 98)),
    }
}

fn hex(c: &str) -> Option<u32> {
    parse_hex(c).map(|(r, g, b)| rgb(r as u32, g as u32, b as u32))
}

fn shade(c: u32, f: f32) -> u32 {
    let r = ((c & 0xFF) as f32 * f).min(255.0) as u32;
    let g = (((c >> 8) & 0xFF) as f32 * f).min(255.0) as u32;
    let b = (((c >> 16) & 0xFF) as f32 * f).min(255.0) as u32;
    rgb(r, g, b)
}

pub fn fit(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max.saturating_sub(1)).collect();
        t.push('.');
        t
    }
}

pub struct Fb {
    pub w: i32,
    pub h: i32,
    pub px: Vec<u32>,
}

impl Default for Fb {
    fn default() -> Self {
        Self::new(640, 420)
    }
}

impl Fb {
    pub fn new(w: i32, h: i32) -> Self {
        Fb { w, h, px: vec![BG; (w * h) as usize] }
    }
    pub fn resize(&mut self, w: i32, h: i32) {
        if w != self.w || h != self.h {
            *self = Fb::new(w, h);
        }
    }
    #[inline]
    pub fn set(&mut self, x: i32, y: i32, c: u32) {
        if x >= 0 && y >= 0 && x < self.w && y < self.h {
            self.px[(y * self.w + x) as usize] = c;
        }
    }
    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: u32) {
        let (x0, y0) = (x.max(0), y.max(0));
        let (x1, y1) = ((x + w).min(self.w), (y + h).min(self.h));
        for yy in y0..y1 {
            let row = (yy * self.w) as usize;
            for xx in x0..x1 {
                self.px[row + xx as usize] = c;
            }
        }
    }
    pub fn outline(&mut self, x: i32, y: i32, w: i32, h: i32, c: u32) {
        self.rect(x, y, w, 1, c);
        self.rect(x, y + h - 1, w, 1, c);
        self.rect(x, y, 1, h, c);
        self.rect(x + w - 1, y, 1, h, c);
    }
    pub fn line(&mut self, mut x0: i32, mut y0: i32, x1: i32, y1: i32, c: u32) {
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
        let mut err = dx + dy;
        loop {
            self.set(x0, y0, c);
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }
    pub fn text(&mut self, x: i32, y: i32, s: &str, c: u32) -> i32 {
        let mut cx = x;
        for ch in s.chars() {
            let g = glyph(ch).as_bytes();
            for i in 0..15 {
                if g[i] == b'1' {
                    self.set(cx + (i % 3) as i32, y + (i / 3) as i32, c);
                }
            }
            cx += 4;
        }
        cx
    }
}

pub fn text_w(s: &str) -> i32 {
    s.chars().count() as i32 * 4
}

pub struct View<'a> {
    pub world: &'a World,
    pub t: f32,
    pub selected: Option<&'a str>,
    pub hovered: Option<&'a str>,
    pub live: bool,
    pub connected: bool,
}

pub fn fb_size(w: &World) -> (i32, i32) {
    (w.layout.w, w.layout.h + HUD_H)
}

pub fn draw(fb: &mut Fb, v: &View) {
    let (w, h) = fb_size(v.world);
    fb.resize(w, h);
    fb.rect(0, 0, fb.w, fb.h, BG);
    draw_rooms(fb, v);
    draw_furniture(fb, v);
    draw_desks(fb, v);
    draw_arena(fb, v);
    draw_agents(fb, v);
    draw_hud(fb, v);
}

fn checker(fb: &mut Fb, x: i32, y: i32, w: i32, h: i32, c: (u32, u32)) {
    for yy in y..y + h {
        for xx in x..x + w {
            fb.set(xx, yy, if ((xx / 8) + (yy / 8)) % 2 == 0 { c.0 } else { c.1 });
        }
    }
}

fn room_floor(v: &View, r: &RoomBox) -> (u32, u32) {
    let custom = v.world.config.rooms.get(r.cfg).and_then(|c| c.floor.as_deref()).and_then(hex);
    match custom {
        Some(c) => (c, shade(c, 0.92)),
        None => floor_color(r.kind),
    }
}

fn draw_rooms(fb: &mut Fb, v: &View) {
    let l = &v.world.layout;
    checker(fb, 0, l.corr_y0, l.w, CORRIDOR_H, floor_color(None));
    for r in &l.rooms {
        let fl = room_floor(v, r);
        checker(fb, r.x, r.y, r.w, r.h, fl);
        fb.rect(r.x, r.y, r.w, 12, WALL);
        fb.rect(r.x, r.y + 11, r.w, 1, WALL_HI);
        fb.text(r.x + 4, r.y + 4, &fit(&r.name.to_uppercase(), ((r.w - 8) / 4) as usize), DIM);
        if r.top {
            fb.rect(r.x, r.y + r.h - 2, r.w, 2, WALL);
            fb.rect(r.door_x - 8, r.y + r.h - 2, 16, 2, floor_color(None).0);
        } else {
            fb.rect(r.door_x - 8, r.y, 16, 12, fl.0);
            fb.rect(r.door_x - 9, r.y, 1, 12, WALL_HI);
            fb.rect(r.door_x + 8, r.y, 1, 12, WALL_HI);
        }
        if r.x > 0 {
            fb.rect(r.x, r.y, 2, r.h, WALL);
        }
    }
}

fn draw_furniture(fb: &mut Fb, v: &View) {
    let t = v.t;
    let l = &v.world.layout;
    for r in &l.rooms {
        let cx = r.x + r.w / 2;
        let y = r.y;
        match r.kind {
            Some(BoxKind::Room(RoomKind::Research)) => {
                fb.rect(cx - 43, y + 15, 86, 16, rgb(200, 200, 205));
                fb.rect(cx - 42, y + 16, 84, 14, rgb(245, 245, 248));
                for i in 0..6 {
                    let h = hash(&format!("wb{}{}", i, (t / 20.0) as i32));
                    let c = [rgb(70, 110, 220), rgb(220, 70, 70), rgb(40, 40, 50)][(h % 3) as usize];
                    fb.rect(cx - 39 + (h % 60) as i32, y + 18 + i * 2, 6 + (h % 16) as i32, 1, c);
                }
            }
            Some(BoxKind::Room(RoomKind::Science)) => {
                fb.rect(cx - 49, y + 22, 98, 8, rgb(150, 156, 162));
                fb.rect(cx - 49, y + 30, 98, 2, rgb(90, 96, 104));
                for i in 0..7 {
                    let x = cx - 43 + i * 13;
                    let c = [rgb(110, 220, 130), rgb(240, 120, 200), rgb(110, 180, 255)][(i % 3) as usize];
                    fb.rect(x + 1, y + 14, 2, 3, rgb(200, 220, 230));
                    fb.rect(x, y + 17, 4, 5, c);
                    if ((t * 3.0) as i32 + i) % 5 == 0 {
                        fb.set(x + 1, y + 12, c);
                    }
                }
            }
            Some(BoxKind::Room(RoomKind::Engineering)) => {
                let n = ((r.w - 20) / 26).min(8);
                let x0 = cx - n * 13;
                for i in 0..n {
                    let x = x0 + i * 26 + 4;
                    fb.rect(x, y + 14, 18, 22, rgb(30, 32, 40));
                    fb.outline(x, y + 14, 18, 22, rgb(70, 74, 90));
                    for j in 0..4 {
                        for k in 0..3 {
                            let on = (hash(&format!("{}{}{}{}", i, j, k, (t * 6.0) as i32)) % 100) as f32 / 100.0 < v.world.gpu;
                            fb.set(x + 4 + k * 5, y + 18 + j * 4, if on { GREEN } else { rgb(40, 70, 50) });
                        }
                    }
                }
            }
            Some(BoxKind::Room(RoomKind::Integrity)) => {
                fb.rect(cx - 37, y + 14, 74, 22, rgb(120, 92, 70));
                fb.outline(cx - 37, y + 14, 74, 22, rgb(80, 60, 46));
                fb.text(cx - 33, y + 17, "KNOWN ARCH", WHITE);
                fb.text(cx - 33, y + 24, "REGISTRY", WHITE);
                fb.outline(cx + 21, y + 22, 10, 10, RED);
                fb.line(cx + 22, y + 31, cx + 30, y + 23, RED);
            }
            Some(BoxKind::Room(RoomKind::Testing)) => {
                fb.rect(cx - 45, y + 26, 90, 8, rgb(110, 116, 124));
                for i in 0..4 {
                    fb.rect(cx - 39 + i * 22, y + 20, 10, 6, rgb(40, 44, 54));
                    fb.set(cx - 35 + i * 22, y + 22, if ((t * 2.0) as i32 + i) % 3 == 0 { GREEN } else { YELLOW });
                }
            }
            Some(BoxKind::Room(RoomKind::Command)) => {
                fb.rect(cx - 64, y + 14, 128, 20, rgb(20, 24, 34));
                fb.outline(cx - 64, y + 14, 128, 20, rgb(80, 86, 110));
                fb.text(cx - 60, y + 17, &fit(&format!("PHASE: {}", v.world.phase), 30), YELLOW);
                let online = v.world.agents.iter().filter(|a| a.online).count();
                fb.text(cx - 60, y + 25, &format!("ONLINE {}/{}  FLAGS {}", online, v.world.agents.len(), v.world.flags), CYAN);
            }
            Some(BoxKind::Room(RoomKind::Lounge)) => {
                fb.rect(cx - 30, y + 18, 60, 12, rgb(120, 70, 70));
                fb.rect(cx - 30, y + 16, 60, 3, rgb(150, 90, 90));
            }
            _ => {}
        }
    }
    // corridor: knowledge board, coffee, plants
    let cy = l.corr_y0;
    fb.rect(180, cy + 1, 84, 11, rgb(160, 120, 80));
    fb.outline(180, cy + 1, 84, 11, rgb(110, 80, 50));
    for (i, n) in v.world.notes.iter().rev().take(13).enumerate() {
        let c = match n.kind {
            NoteKind::Works => GREEN,
            NoteKind::Fails => RED,
            NoteKind::Info => YELLOW,
        };
        fb.rect(183 + i as i32 * 6, cy + 3 + (i as i32 % 2) * 3, 4, 4, c);
    }
    let mx = l.w - 40;
    fb.rect(mx, cy + 1, 14, 13, rgb(60, 60, 66));
    fb.rect(mx + 3, cy + 4, 8, 4, rgb(30, 30, 34));
    fb.set(mx + 7, cy + 9, RED);
    let mut px = 8;
    while px < l.w - 60 {
        fb.rect(px, cy + 18, 6, 5, rgb(140, 90, 60));
        fb.rect(px - 1, cy + 11, 8, 7, rgb(60, 150, 70));
        px += 300;
    }
}

fn occupant<'a>(w: &'a World, room: usize, slot: usize) -> Option<&'a Agent> {
    w.agents.iter().find(|a| a.desk == Some((room, slot)))
}

fn draw_desks(fb: &mut Fb, v: &View) {
    for r in &v.world.layout.rooms {
        for (slot, &(dx, dy)) in r.desks.iter().enumerate() {
            let who = occupant(v.world, r.cfg, slot);
            let seated = who.is_some_and(|a| a.seated() && a.online);
            fb.rect(dx + 7, dy - 11, 20, 13, rgb(28, 28, 34));
            draw_screen(fb, dx + 8, dy - 10, 18, 10, if seated { who } else { None }, v);
            fb.rect(dx, dy, 34, 4, rgb(148, 110, 76));
            fb.rect(dx, dy + 4, 34, 8, rgb(120, 86, 58));
            fb.rect(dx + 2, dy + 12, 2, 2, rgb(80, 58, 40));
            fb.rect(dx + 30, dy + 12, 2, 2, rgb(80, 58, 40));
            fb.rect(dx + 12, dy + 1, 10, 2, rgb(60, 62, 70));
        }
    }
}

/// Draws an agent's computer screen (desk monitor size).
pub fn draw_screen(fb: &mut Fb, x: i32, y: i32, w: i32, h: i32, a: Option<&Agent>, v: &View) {
    fb.rect(x, y, w, h, SCREEN_BG);
    let Some(a) = a else {
        let bx = x + ((v.t * 0.7) as i32 % w.max(1));
        fb.set(bx, y + h / 2, rgb(40, 60, 80));
        return;
    };
    let t = v.t;
    match a.activity {
        Activity::Training => {
            if let Some(m) = v.world.models.iter().rev().find(|m| m.owner == a.id) {
                let n = m.losses.len();
                if n > 1 {
                    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
                    for l in &m.losses {
                        lo = lo.min(*l);
                        hi = hi.max(*l);
                    }
                    let span = (hi - lo).max(1e-3);
                    let mut prev = None;
                    for (i, l) in m.losses.iter().enumerate() {
                        let px = x + (i as i32 * (w - 1)) / (n as i32 - 1).max(1);
                        let py = y + 1 + ((hi - l) / span * (h - 4) as f32) as i32;
                        if let Some((ox, oy)) = prev {
                            fb.line(ox, oy, px, py, GREEN);
                        }
                        prev = Some((px, py));
                    }
                }
                fb.rect(x, y + h - 2, ((w as f32) * m.progress) as i32, 2, YELLOW);
            }
        }
        Activity::Testing => {
            let ar = &v.world.arena;
            let n = ar.robots.len().max(1) as i32;
            let bh = ((h - 2) / n).max(1);
            for (i, r) in ar.robots.iter().enumerate() {
                let p = ar.progress(i);
                fb.rect(x + 1, y + 1 + i as i32 * bh, ((w - 2) as f32 * p) as i32 + 1, (bh - 1).max(1), ROBOTS[r.color as usize % 8]);
            }
        }
        Activity::Auditing => {
            for i in 0..(h / 2) {
                let ok = hash(&format!("{}{}", a.id, i)) % 4 != 0;
                fb.set(x + 1, y + 1 + i * 2, if ok { GREEN } else { RED });
                fb.rect(x + 3, y + 1 + i * 2, w / 2, 1, rgb(90, 100, 120));
            }
        }
        Activity::Grading => {
            for i in 0..(w / 3) {
                let bh = 2 + (hash(&format!("{}g{}", a.id, i)) % (h as u32 - 3)) as i32;
                fb.rect(x + 1 + i * 3, y + h - bh, 2, bh, CYAN);
            }
        }
        Activity::Reading => {
            fb.rect(x + 2, y + 1, w - 4, h - 2, rgb(225, 225, 230));
            for i in 0..((h - 3) / 2) {
                let l = (hash(&format!("{}r{}", a.id, i)) % (w as u32 - 8)) as i32 + 3;
                fb.rect(x + 4, y + 2 + i * 2, l, 1, rgb(130, 130, 140));
            }
        }
        Activity::Idle | Activity::Waiting => {
            if (t * 2.0) as i32 % 2 == 0 {
                fb.rect(x + 2, y + 2, 1, 2, WHITE);
            }
            if a.activity == Activity::Waiting {
                fb.text(x + w / 2 - 2, y + h / 2 - 2, "?", YELLOW);
            }
        }
        _ => {
            // real terminal lines when the agent streams output, else synthetic code
            let pal = [rgb(200, 120, 230), rgb(110, 200, 240), rgb(230, 200, 110), rgb(150, 220, 140), rgb(170, 175, 190)];
            let rows = h / 2;
            if !a.terminal.is_empty() {
                let start = a.terminal.len().saturating_sub(rows as usize);
                for (i, line) in a.terminal.iter().skip(start).enumerate() {
                    let len = (line.trim_end().chars().count() as i32 / 3).clamp(1, w - 2);
                    let indent = (line.chars().take_while(|c| *c == ' ').count() as i32 / 2).min(6);
                    fb.rect(x + 1 + indent, y + 1 + i as i32 * 2, (len - indent).max(1), 1, pal[(hash(line) % 5) as usize]);
                }
            } else {
                let speed = if a.activity == Activity::Typing { 3.0 } else { 0.8 };
                let scroll = (t * speed) as u32;
                for i in 0..rows {
                    let hh = hash(&format!("{}{}", a.look, scroll + i as u32));
                    let indent = ((hh >> 8) % 3) as i32 * 2;
                    let len = 2 + (hh % (w as u32 - 4)) as i32;
                    fb.rect(x + 1 + indent, y + 1 + i * 2, (len - indent).max(2), 1, pal[(hh % 5) as usize]);
                }
            }
        }
    }
}

fn draw_agent(fb: &mut Fb, a: &Agent, v: &View) {
    let l = a.x as i32 - 4;
    let top = a.y as i32 - 14;
    let dim = if a.online { 1.0 } else { 0.45 };
    let skin = shade(SKINS[(a.look % 5) as usize], dim);
    let hair = shade(HAIRS[((a.look / 5) % 6) as usize], dim);
    let base = a.color.map(|(r, g, b)| rgb(r as u32, g as u32, b as u32)).unwrap_or(role_color(a.role));
    let shirt = shade(base, dim);
    let seated = a.seated();
    let walking = !a.path.is_empty();
    let front = if seated { false } else if walking { a.facing_down } else { true };
    if v.selected == Some(a.id.as_str()) {
        let pulse = ((v.t * 4.0).sin() * 1.5) as i32;
        fb.outline(l - 3 - pulse, top - 3 - pulse, 14 + pulse * 2, 21 + pulse * 2, CYAN);
    }
    fb.rect(l + 1, top + 14, 6, 1, rgb(20, 20, 26));
    fb.rect(l + 1, top, 6, 2, hair);
    fb.rect(l + 1, top + 2, 6, 4, skin);
    if front {
        fb.set(l + 2, top + 3, DARK);
        fb.set(l + 5, top + 3, DARK);
    } else {
        fb.rect(l + 1, top + 2, 6, 3, hair);
    }
    fb.rect(l + 1, top + 6, 6, 5, shirt);
    if a.role == Role::Scientist {
        fb.rect(l + 3, top + 6, 2, 3, shade(rgb(80, 140, 200), dim));
    }
    if a.role == Role::Captain {
        fb.set(l + 2, top + 7, YELLOW);
    }
    let arm = shade(shirt, 0.75);
    let typing = seated && a.online && a.activity == Activity::Typing;
    let lift = if typing && ((v.t * 10.0) as i32 + a.look as i32) % 2 == 0 { 1 } else { 0 };
    let lift2 = if typing { 1 - lift } else { 0 };
    fb.rect(l, top + 6 - lift, 1, 4, arm);
    fb.rect(l + 7, top + 6 - lift2, 1, 4, arm);
    fb.set(l, top + 10 - lift, skin);
    fb.set(l + 7, top + 10 - lift2, skin);
    if seated {
        fb.rect(l, top + 10, 8, 4, rgb(54, 56, 66));
        fb.rect(l + 1, top + 10, 6, 1, rgb(74, 76, 88));
    } else {
        let pants = shade(rgb(50, 55, 75), dim);
        let ph = (a.walk as i32) % 2;
        fb.rect(l + 2, top + 11, 2, 3 - ph, pants);
        fb.rect(l + 4, top + 11, 2, 2 + ph, pants);
    }
    if a.external {
        fb.rect(a.x as i32, top - 3, 1, 2, CYAN);
    }
    if !a.online && seated {
        let z = ((v.t * 1.5) as i32 % 3) as i32;
        fb.text(l + 7, top - 4 - z, "Z", DIM);
    }
}

fn bubble(fb: &mut Fb, cx: i32, by: i32, text: &str, fg: u32, bg: u32) {
    let s = fit(text, 34);
    let w = text_w(&s) + 3;
    let x = (cx - w / 2).clamp(1, (fb.w - w - 1).max(1));
    let y = (by - 9).max(1);
    fb.rect(x, y, w, 9, bg);
    fb.outline(x, y, w, 9, shade(bg, 0.6));
    fb.set(cx, y + 9, bg);
    fb.text(x + 2, y + 2, &s, fg);
}

fn draw_agents(fb: &mut Fb, v: &View) {
    let mut order: Vec<&Agent> = v.world.agents.iter().collect();
    order.sort_by(|a, b| a.y.partial_cmp(&b.y).unwrap());
    for a in &order {
        draw_agent(fb, a, v);
    }
    for a in &order {
        let top = a.y as i32 - 14;
        if let Some(b) = &a.bubble {
            bubble(fb, a.x as i32, top - 1, b, DARK, WHITE);
        } else if v.hovered == Some(a.id.as_str()) || v.selected == Some(a.id.as_str()) {
            bubble(fb, a.x as i32, top - 1, &a.name, WHITE, rgb(30, 34, 48));
        }
    }
}

fn part_color(ar: &Arena, p: u8) -> u32 {
    ar.parts
        .get(p as usize - 1)
        .and_then(|x| hex(&x.color))
        .unwrap_or(BLOCKS[(p as usize - 1) % 4 + 1])
}

fn draw_part(fb: &mut Fb, ar: &Arena, p: u8, x: i32, y: i32, s: i32) {
    let c = part_color(ar, p);
    fb.rect(x, y, s, s, c);
    fb.rect(x, y, s, 1, shade(c, 1.3));
    if let Some(g) = ar.parts.get(p as usize - 1).and_then(|x| x.glyph) {
        if s >= 6 {
            let lum = (c & 0xFF) + ((c >> 8) & 0xFF) + ((c >> 16) & 0xFF);
            fb.text(x + (s - 3) / 2, y + (s - 5) / 2, &g.to_string(), if lum > 380 { DARK } else { WHITE });
        }
    }
}

fn draw_arena(fb: &mut Fb, v: &View) {
    let l = &v.world.layout;
    let Some(room) = l.arena() else { return };
    let ar = &v.world.arena;
    let title = if ar.robots.is_empty() {
        format!("{} - IDLE", room.name)
    } else if ar.running {
        format!("{} - {} - MATCH {} - T+{:.1}S", room.name, ar.title, ar.matches, ar.elapsed)
    } else {
        format!("{} - {} - MATCH {} - FINAL", room.name, ar.title, ar.matches)
    };
    fb.rect(room.x + 2, room.y + 1, room.w - 4, 10, WALL);
    fb.text(room.x + 6, room.y + 4, &fit(&title.to_uppercase(), ((room.w - 12) / 4) as usize), if ar.running { YELLOW } else { DIM });
    if ar.robots.is_empty() {
        fb.text(room.x + room.w / 2 - 46, room.y + room.h / 2, "WAITING FOR ARENA_START", DIM);
        return;
    }
    let np = ar.parts.len().max(1);
    let bw = bin_w(np);
    for i in 0..l.lanes {
        let (lx, ly) = l.lane_origin(i);
        fb.outline(lx, ly, LANE_W, LANE_H, rgb(60, 66, 92));
        let Some(r) = ar.robots.get(i) else { continue };
        let rc = ROBOTS[r.color as usize % 8];
        fb.text(lx + 2, ly + 3, &fit(&r.name.to_uppercase(), 15), rc);
        fb.rect(lx + 2, ly + 11, 60, 3, rgb(30, 34, 48));
        fb.rect(lx + 2, ly + 11, (60.0 * ar.progress(i)) as i32, 3, GREEN);
        let (gx, gy) = (lx + GRID_X, ly + GRID_Y);
        if !ar.has_target() {
            fb.outline(gx - 1, gy - 1, ar.cols as i32 * CELL + 2, ar.rows as i32 * CELL + 2, rgb(50, 56, 80));
        }
        for row in 0..ar.rows {
            for col in 0..ar.cols {
                let k = row * ar.cols + col;
                let (cx, cy) = (gx + col as i32 * CELL, gy + row as i32 * CELL);
                let want = ar.blueprint.get(k).copied().unwrap_or(0);
                let have = r.grid.get(k).copied().unwrap_or(0);
                if want != 0 {
                    fb.outline(cx, cy, CELL, CELL, shade(part_color(ar, want), 0.45));
                }
                if have != 0 {
                    draw_part(fb, ar, have, cx, cy, CELL - 1);
                    if ar.has_target() && have != want && (v.t * 4.0) as i32 % 2 == 0 {
                        fb.line(cx, cy, cx + 6, cy + 6, RED);
                        fb.line(cx + 6, cy, cx, cy + 6, RED);
                    }
                }
            }
        }
        // intent line under the grid
        let gy_end = gy + ar.rows as i32 * CELL + 3;
        fb.text(lx + 2, gy_end, &fit(&r.intent.to_uppercase(), 15), rgb(170, 180, 200));
        // part bins
        for p in 0..np {
            let px = lx + 4 + p as i32 * bw;
            let s = (bw - 1).min(7);
            for st in 0..2 {
                draw_part(fb, ar, p as u8 + 1, px, ly + PILE_Y as i32 - st * (s + 1), s);
            }
        }
        // robot
        let bob = ((v.t * 7.0 + i as f32).sin() * 1.2) as i32;
        let (rx, ry) = (lx + r.x as i32, ly + r.y as i32 + bob);
        fb.rect(rx - 4, ry - 4, 8, 6, rc);
        fb.rect(rx - 4, ry - 4, 8, 1, shade(rc, 1.25));
        fb.rect(rx - 3, ry - 2, 6, 2, DARK);
        fb.set(rx - 2, ry - 2, CYAN);
        fb.set(rx + 1, ry - 2, CYAN);
        fb.set(rx, ry - 6, RED);
        fb.set(rx, ry - 5, rgb(120, 120, 130));
        fb.rect(rx - 3, ry + 2, 2, 1, DARK);
        fb.rect(rx + 1, ry + 2, 2, 1, DARK);
        if let Some(c) = r.carrying {
            draw_part(fb, ar, c, rx - 3, ry - 12, 6);
        }
        if let Some(th) = &r.thought {
            bubble(fb, rx, ry - 13, th, DARK, rgb(200, 240, 255));
        }
        if !ar.running {
            if let Some(pos) = ar.ranking.iter().position(|&x| x == i) {
                let label = match pos {
                    0 => "WINNER".to_string(),
                    1 => "2ND".into(),
                    2 => "3RD".into(),
                    n => format!("{}TH", n + 1),
                };
                let c = if pos == 0 { YELLOW } else { DIM };
                fb.text(lx + 2, ly + LANE_H - 8, &label, c);
                if pos == 0 {
                    fb.outline(lx - 1, ly - 1, LANE_W + 2, LANE_H + 2, YELLOW);
                    fb.rect(rx - 3, ry - 9, 7, 2, YELLOW);
                    fb.set(rx - 3, ry - 10, YELLOW);
                    fb.set(rx, ry - 10, YELLOW);
                    fb.set(rx + 3, ry - 10, YELLOW);
                }
            }
        }
        if let Some(sc) = r.score {
            let s = format!("{:.1}", sc);
            fb.text(lx + LANE_W - 2 - text_w(&s), ly + LANE_H - 8, &s, CYAN);
        }
    }
}

fn draw_hud(fb: &mut Fb, v: &View) {
    let w = v.world;
    let hy = w.layout.h;
    fb.rect(0, hy, fb.w, HUD_H, rgb(14, 16, 22));
    fb.rect(0, hy, fb.w, 1, WALL_HI);
    let col = (fb.w / 3).max(200);
    let chars = ((col - 8) / 4) as usize;
    let mut y = hy + 3;
    let title = fit(&w.config.lab.name.to_uppercase(), 24);
    let tx = fb.text(4, y, &title, YELLOW);
    fb.text(tx + 6, y, &fit(&w.phase, 24), WHITE);
    y += 7;
    let online = w.agents.iter().filter(|a| a.online).count();
    let live = w.agents.iter().filter(|a| a.external).count();
    fb.text(4, y, &format!("AGENTS {}  ONLINE {}  LIVE {}  MODELS {}", w.agents.len(), online, live, w.models.len()), DIM);
    y += 7;
    let (msg, c) = if w.demo {
        ("DEMO FEED ON - SAMPLE DATA, NOT REAL MODELS", ORANGE)
    } else if v.live && !v.connected {
        ("SERVER OFFLINE - RECONNECTING", RED)
    } else {
        ("LIVE - WAITING FOR HARNESS EVENTS", GREEN)
    };
    fb.text(4, y, msg, c);
    y += 8;
    for m in w.models.iter().rev().take(4) {
        let s = format!("{} {:?} {:.3}", fit(&m.name, 16), m.stage, m.loss).to_uppercase();
        fb.text(4, y, &fit(&s, chars.saturating_sub(16)), CYAN);
        fb.rect(col - 62, y + 1, 56, 3, rgb(30, 34, 48));
        fb.rect(col - 62, y + 1, (56.0 * m.progress) as i32, 3, YELLOW);
        y += 7;
    }
    let x2 = col + 4;
    fb.rect(col, hy + 2, 1, HUD_H - 4, WALL_HI);
    fb.text(x2, hy + 3, "KNOWLEDGE BOARD", YELLOW);
    for (i, n) in w.notes.iter().rev().take(7).enumerate() {
        let (p, c) = match n.kind {
            NoteKind::Works => ("+", GREEN),
            NoteKind::Fails => ("-", RED),
            NoteKind::Info => ("*", CYAN),
        };
        fb.text(x2, hy + 10 + i as i32 * 7, &format!("{} {}", p, fit(&n.text, chars - 3)), c);
    }
    let x3 = col * 2 + 4;
    fb.rect(col * 2, hy + 2, 1, HUD_H - 4, WALL_HI);
    fb.text(x3, hy + 3, "LIVE LOG", YELLOW);
    let chars3 = ((fb.w - x3 - 4) / 4).max(10) as usize;
    for (i, l) in w.log.iter().rev().take(7).enumerate() {
        fb.text(x3, hy + 10 + i as i32 * 7, &fit(&l.text, chars3), if l.alert { YELLOW } else { DIM });
    }
}

/// Agent under a framebuffer point.
pub fn hit(w: &World, x: i32, y: i32) -> Option<String> {
    let mut best: Option<&Agent> = None;
    for a in &w.agents {
        let (ax, ay) = (a.x as i32, a.y as i32);
        if x >= ax - 6 && x <= ax + 6 && y >= ay - 17 && y <= ay + 2 && best.is_none_or(|b| a.y > b.y) {
            best = Some(a);
        }
    }
    best.map(|a| a.id.clone())
}
