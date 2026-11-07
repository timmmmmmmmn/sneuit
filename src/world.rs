//! De sneeuwwereld: vallende vlokken en klonten, hoopjes op vensterranden en op de grond (Dock).
//! Alle coördinaten zijn globaal in CoreGraphics-ruimte: oorsprong linksboven van het hoofdscherm, y naar beneden.

use std::collections::{HashMap, HashSet, VecDeque};
use std::f64::consts::PI;

use crate::actors::Actors;
use crate::wins::WinInfo;

/// Zwaartekracht in punten/s².
pub const G: f64 = 900.0;
/// Straal van de afgeronde vensterhoeken; sneeuw glijdt daar vanzelf vanaf.
pub const CORNER: f32 = 14.0;

/// Vlokken per punt schermbreedte per seconde.
const FLAKES_PER_PX: f64 = 0.012;
/// Hoeveel sneeuw (punt²) één vlok achterlaat. Afgesteld zodat een venster pas na
/// ongeveer een uur een hoopje van ~14 punten heeft.
const DEPOSIT: f32 = 0.32;
/// Maximale helling (hoogteverschil per punt) voordat sneeuw gaat schuiven.
const WIN_REPOSE: f32 = 1.0;
const GROUND_REPOSE: f32 = 0.75;
/// Hoeveel sneeuw de buitenste kolom van een venster vasthoudt voor het eraf valt.
const EDGE_HOLD: f32 = 1.2;
/// Boven deze gemiddelde hoogte druppelt sneeuw af en toe zachtjes van een venster.
const WIN_COMFORT: f32 = 16.0;
const WIN_MAX: f32 = 34.0;
/// Boven deze hoogte zakt sneeuw op de grond langzaam in.
const GROUND_COMFORT: f32 = 26.0;
const GROUND_MAX: f32 = 60.0;
const MAX_PARTICLES: usize = 6000;
/// Elke 5 minuten een kans van 1 op 5 op bezoek.
const EVENT_EVERY: f64 = 300.0;
const EVENT_CHANCE: f64 = 0.2;

pub fn rnd(a: f64, b: f64) -> f64 {
    a + (b - a) * fastrand::f64()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Rect { x, y, w, h }
    }
    pub fn intersects(&self, o: &Rect) -> bool {
        self.x < o.x + o.w && o.x < self.x + self.w && self.y < o.y + o.h && o.y < self.y + self.h
    }
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    Flake,
    Clump,
    Puff,
}

pub struct Particle {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub mass: f32,
    pub r: f64,
    pub kind: Kind,
    pub phase: f64,
}

impl Particle {
    pub fn clump(x: f64, y: f64, vx: f64, vy: f64, mass: f32) -> Self {
        let r = ((mass as f64) / PI).sqrt() * 1.15 + 0.45;
        Particle { x, y, vx, vy, mass, r, kind: Kind::Clump, phase: 0.0 }
    }
    pub fn puff(x: f64, y: f64, vx: f64, vy: f64, mass: f32) -> Self {
        Particle { kind: Kind::Puff, ..Particle::clump(x, y, vx, vy, mass) }
    }
}

/// Een sneeuwlaag als hoogtekaart met kolommen van 1 punt breed.
pub struct Pile {
    pub x: f64,
    /// Basislijn: bovenrand van het venster, of onderkant van het scherm voor de grond.
    pub y: f64,
    pub cells: Vec<f32>,
    /// Hoeveel lager de basis ligt door de afgeronde hoeken.
    pub bases: Vec<f32>,
    pub visible: Vec<bool>,
    pub maxh: f32,
    walls: bool,
    spill: [f32; 2],
    spill_next: [f32; 2],
    drip_acc: f32,
    drip_next: f32,
    flip: bool,
    dirty: Option<(usize, usize)>,
    /// Zichtbare verandering die meteen getekend moet worden; kleine dingen mogen even wachten.
    urgent: bool,
}

impl Pile {
    pub fn new(x: f64, y: f64, w: f64, walls: bool) -> Self {
        let n = w.round().max(1.0) as usize;
        let mut p = Pile {
            x,
            y,
            cells: vec![0.0; n],
            bases: vec![0.0; n],
            visible: vec![true; n],
            maxh: 0.0,
            walls,
            spill: [0.0; 2],
            spill_next: [rnd(0.4, 2.0) as f32, rnd(0.4, 2.0) as f32],
            drip_acc: 0.0,
            drip_next: rnd(0.5, 1.5) as f32,
            flip: false,
            dirty: None,
            urgent: false,
        };
        p.compute_bases();
        p
    }

