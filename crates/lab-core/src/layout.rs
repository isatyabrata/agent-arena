//! Floor plan, computed from the config and from how many people each room holds.
//! Rooms grow (more desk rows) when more agents arrive, and the world grows with them.

use crate::config::{Config, RoomKind, Row};
use serde::{Deserialize, Serialize};

/// `RoomBox::cfg` value for the arena (fits in 32-bit usize, so WASM can read snapshots)
pub const NO_CFG: usize = 1_000_000;
pub const CORRIDOR_H: i32 = 28;
pub const DESK_W: i32 = 34;
pub const DESK_PITCH_X: i32 = 52;
pub const DESK_PITCH_Y: i32 = 48;
pub const DESK_TOP: i32 = 52;
pub const MIN_ROOM_W: i32 = 150;
pub const MIN_ROOM_H: i32 = 150;

// Arena geometry, lane-local coordinates
pub const LANE_W: i32 = 64;
pub const LANE_PITCH: i32 = 70;
pub const LANE_H: i32 = 138;
pub const LANE_TOP: i32 = 16;
pub const CELL: i32 = 8;
pub const GRID_X: i32 = 8;
pub const GRID_Y: i32 = 22;
pub const PILE_Y: f32 = 124.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoxKind {
    Room(RoomKind),
    Arena,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RoomBox {
    /// index into config.rooms (NO_CFG for the arena)
    pub cfg: usize,
    pub kind: Option<BoxKind>,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub door_x: i32,
    pub top: bool,
    pub cols: usize,
    /// desk top-left corners, by slot
    pub desks: Vec<(i32, i32)>,
}

impl RoomBox {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        let (x, y) = (x as i32, y as i32);
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
    pub fn inside_y(&self) -> f32 {
        if self.top {
            (self.y + self.h - 10) as f32
        } else {
            (self.y + 18) as f32
        }
    }
    pub fn is_arena(&self) -> bool {
        self.kind == Some(BoxKind::Arena)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Layout {
    pub w: i32,
    pub h: i32,
    pub corr_y0: i32,
    pub rooms: Vec<RoomBox>,
    pub lanes: usize,
}

pub fn seat_of(desk: (i32, i32)) -> (f32, f32) {
    (desk.0 as f32 + 17.0, desk.1 as f32 + 22.0)
}

impl Layout {
    /// `demand[i]` = desks needed in config room i.
    pub fn compute(cfg: &Config, demand: &[usize]) -> Layout {
        let cols_max = cfg.limits.desk_columns.clamp(1, 8);
        let lanes = cfg.arena.lanes.clamp(1, 8);
        let mut top = vec![];
        let mut bottom = vec![];
        for (i, r) in cfg.rooms.iter().enumerate() {
            let cap = r.desks.max(demand.get(i).copied().unwrap_or(0)).max(1);
            let cols = cols_max.min(cap);
            let rows = cap.div_ceil(cols);
            let w = MIN_ROOM_W.max(10 + cols as i32 * DESK_PITCH_X + 4);
            let h = MIN_ROOM_H.max(DESK_TOP + rows as i32 * DESK_PITCH_Y + 4);
            let b = RoomBox {
                cfg: i,
                kind: Some(BoxKind::Room(r.kind)),
                name: r.name.clone(),
                w,
                h,
                top: r.row == Row::Top,
                cols,
                desks: (0..cap).map(|s| ((s % cols) as i32, (s / cols) as i32)).collect(),
                ..Default::default()
            };
            if b.top {
                top.push(b)
            } else {
                bottom.push(b)
            }
        }
        let arena = RoomBox {
            cfg: NO_CFG,
            kind: Some(BoxKind::Arena),
            name: cfg.arena.name.clone(),
            w: 300.max(lanes as i32 * LANE_PITCH + 18),
            h: LANE_TOP + LANE_H + 28,
            top: false,
            cols: 0,
            ..Default::default()
        };
        let slot = cfg.arena.slot.min(bottom.len());
        bottom.insert(slot, arena);

        let width = |v: &Vec<RoomBox>| v.iter().map(|r| r.w).sum::<i32>();
        let w = 640.max(width(&top)).max(width(&bottom));
        let top_h = top.iter().map(|r| r.h).max().unwrap_or(0);
        let bottom_h = bottom.iter().map(|r| r.h).max().unwrap_or(0);
        let corr_y0 = top_h;
        let by = top_h + CORRIDOR_H;
        let mut rooms = vec![];
        for (row, y, h) in [(top, 0, top_h), (bottom, by, bottom_h)] {
            let n = row.len();
            let mut x = 0;
            for (k, mut r) in row.into_iter().enumerate() {
                r.x = x;
                r.y = y;
                r.h = h;
                if k + 1 == n {
                    r.w = w - x;
                }
                x += r.w;
                r.door_x = if r.is_arena() { r.x + r.w / 2 } else if r.cols >= 2 { r.x + 53 } else { r.x + r.w - 24 };
                let (rx, ry) = (r.x, r.y);
                for d in r.desks.iter_mut() {
                    *d = (rx + 10 + d.0 * DESK_PITCH_X, ry + DESK_TOP + d.1 * DESK_PITCH_Y);
                }
                rooms.push(r);
            }
        }
        Layout { w, h: by + bottom_h, corr_y0, rooms, lanes }
    }

    pub fn corr_mid(&self) -> f32 {
        (self.corr_y0 + CORRIDOR_H / 2) as f32
    }

    pub fn room_at(&self, x: f32, y: f32) -> Option<usize> {
        self.rooms.iter().position(|r| r.contains(x, y))
    }

    pub fn room_for_cfg(&self, cfg: usize) -> Option<&RoomBox> {
        self.rooms.iter().find(|r| r.cfg == cfg)
    }

    pub fn first_of(&self, kind: BoxKind) -> Option<&RoomBox> {
        self.rooms.iter().find(|r| r.kind == Some(kind))
    }

    pub fn arena(&self) -> Option<&RoomBox> {
        self.first_of(BoxKind::Arena)
    }

    pub fn desk(&self, cfg: usize, slot: usize) -> Option<(i32, i32)> {
        self.room_for_cfg(cfg)?.desks.get(slot).copied()
    }

    /// Lane top-left in world pixels.
    pub fn lane_origin(&self, i: usize) -> (i32, i32) {
        let a = self.arena().map(|a| (a.x, a.y, a.w)).unwrap_or((0, 0, 300));
        let used = self.lanes as i32 * LANE_PITCH - (LANE_PITCH - LANE_W);
        let x0 = a.0 + (a.2 - used) / 2;
        (x0 + i as i32 * LANE_PITCH, a.1 + LANE_TOP)
    }

    /// Walkable route: rooms connect only through the corridor, via their door aisle.
    pub fn route(&self, from: (f32, f32), to: (f32, f32)) -> Vec<(f32, f32)> {
        let ra = self.room_at(from.0, from.1);
        let rb = self.room_at(to.0, to.1);
        let mid = self.corr_mid();
        let mut p = Vec::new();
        if ra == rb {
            if let Some(r) = ra.map(|i| &self.rooms[i]) {
                if !r.is_arena() && (from.1 - to.1).abs() > 2.0 {
                    let d = r.door_x as f32;
                    p.push((d, from.1));
                    p.push((d, to.1));
                }
            }
        } else {
            if let Some(r) = ra.map(|i| &self.rooms[i]) {
                let d = r.door_x as f32;
                p.push((d, from.1));
                p.push((d, r.inside_y()));
                p.push((d, mid));
            }
            if let Some(r) = rb.map(|i| &self.rooms[i]) {
                let d = r.door_x as f32;
                p.push((d, mid));
                p.push((d, r.inside_y()));
                p.push((d, to.1));
            }
        }
        p.push(to);
        p
    }
}

/// Width of one part bin at the bottom of a lane.
pub fn bin_w(nparts: usize) -> i32 {
    (LANE_W - 8) / nparts.clamp(4, 9) as i32
}

/// Robot stand point for a grid cell, lane-local (row -1 = part bins, col = part-1).
pub fn cell_point(col: i32, row: i32, nparts: usize) -> (f32, f32) {
    if row < 0 {
        let bw = bin_w(nparts);
        ((4 + col * bw + bw / 2) as f32, PILE_Y - 6.0)
    } else {
        ((GRID_X + col * CELL + CELL / 2) as f32, (GRID_Y + row * CELL + CELL / 2) as f32)
    }
}