    fn compute_bases(&mut self) {
        let n = self.cells.len();
        self.bases.resize(n, 0.0);
        let r = if self.walls { 0.0 } else { CORNER.min(n as f32 / 2.0) };
        for i in 0..n {
            let dx = (i as f32 + 0.5).min(n as f32 - i as f32 - 0.5);
            self.bases[i] = if dx < r { r - (r * r - (r - dx) * (r - dx)).max(0.0).sqrt() } else { 0.0 };
        }
    }

    pub fn w(&self) -> f64 {
        self.cells.len() as f64
    }

    pub fn col(&self, gx: f64) -> Option<usize> {
        let i = (gx - self.x).floor();
        (i >= 0.0 && (i as usize) < self.cells.len()).then_some(i as usize)
    }

    fn clamp_col(&self, gx: f64) -> usize {
        ((gx - self.x).floor().max(0.0) as usize).min(self.cells.len() - 1)
    }

    pub fn surface(&self, i: usize) -> f64 {
        self.y + self.bases[i] as f64 - self.cells[i] as f64
    }

    /// Gladgestreken oppervlak, handig voor figuurtjes die erover lopen.
    pub fn surface_at(&self, gx: f64) -> f64 {
        let c = self.clamp_col(gx);
        let lo = c.saturating_sub(3);
        let hi = (c + 4).min(self.cells.len());
        let sum: f64 = (lo..hi).map(|i| self.surface(i)).sum();
        sum / (hi - lo) as f64
    }

    fn mark(&mut self, lo: usize, hi: usize) {
        self.dirty = Some(match self.dirty {
            Some((a, b)) => (a.min(lo), b.max(hi)),
            None => (lo, hi),
        });
    }

    pub fn mark_all(&mut self) {
        self.mark(0, self.cells.len());
        self.urgent = true;
    }

    pub fn bbox(&self) -> Rect {
        Rect::new(self.x - 4.0, self.y - self.maxh as f64 - 8.0, self.w() + 8.0, self.maxh as f64 + CORNER as f64 + 12.0)
    }

    /// Het stuk dat sinds de vorige keer veranderd is, als scherm-rechthoek.
    pub fn take_dirty(&mut self, force: bool) -> Option<Rect> {
        if !(force || self.urgent) {
            return None;
        }
        self.urgent = false;
        let (lo, hi) = self.dirty.take()?;
        let b = self.bbox();
        Some(Rect::new(self.x + lo as f64 - 3.0, b.y, (hi - lo) as f64 + 6.0, b.h))
    }

    pub fn deposit(&mut self, gx: f64, mass: f32, spread: f64) {
        let n = self.cells.len();
        let c = gx - self.x;
        let r = spread.max(1.0);
        let lo = ((c - r).floor().max(0.0) as usize).min(n - 1);
        let hi = ((c + r).ceil().max(0.0) as usize).clamp(lo + 1, n);
        let weights: Vec<f64> = (lo..hi).map(|i| (1.0 - ((i as f64 + 0.5) - c).abs() / (r + 0.5)).max(0.05)).collect();
        let sum: f64 = weights.iter().sum();
        for (k, i) in (lo..hi).enumerate() {
            self.cells[i] += (mass as f64 * weights[k] / sum) as f32;
        }
        self.maxh = self.maxh.max(self.cells[lo..hi].iter().cloned().fold(0.0, f32::max));
        self.mark(lo, hi);
        self.urgent |= mass > 0.6;
    }

    /// Haalt een fractie van de sneeuw tussen gx0 en gx1 weg en geeft de hoeveelheid terug.
    pub fn take(&mut self, gx0: f64, gx1: f64, frac: f32) -> f32 {
        let n = self.cells.len() as f64;
        let lo = (gx0 - self.x).floor().clamp(0.0, n) as usize;
        let hi = (gx1 - self.x).ceil().clamp(0.0, n) as usize;
        let mut got = 0.0;
        for c in &mut self.cells[lo..hi] {
            let m = *c * frac;
            *c -= m;
            got += m;
        }
        if hi > lo {
            self.mark(lo, hi);
            self.urgent = true;
        }
        got
    }

    pub fn total(&self) -> f32 {
        self.cells.iter().sum()
    }

    pub fn clear(&mut self) {
        self.cells.iter_mut().for_each(|c| *c = 0.0);
        self.mark_all();
        self.maxh = 0.0;
    }

    /// Zwaartekracht: waar de helling te steil is schuift sneeuw naar beneden.
    fn relax(&mut self, repose: f32, flow: f32) {
        let n = self.cells.len();
        if n < 2 {
            return;
        }
        let (mut lo, mut hi) = (usize::MAX, 0);
        for pass in 0..2 {
            let fwd = self.flip ^ (pass == 1);
            for k in 0..n - 1 {
                let i = if fwd { k } else { n - 2 - k };
                let a = self.cells[i] - self.bases[i];
                let b = self.cells[i + 1] - self.bases[i + 1];
                let d = a - b;
                let m = if d > repose {
                    ((d - repose) * flow).min(self.cells[i])
                } else if d < -repose {
                    -((-d - repose) * flow).min(self.cells[i + 1])
                } else if d.abs() > 0.2 {
                    // Een klein beetje uitvlakken, zodat verse sneeuw zacht en rond oogt.
                    (d * 0.03).clamp(-self.cells[i + 1], self.cells[i])
                } else {
                    continue;
                };
                self.cells[i] -= m;
                self.cells[i + 1] += m;
                if m.abs() > 0.002 {
                    lo = lo.min(i);
                    hi = hi.max(i + 2);
                    self.urgent |= m.abs() > 0.05;
                }
            }
        }
        self.flip = !self.flip;
        if !self.walls {
            for (side, idx) in [(0usize, 0usize), (1, n - 1)] {
                let h = self.cells[idx];
                if h > EDGE_HOLD {
                    let m = (h - EDGE_HOLD) * 0.25;
                    self.cells[idx] -= m;
                    self.spill[side] += m;
                    lo = lo.min(idx);
                    hi = hi.max(idx + 1);
                }
            }
        }
        if hi > lo {
            self.mark(lo, hi);
        }
        self.maxh = self.cells.iter().cloned().fold(0.0, f32::max);
    }

    /// Laat een deel van de sneeuw los als klonten die met de opgegeven snelheid wegvliegen.
    fn release(&mut self, frac: &dyn Fn(usize) -> f32, vel: &dyn Fn(usize) -> (f64, f64), parts: &mut Vec<Particle>) {
        let n = self.cells.len();
        let total: f32 = (0..n).map(|i| self.cells[i] * frac(i)).sum();
        if total < 0.05 {
            return;
        }
        let room = MAX_PARTICLES.saturating_sub(parts.len()).max(50) as f32;
        let chunk = (total / 300.0).max(total / room).max(1.2);
        let mut acc = 0.0;
        let mut next = chunk * rnd(0.6, 1.5) as f32;
        let mut last = 0;
        for i in 0..n {
            let f = frac(i);
            if f <= 0.0 || self.cells[i] <= 0.0 {
                continue;
            }
            let m = self.cells[i] * f;
            self.cells[i] -= m;
            acc += m;
            last = i;
            if acc >= next {
                let (vx, vy) = vel(i);
                let sy = self.surface(i) - 1.0;
                parts.push(Particle::clump(self.x + i as f64 + 0.5, sy, vx + rnd(-15.0, 15.0), vy + rnd(-15.0, 5.0), acc));
                acc = 0.0;
                next = chunk * rnd(0.6, 1.5) as f32;
            }
        }
        if acc > 0.05 {
            let (vx, vy) = vel(last);
            parts.push(Particle::clump(self.x + last as f64 + 0.5, self.surface(last) - 1.0, vx, vy, acc));
        }
        self.mark_all();
        self.maxh = self.cells.iter().cloned().fold(0.0, f32::max);
    }

    /// Klonten die over de rand zijn geschoven vallen er nu echt af.
    fn emit_spills(&mut self, vx: f64, parts: &mut Vec<Particle>) {
        let n = self.cells.len();
        for side in 0..2 {
            if self.spill[side] < self.spill_next[side] {
                continue;
            }
            let (idx, gx, dir) = if side == 0 { (0, self.x - 1.0, -1.0) } else { (n - 1, self.x + n as f64 + 1.0, 1.0) };
            let y = self.surface(idx) - 0.5;
            parts.push(Particle::clump(gx, y, dir * rnd(4.0, 22.0) + vx * 0.5, rnd(-5.0, 20.0), self.spill[side]));
            self.spill[side] = 0.0;
            self.spill_next[side] = rnd(0.4, 2.6) as f32;
        }
    }

    /// Als er te veel ligt druppelt er af en toe onopvallend een klein klontje vanaf een rand.
    fn drip(&mut self, dt: f64, parts: &mut Vec<Particle>) {
        let n = self.cells.len();
        if n < 8 {
            return;
        }
        for c in &mut self.cells {
            if *c > WIN_MAX {
                self.drip_acc += *c - WIN_MAX;
                *c = WIN_MAX;
            }
        }
        let mean = self.total() / n as f32;
        if mean > WIN_COMFORT {
            self.drip_acc += (mean - WIN_COMFORT) * 0.0025 * n as f32 * dt as f32;
        }
        if self.drip_acc < self.drip_next {
            return;
        }
        let left = fastrand::bool();
        let range: Vec<usize> = if left { (0..8).collect() } else { (n - 8..n).collect() };
        let avail: f32 = range.iter().map(|&i| self.cells[i]).sum();
        let want = self.drip_next.min(avail * 0.5);
        if want > 0.05 {
            for &i in &range {
                self.cells[i] -= self.cells[i] / avail * want;
            }
            let (gx, dir, idx) = if left { (self.x - 1.0, -1.0, 0) } else { (self.x + n as f64 + 1.0, 1.0, n - 1) };
            parts.push(Particle::clump(gx, self.surface(idx), dir * rnd(2.0, 12.0), rnd(0.0, 15.0), want));
            self.mark(range[0], range[7] + 1);
        }
        self.drip_acc = 0.0;
        self.drip_next = rnd(0.5, 1.5) as f32;
    }
}

/// Sneeuw op de bovenrand van een venster van een andere app.
pub struct WinPile {
    pub id: u32,
    pub pile: Pile,
    pub h: f64,
    hist: VecDeque<(f64, f64, f64)>,
    cooldown_until: f64,
}

impl WinPile {
    fn new(w: &WinInfo) -> Self {
        WinPile { id: w.id, pile: Pile::new(w.x, w.y, w.w, false), h: w.h, hist: VecDeque::new(), cooldown_until: 0.0 }
    }

    pub fn rect(&self) -> Rect {
        Rect::new(self.pile.x, self.pile.y, self.pile.w(), self.h)
    }

    fn velocity(hist: &VecDeque<(f64, f64, f64)>, a: usize, b: usize) -> (f64, f64) {
        let (t0, x0, y0) = hist[a];
        let (t1, x1, y1) = hist[b];
        let dt = (t1 - t0).max(1e-3);
        ((x1 - x0) / dt, (y1 - y0) / dt)
    }

    fn update(&mut self, w: &WinInfo, t: f64, parts: &mut Vec<Particle>, dirty: &mut Vec<Rect>) {
        self.h = w.h;
        let p = &mut self.pile;
        let new_n = w.w.round().max(1.0) as usize;
        if new_n != p.cells.len() {
            // Formaat gewijzigd: sneeuw blijft op zijn plek liggen; wat buiten het venster
            // valt stort naar beneden, nieuwe ruimte vult zich via de zwaartekracht.
            dirty.push(p.bbox());
            let off = (w.x - p.x).round() as i64;
            let outside = |i: usize| {
                let j = i as i64 - off;
                j < 0 || j >= new_n as i64
            };
            p.release(
                &|i| if outside(i) { 1.0 } else { 0.0 },
                &|i| (if (i as i64) < off { -rnd(5.0, 30.0) } else { rnd(5.0, 30.0) }, 0.0),
                parts,
            );
            let mut cells = vec![0.0; new_n];
            for (i, &c) in p.cells.iter().enumerate() {
                let j = i as i64 - off;
                if j >= 0 && (j as usize) < new_n {
                    cells[j as usize] = c;
                }
            }
            p.cells = cells;
            p.x = w.x;
            p.y = w.y;
            p.visible = vec![true; new_n];
            p.compute_bases();
            p.maxh = p.cells.iter().cloned().fold(0.0, f32::max);
            p.mark_all();
            self.hist.clear();
            return;
        }
        if (w.x - p.x).abs() > 0.01 || (w.y - p.y).abs() > 0.01 {
            dirty.push(p.bbox());
            p.x = w.x;
            p.y = w.y;
            p.mark_all();
        }
        self.hist.push_back((t, w.x, w.y));
        if self.hist.len() > 8 {
            self.hist.pop_front();
        }
        let n = self.hist.len();
        if n < 7 || t < self.cooldown_until || p.maxh < 0.3 {
            return;
        }
        // Traagheid: stopt of schudt een venster abrupt, dan vliegt de sneeuw door.
        // Schiet het venster sneller omlaag dan de sneeuw kan vallen, dan komt die los.
        let (pvx, pvy) = Self::velocity(&self.hist, n - 7, n - 3);
        let (cvx, cvy) = Self::velocity(&self.hist, n - 3, n - 1);
        let ix = pvx - cvx;
        let iy = (pvy - cvy).min(0.0);
        let m = (ix * ix + iy * iy).sqrt();
        if m > 700.0 {
            let frac = ((m - 700.0) / 3000.0).clamp(0.0, 0.45) as f32;
            let (vx, vy) = (pvx.clamp(-900.0, 900.0), pvy.clamp(-900.0, 900.0));
            p.release(&|_| frac, &|_| (vx + rnd(-40.0, 40.0), vy + rnd(-60.0, 0.0)), parts);
            self.cooldown_until = t + 0.25;
        }
    }
}

pub enum Target {
    Win(usize),
    Ground(usize),
}

pub struct World {
    pub screens: Vec<Rect>,
    pub grounds: Vec<Pile>,
    /// Vensters van voor naar achter.
    pub piles: Vec<WinPile>,
    stash: HashMap<u32, (WinPile, f64)>,
    pub particles: Vec<Particle>,
    pub actors: Actors,
    pub t: f64,
    pub wind: f64,
    /// Vermenigvuldiger voor hoe snel sneeuw zich ophoopt (1 = rustig, een uur voor een hoopje).
    pub speed: f64,
    pub flake_mult: f64,
    spawn_acc: Vec<f64>,
    event_timer: f64,
    frame: u64,
    pub dirty: Vec<Rect>,
}

impl World {
    pub fn new(screens: Vec<Rect>) -> Self {
        let mut w = World {
            screens: Vec::new(),
            grounds: Vec::new(),
            piles: Vec::new(),
            stash: HashMap::new(),
            particles: Vec::new(),
            actors: Actors::default(),
            t: 0.0,
            wind: 0.0,
            speed: 1.0,
            flake_mult: 1.0,
            spawn_acc: Vec::new(),
            event_timer: 0.0,
            frame: 0,
            dirty: Vec::new(),
        };
        w.set_screens(screens);
        w
    }

    /// Nieuwe schermindeling; de grond van schermen die hetzelfde bleven blijft liggen.
    pub fn set_screens(&mut self, screens: Vec<Rect>) {
        let mut old: Vec<(Rect, Pile)> = self.screens.drain(..).zip(self.grounds.drain(..)).collect();
        for s in &screens {
            let g = match old.iter().position(|(r, _)| r == s) {
                Some(i) => old.swap_remove(i).1,
                None => Pile::new(s.x, s.y + s.h, s.w, true),
            };
            self.grounds.push(g);
        }
        self.spawn_acc = vec![0.0; screens.len()];
        self.screens = screens;
        self.actors.boy = None;
        self.actors.sweeper = None;
        self.actors.snowmen.clear();
    }

    pub fn clear_all(&mut self) {
        for p in &mut self.piles {
            p.pile.clear();
        }
        for g in &mut self.grounds {
            g.clear();
        }
        self.stash.clear();
        self.particles.retain(|p| p.kind == Kind::Flake);
        self.actors.snowmen.clear();
        for s in &self.screens {
            self.dirty.push(*s);
        }
    }

    /// `wins` is None als de vensterlijst deze frame niet is opgevraagd. Geeft true als er
    /// een venster bewoog, verscheen of verdween.
    pub fn step(&mut self, real_dt: f64, wins: Option<Vec<WinInfo>>, all_ids: &dyn Fn() -> HashSet<u32>) -> bool {
        let dt = real_dt.clamp(0.001, 0.05);
        self.t += dt;
        let t = self.t;
        self.wind = 14.0 * (t / 41.0).sin() + 9.0 * (t / 17.0 + 1.3).sin() + 4.0 * (t / 5.3).sin();
        let changed = match wins {
            Some(wins) => {
                let changed = self.sync_windows(wins, all_ids);
                self.update_visibility();
                changed
            }
            None => false,
        };
        self.spawn(dt);
        self.update_particles(dt);
        self.settle_piles(dt);
        self.actors.update(dt, &mut self.grounds, &self.screens, &mut self.particles, &mut self.dirty);
        self.events(dt);
        changed
    }

    fn sync_windows(&mut self, wins: Vec<WinInfo>, all_ids: &dyn Fn() -> HashSet<u32>) -> bool {
        let mut old: HashMap<u32, WinPile> = self.piles.drain(..).map(|p| (p.id, p)).collect();
        let mut appeared = 0;
        let mut moved = false;
        for w in &wins {
            if let Some(mut p) = old.remove(&w.id) {
                moved |= p.rect() != Rect::new(w.x, w.y, w.w.round().max(1.0), w.h);
                p.update(w, self.t, &mut self.particles, &mut self.dirty);
                self.piles.push(p);
            } else if let Some((mut p, _)) = self.stash.remove(&w.id) {
                // Terug van een andere Space: de sneeuw ligt er nog.
                p.hist.clear();
                p.update(w, self.t, &mut self.particles, &mut self.dirty);
                p.pile.x = w.x;
                p.pile.y = w.y;
                p.pile.mark_all();
                self.piles.push(p);
            } else {
                appeared += 1;
                self.piles.push(WinPile::new(w));
            }
        }
        let old_empty = old.is_empty();
        if !old_empty {
            // Verdwijnen er veel tegelijk, dan is het een Space-wissel: bewaren.
            // Eén venster dicht of geminimaliseerd: de sneeuw stort naar beneden.
            let space_switch = old.len() >= 3 || appeared >= 2 || self.piles.is_empty();
            let exists = all_ids();
            for (id, mut p) in old {
                self.dirty.push(p.pile.bbox());
                if space_switch && exists.contains(&id) {
                    self.stash.insert(id, (p, self.t));
                } else {
                    p.pile.release(&|_| 1.0, &|_| (rnd(-20.0, 20.0), rnd(-30.0, 10.0)), &mut self.particles);
                }
            }
        }
        let t = self.t;
        self.stash.retain(|_, (_, since)| t - *since < 3.0 * 3600.0);
        moved || appeared > 0 || !old_empty
    }

    /// Kolommen waarvan de vensterrand achter een ander venster zit tonen geen sneeuw
    /// en vangen ook geen vlokken.
    fn update_visibility(&mut self) {
        let rects: Vec<Rect> = self.piles.iter().map(|p| p.rect()).collect();
        for (k, wp) in self.piles.iter_mut().enumerate() {
            let p = &mut wp.pile;
            let n = p.cells.len();
            let mut vis = vec![true; n];
            let edge = p.y + 1.0;
            for f in &rects[..k] {
                if f.y <= edge && edge < f.y + f.h {
                    let lo = (f.x - p.x).floor().clamp(0.0, n as f64) as usize;
                    let hi = (f.x + f.w - p.x).ceil().clamp(0.0, n as f64) as usize;
                    vis[lo..hi].iter_mut().for_each(|v| *v = false);
                }
            }
            if vis != p.visible {
                p.visible = vis;
                p.mark_all();
            }
        }
    }

    fn spawn(&mut self, dt: f64) {
        for (k, s) in self.screens.iter().enumerate() {
            self.spawn_acc[k] += FLAKES_PER_PX * s.w * self.flake_mult * dt;
            while self.spawn_acc[k] >= 1.0 {
                self.spawn_acc[k] -= 1.0;
                if self.particles.len() >= MAX_PARTICLES {
                    continue;
                }
                let r = rnd(0.0, 1.0).powi(2) * 1.4 + 0.7;
                self.particles.push(Particle {
                    x: s.x + rnd(-80.0, s.w + 80.0),
                    y: s.y - 6.0,
                    vx: self.wind,
                    vy: 20.0,
                    mass: DEPOSIT * (self.speed / self.flake_mult) as f32,
                    r,
                    kind: Kind::Flake,
                    phase: rnd(0.0, 2.0 * PI),
                });
            }
        }
    }

    fn update_particles(&mut self, dt: f64) {
        let mut parts = std::mem::take(&mut self.particles);
        let mut keep = Vec::with_capacity(parts.len());
        let mut spawned = Vec::new();
        let bottom = self.screens.iter().map(|s| s.y + s.h).fold(f64::MIN, f64::max);
        let top = self.screens.iter().map(|s| s.y).fold(f64::MAX, f64::min);
        let left = self.screens.iter().map(|s| s.x).fold(f64::MAX, f64::min);
        let right = self.screens.iter().map(|s| s.x + s.w).fold(f64::MIN, f64::max);
        for mut p in parts.drain(..) {
            let y0 = p.y;
            match p.kind {
                Kind::Flake => {
                    let sway = (self.t * (0.9 + p.phase * 0.1) + p.phase).sin() * 16.0 / p.r;
                    p.vx += (self.wind + sway - p.vx) * (2.0 * dt).min(1.0);
                    let term = 24.0 + p.r * 12.0;
                    p.vy += (term - p.vy) * (1.5 * dt).min(1.0);
                }
                Kind::Clump | Kind::Puff => {
                    // Zwaartekracht met luchtweerstand: grote klonten vallen harder.
                    p.vy += G * dt;
                    let vt = (260.0 + p.mass as f64 * 25.0).min(650.0);
                    let k = G / (vt * vt);
                    let speed = (p.vx * p.vx + p.vy * p.vy).sqrt();
                    p.vx -= p.vx * (k * speed * dt).min(1.0);
                    p.vy -= p.vy * (k * speed * dt).min(1.0);
                    p.vx += (self.wind - p.vx) * 0.1 * dt;
                }
            }
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            if p.vy > 0.0 {
                if let Some(target) = self.find_landing(&p, y0) {
                    self.land(p, target, &mut spawned);
                    continue;
                }
            }
            if p.y > bottom + 30.0 || p.y < top - 3000.0 || p.x < left - 400.0 || p.x > right + 400.0 {
                continue;
            }
            keep.push(p);
        }
        keep.extend(spawned);
        self.particles = keep;
    }

    fn find_landing(&self, p: &Particle, y0: f64) -> Option<Target> {
        for (k, wp) in self.piles.iter().enumerate() {
            let pl = &wp.pile;
            let Some(i) = pl.col(p.x) else { continue };
            if !pl.visible[i] {
                continue;
            }
            let s = pl.surface(i);
            let band = pl.y + pl.bases[i] as f64 + 3.0;
            if p.y >= s && (y0 <= s + 0.5 || p.y <= band) {
                return Some(Target::Win(k));
            }
        }
        for (g, gr) in self.grounds.iter().enumerate() {
            let Some(i) = gr.col(p.x) else { continue };
            if p.y >= gr.surface(i) && y0 <= gr.y {
                return Some(Target::Ground(g));
            }
        }
        None
    }

    fn land(&mut self, p: Particle, target: Target, spawned: &mut Vec<Particle>) {
        let pile = match target {
            Target::Win(k) => &mut self.piles[k].pile,
            Target::Ground(g) => &mut self.grounds[g],
        };
        let mut mass = p.mass;
        // Harde klappen spatten een beetje op.
        if p.kind == Kind::Clump && p.vy > 220.0 && mass > 1.0 {
            let splash = mass * 0.3;
            let n = fastrand::usize(2..5);
            let sy = pile.surface(pile.clamp_col(p.x)) - 1.0;
            for _ in 0..n {
                spawned.push(Particle::puff(
                    p.x,
                    sy,
                    p.vx * 0.3 + rnd(-60.0, 60.0),
                    -p.vy * rnd(0.12, 0.28),
                    splash / n as f32,
                ));
            }
            mass -= splash;
        }
        let spread = if p.kind == Kind::Flake { 1.0 } else { p.r * 1.3 + 1.0 };
        pile.deposit(p.x, mass, spread);
    }

    fn settle_piles(&mut self, dt: f64) {
        self.frame += 1;
        let slow_tick = self.frame % 6 == 0;
        for wp in &mut self.piles {
            let p = &mut wp.pile;
            p.relax(WIN_REPOSE, 0.3);
            p.emit_spills(0.0, &mut self.particles);
            p.drip(dt, &mut self.particles);
            if let Some(r) = p.take_dirty(slow_tick) {
                self.dirty.push(r);
            }
        }
        for g in &mut self.grounds {
            g.relax(GROUND_REPOSE, 0.3);
            let mut changed = false;
            for c in &mut g.cells {
                if *c > GROUND_COMFORT {
                    *c -= ((*c - GROUND_COMFORT) * 0.004 * dt as f32).max(0.0);
                    *c = c.min(GROUND_MAX);
                    changed = true;
                }
            }
            if changed {
                let n = g.cells.len();
                g.mark(0, n);
            }
            if let Some(r) = g.take_dirty(slow_tick) {
                self.dirty.push(r);
            }
        }
    }

    fn events(&mut self, dt: f64) {
        self.event_timer += dt;
        if self.event_timer < EVENT_EVERY {
            return;
        }
        self.event_timer = 0.0;
        if fastrand::f64() < EVENT_CHANCE && !self.actors.busy() {
            if fastrand::bool() {
                self.summon_boy();
            } else {
                self.summon_sweeper();
            }
        }
    }

    fn pick_screen(&self) -> usize {
        // Meestal het hoofdscherm, soms een ander.
        if self.screens.len() > 1 && fastrand::f64() < 0.35 { fastrand::usize(..self.screens.len()) } else { 0 }
    }

    pub fn summon_boy(&mut self) {
        if self.actors.boy.is_none() && !self.screens.is_empty() {
            let g = self.pick_screen();
            self.actors.summon_boy(g, &self.screens[g]);
        }
    }

    pub fn summon_sweeper(&mut self) {
        if self.actors.sweeper.is_none() && !self.screens.is_empty() {
            let g = self.pick_screen();
            self.actors.summon_sweeper(g, &self.screens[g], &self.grounds[g]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world_with_window() -> (World, WinInfo) {
        let mut w = World::new(vec![Rect::new(0.0, 0.0, 1600.0, 1000.0)]);
        w.speed = 0.0;
        let win = WinInfo { id: 1, x: 200.0, y: 300.0, w: 400.0, h: 300.0 };
        w.step(1.0 / 30.0, Some(vec![win]), &|| [1].into());
        w.piles[0].pile.cells.iter_mut().for_each(|c| *c = 10.0);
        w.piles[0].pile.maxh = 10.0;
        (w, win)
    }

    fn snow_on(w: &World) -> f32 {
        w.piles[0].pile.total()
    }

    #[test]
    fn steady_drag_with_stutter_keeps_snow() {
        let (mut w, mut win) = world_with_window();
        for f in 0..90 {
            // Haperende updates: soms geen beweging, daarna een dubbele stap.
            win.x += match f % 3 { 0 => 0.0, 1 => 40.0, _ => 20.0 };
            w.step(1.0 / 30.0, Some(vec![win]), &|| [1].into());
        }
        assert_eq!(w.piles[0].cooldown_until, 0.0, "slepen zonder schokken mag geen sneeuw afwerpen");
    }

    #[test]
    fn abrupt_stop_throws_snow() {
        let (mut w, mut win) = world_with_window();
        for _ in 0..20 {
            win.x += 40.0;
            w.step(1.0 / 30.0, Some(vec![win]), &|| [1].into());
        }
        let before = snow_on(&w);
        let mut least = before;
        for _ in 0..6 {
            w.step(1.0 / 30.0, Some(vec![win]), &|| [1].into());
            least = least.min(snow_on(&w));
        }
        assert!(w.piles[0].cooldown_until > 0.0);
        assert!(least < before * 0.9, "{before} -> {least}");
    }

    #[test]
    fn closing_window_drops_snow_as_clumps() {
        let (mut w, _) = world_with_window();
        let mass = snow_on(&w);
        w.step(1.0 / 30.0, Some(vec![]), &|| HashSet::new());
        let falling: f32 = w.particles.iter().filter(|p| p.kind != Kind::Flake).map(|p| p.mass).sum();
        assert!(w.piles.is_empty());
        assert!((falling - mass).abs() < mass * 0.05, "{falling} vs {mass}");
    }

    #[test]
    fn takes_about_an_hour_to_build_up() {
        let (mut w, win) = world_with_window();
        w.speed = 1.0;
        w.piles[0].pile.clear();
        let mut at = Vec::new();
        for f in 1..=(30 * 3600) {
            w.step(1.0 / 30.0, Some(vec![win]), &|| [1].into());
            if f % (30 * 900) == 0 {
                let p = &w.piles[0].pile;
                at.push((p.total() / p.cells.len() as f32, p.maxh));
            }
        }
        eprintln!("gemiddeld/hoogst per kwartier: {at:?}");
        assert!(at[0].0 < 5.0, "na 15 min al te veel: {:?}", at[0]);
        assert!(at[3].0 > 9.0, "na een uur te weinig: {:?}", at[3]);
    }

    #[test]
    fn shrinking_spills_outside_part() {
        let (mut w, mut win) = world_with_window();
        win.w = 300.0;
        w.step(1.0 / 30.0, Some(vec![win]), &|| [1].into());
        assert_eq!(w.piles[0].pile.cells.len(), 300);
        assert!(w.particles.iter().any(|p| p.kind == Kind::Clump && p.x > 499.0));
    }
}
