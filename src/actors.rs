//! Bezoekers: een sneeuwman met bezem die de grond opruimt, en een jongetje dat een
//! sneeuwbal door de sneeuw rolt en er een sneeuwpop van bouwt.

use std::f64::consts::PI;

use crate::world::{rnd, Particle, Pile, Rect, G};

/// Een gebouwde sneeuwpop blijft nooit staan: zodra het jongetje wegloopt smelt hij
/// binnen een halve minuut terug in de sneeuw.
const MELT_START: f64 = 5.0;
const MELT_TIME: f64 = 30.0;

pub struct Snowman {
    pub x: f64,
    pub g: usize,
    pub r: [f64; 3],
    pub count: usize,
    pub off: [f64; 3],
    vel: [f64; 3],
    pub deco: f64,
    pub scale: f64,
    age: f64,
}

impl Snowman {
    fn new(x: f64, g: usize, r0: f64) -> Self {
        Snowman { x, g, r: [r0, 0.0, 0.0], count: 1, off: [0.0; 3], vel: [0.0; 3], deco: 0.0, scale: 1.0, age: 0.0 }
    }

    /// Middelpunten van de bollen, gestapeld op het sneeuwoppervlak `gy`.
    pub fn centers(&self, gy: f64) -> [f64; 3] {
        let s = self.scale;
        let y0 = gy - self.r[0] * s * 0.93 + self.off[0];
        let y1 = y0 - (self.r[0] + self.r[1]) * s * 0.8 + self.off[1];
        let y2 = y1 - (self.r[1] + self.r[2]) * s * 0.8 + self.off[2];
        [y0, y1, y2]
    }

    /// Net neergezette bollen vallen het laatste stukje en stuiteren een beetje na.
    fn settle(&mut self, dt: f64) {
        for k in 0..3 {
            if self.off[k] < 0.0 || self.vel[k] != 0.0 {
                self.vel[k] += G * dt;
                self.off[k] += self.vel[k] * dt;
                if self.off[k] >= 0.0 {
                    self.off[k] = 0.0;
                    self.vel[k] = if self.vel[k] > 60.0 { -self.vel[k] * 0.25 } else { 0.0 };
                }
            }
        }
    }

    pub fn bbox(&self, gy: f64) -> Rect {
        let h = (self.r[0] + self.r[1] + self.r[2]) * 2.0 + 40.0;
        let w = self.r[0] * 2.0 + 50.0;
        Rect::new(self.x - w / 2.0, gy - h, w, h + 8.0)
    }
}

pub struct Ball {
    pub x: f64,
    pub y: f64,
    pub r: f64,
    area: f64,
    pub rot: f64,
}

impl Ball {
    fn new(x: f64, r: f64) -> Self {
        Ball { x, y: 0.0, r, area: PI * r * r, rot: 0.0 }
    }

    /// Rol naar `nx`; als `collect` dan plakt de sneeuw eronder aan de bal vast.
    fn roll(&mut self, nx: f64, gr: &mut Pile, collect: bool) {
        if collect {
            let got = gr.take(self.x.min(nx) - 2.0, self.x.max(nx) + 2.0, 0.3);
            self.area += got as f64;
            self.r = (self.area / PI).sqrt();
        }
        self.rot += (nx - self.x) / self.r.max(1.0);
        self.x = nx;
        self.y = gr.surface_at(nx) - self.r * 0.88;
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Pose {
    Walk,
    Push,
    Lift,
    Crouch,
    Deco,
    Wave,
}

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    RollBase,
    Turn,
    WalkAround,
    RollMid,
    Lift,
    Head,
    Decorate,
    Wave,
    Leave,
}

pub struct Boy {
    pub g: usize,
    pub x: f64,
    pub y: f64,
    pub dir: f64,
    pub step: f64,
    pub pose: Pose,
    pub pt: f64,
    pub ball: Option<Ball>,
    pub sm: Option<Snowman>,
    phase: Phase,
    target: f64,
    /// Waar de sneeuwpop moet komen: ergens in het midden van het scherm.
    site: f64,
    passes: u32,
    x2: f64,
    from: (f64, f64),
    scooped: bool,
}

impl Boy {
    fn new(g: usize, sc: &Rect) -> Self {
        let dir = if fastrand::bool() { 1.0 } else { -1.0 };
        let x = if dir > 0.0 { sc.x - 25.0 } else { sc.x + sc.w + 25.0 };
        Boy {
            g,
            x,
            y: sc.y + sc.h,
            dir,
            step: 0.0,
            pose: Pose::Push,
            pt: 0.0,
            ball: Some(Ball::new(x + dir * 11.0, 4.0)),
            sm: None,
            phase: Phase::RollBase,
            target: rnd(19.0, 25.0),
            site: sc.x + sc.w * rnd(0.3, 0.7),
            passes: 0,
            x2: 0.0,
            from: (0.0, 0.0),
            scooped: false,
        }
    }

    pub fn bbox(&self) -> Rect {
        Rect::new(self.x - 110.0, self.y - 150.0, 220.0, 160.0)
    }

    fn walk(&mut self, dt: f64, speed: f64) {
        self.x += self.dir * speed * dt;
        self.step += speed * dt * 0.22;
    }

    fn go(&mut self, phase: Phase) {
        self.phase = phase;
        self.pt = 0.0;
    }

    /// Geeft true als het jongetje klaar is en uit beeld is gelopen.
    fn update(&mut self, dt: f64, gr: &mut Pile, sc: &Rect, built: &mut Vec<Snowman>) -> bool {
        self.pt += dt;
        if let Some(sm) = &mut self.sm {
            sm.settle(dt);
        }
        match self.phase {
            Phase::RollBase => {
                self.pose = Pose::Push;
                self.walk(dt, 38.0);
                let ball = self.ball.as_mut().unwrap();
                ball.roll(self.x + self.dir * (ball.r + 7.0), gr, true);
                // Rol heen en weer rond de gekozen plek tot de bal groot genoeg is.
                let past = (ball.x - self.site) * self.dir;
                let big = ball.r >= self.target || self.passes >= 4;
                let edge = if self.dir > 0.0 { sc.x + sc.w - 60.0 } else { sc.x + 60.0 };
                if !big && (past > 140.0 || (ball.x - edge) * self.dir >= 0.0) {
                    self.passes += 1;
                    self.dir = -self.dir;
                    self.x2 = ball.x - self.dir * (ball.r + 7.0);
                    self.go(Phase::Turn);
                } else if big && past >= 0.0 {
                    let r = ball.r.max(12.0);
                    let site = ball.x;
                    self.sm = Some(Snowman::new(site, self.g, r));
                    self.ball = None;
                    // Verse sneeuw zoeken aan de kant waar nog plek is.
                    let room = if self.dir > 0.0 { sc.x + sc.w - site } else { site - sc.x };
                    let side = if room > r + 160.0 { self.dir } else { -self.dir };
                    self.x2 = (site + side * (r + rnd(120.0, 170.0))).clamp(sc.x + 40.0, sc.x + sc.w - 40.0);
                    self.go(Phase::WalkAround);
                }
            }
            Phase::Turn => {
                // Om de bal heen lopen om hem de andere kant op te duwen.
                self.pose = Pose::Walk;
                let d = (self.x2 - self.x).signum();
                self.x += d * 55.0 * dt;
                self.step += 55.0 * dt * 0.22;
                if (self.x2 - self.x).abs() < 2.0 {
                    self.x = self.x2;
                    self.go(Phase::RollBase);
                }
            }
            Phase::WalkAround => {
                self.pose = Pose::Walk;
                self.dir = (self.x2 - self.x).signum();
                self.walk(dt, 55.0);
                if (self.x2 - self.x).abs() < 2.0 {
                    let site = self.sm.as_ref().unwrap().x;
                    self.dir = (site - self.x).signum();
                    let mut b = Ball::new(self.x + self.dir * 11.0, 4.0);
                    b.y = gr.surface_at(b.x) - 4.0;
                    self.ball = Some(b);
                    self.go(Phase::RollMid);
                }
            }
            Phase::RollMid => {
                self.pose = Pose::Push;
                self.walk(dt, 34.0);
                let sm = self.sm.as_ref().unwrap();
                let (site, base) = (sm.x, sm.r[0]);
                let ball = self.ball.as_mut().unwrap();
                let collect = ball.r < base * 0.7;
                ball.roll(self.x + self.dir * (ball.r + 7.0), gr, collect);
                if (site - ball.x).abs() <= base + ball.r * 0.9 + 1.0 || (site - ball.x) * self.dir < 0.0 {
                    ball.r = ball.r.max(base * 0.5);
                    self.from = (ball.x, ball.y);
                    self.go(Phase::Lift);
                }
            }
            Phase::Lift => {
                self.pose = Pose::Lift;
                let u = (self.pt / 1.3).min(1.0);
                let e = u * u * (3.0 - 2.0 * u);
                let sm = self.sm.as_mut().unwrap();
                let ball = self.ball.as_mut().unwrap();
                let gy = gr.surface_at(sm.x);
                let ty = sm.centers(gy)[0] - (sm.r[0] + ball.r) * 0.8 - 7.0;
                ball.x = self.from.0 + (sm.x - self.from.0) * e;
                ball.y = self.from.1 + (ty - self.from.1) * e - (PI * u).sin() * 26.0;
                let stand = sm.x - self.dir * (sm.r[0] + 9.0);
                self.x += (stand - self.x) * (3.0 * dt).min(1.0);
                if u >= 1.0 {
                    sm.r[1] = ball.r;
                    sm.count = 2;
                    sm.off[1] = -7.0;
                    self.ball = None;
                    self.go(Phase::Head);
                }
            }
            Phase::Head => {
                let sm = self.sm.as_mut().unwrap();
                if self.pt < 0.9 {
                    self.pose = Pose::Crouch;
                    if self.pt > 0.4 && !self.scooped {
                        self.scooped = true;
                        gr.take(self.x - 14.0, self.x + 14.0, 0.5);
                        let mut b = Ball::new(self.x + self.dir * 9.0, sm.r[1] * 0.66);
                        b.y = self.y - 6.0;
                        self.ball = Some(b);
                        self.from = (self.x + self.dir * 9.0, self.y - 6.0);
                    }
                } else {
                    self.pose = Pose::Lift;
                    let u = ((self.pt - 0.9) / 1.0).min(1.0);
                    let e = u * u * (3.0 - 2.0 * u);
                    let ball = self.ball.as_mut().unwrap();
                    let gy = gr.surface_at(sm.x);
                    let ty = sm.centers(gy)[1] - (sm.r[1] + ball.r) * 0.8 - 6.0;
                    ball.x = self.from.0 + (sm.x - self.from.0) * e;
                    ball.y = self.from.1 + (ty - self.from.1) * e - (PI * u).sin() * 22.0;
                    if u >= 1.0 {
                        sm.r[2] = ball.r;
                        sm.count = 3;
                        sm.off[2] = -6.0;
                        self.ball = None;
                        self.go(Phase::Decorate);
                    }
                }
            }
            Phase::Decorate => {
                self.pose = Pose::Deco;
                self.sm.as_mut().unwrap().deco = (self.pt / 2.6).min(1.0);
                if self.pt > 2.9 {
                    self.go(Phase::Wave);
                }
            }
            Phase::Wave => {
                self.pose = Pose::Wave;
                if self.pt > 2.2 {
                    built.push(self.sm.take().unwrap());
                    self.dir = if self.x - sc.x < sc.x + sc.w - self.x { -1.0 } else { 1.0 };
                    self.go(Phase::Leave);
                }
            }
            Phase::Leave => {
                self.pose = Pose::Walk;
                self.walk(dt, 60.0);
                if self.x < sc.x - 40.0 || self.x > sc.x + sc.w + 40.0 {
                    return true;
                }
            }
        }
        self.y = gr.surface_at(self.x);
        false
    }
}

pub struct Sweeper {
    pub g: usize,
    pub x: f64,
    pub dir: f64,
    /// Hoogte van de huidige sprong (negatief = omhoog).
    pub hop: f64,
    vy: f64,
    air: bool,
    pub squash: f64,
    cool: f64,
    pub broom: f64,
    last_b: f64,
    pub heap: f64,
    fling: f32,
    pub t: f64,
}

impl Sweeper {
    fn new(g: usize, sc: &Rect, gr: &Pile) -> Self {
        // Begin aan de kant waar de meeste sneeuw ligt en veeg die het scherm af.
        let half = gr.cells.len() / 2;
        let left: f32 = gr.cells[..half].iter().sum();
        let right: f32 = gr.cells[half..].iter().sum();
        let dir = if left >= right { -1.0 } else { 1.0 };
        let x = if dir > 0.0 { sc.x - 50.0 } else { sc.x + sc.w + 50.0 };
        Sweeper { g, x, dir, hop: 0.0, vy: 0.0, air: false, squash: 0.0, cool: 0.3, broom: 0.0, last_b: x, heap: 0.0, fling: 0.0, t: 0.0 }
    }

    pub fn broom_x(&self) -> f64 {
        self.x + self.dir * (27.0 + self.broom.sin() * 5.0)
    }

    pub fn heap_r(&self) -> f64 {
        (2.0 * self.heap / PI).sqrt().min(30.0)
    }

    pub fn bbox(&self, gy: f64) -> Rect {
        Rect::new(self.x - 120.0, gy - 110.0, 240.0, 120.0)
    }

    fn update(&mut self, dt: f64, gr: &mut Pile, sc: &Rect, parts: &mut Vec<Particle>) -> bool {
        self.t += dt;
        self.broom += dt * 13.0;
        // Hij springt vooruit; zwaartekracht trekt hem terug, bij het landen plet hij even in.
        if !self.air {
            self.cool -= dt;
            if self.cool <= 0.0 {
                self.vy = -175.0;
                self.air = true;
            }
        } else {
            self.vy += G * dt;
            self.hop += self.vy * dt;
            self.x += self.dir * 115.0 * dt;
            if self.hop >= 0.0 {
                self.hop = 0.0;
                self.vy = 0.0;
                self.air = false;
                self.squash = 0.28;
                self.cool = rnd(0.08, 0.2);
            }
        }
        self.squash *= (1.0 - 9.0 * dt).max(0.0);

        let xb = self.broom_x();
        let got = gr.take(self.last_b.min(xb) - 3.0, self.last_b.max(xb) + 3.0, 0.45);
        self.heap += got as f64 * 0.85;
        self.fling += got * 0.15;
        while self.fling > 1.4 {
            self.fling -= 1.4;
            parts.push(Particle::puff(xb + self.dir * 3.0, gr.surface_at(xb) - 2.0, self.dir * rnd(60.0, 190.0), -rnd(110.0, 260.0), 1.4));
        }
        self.last_b = xb;
        // De opgeveegde berg schuift voor hem uit en valt over de schermrand weg.
        let hx = xb + self.dir * (self.heap_r() * 0.9 + 4.0);
        if hx < sc.x - self.heap_r() || hx > sc.x + sc.w + self.heap_r() {
            self.heap = 0.0;
        }
        (self.dir > 0.0 && self.x > sc.x + sc.w + 70.0) || (self.dir < 0.0 && self.x < sc.x - 70.0)
    }
}

#[derive(Default)]
pub struct Actors {
    pub boy: Option<Boy>,
    pub sweeper: Option<Sweeper>,
    pub snowmen: Vec<Snowman>,
}

impl Actors {
    pub fn busy(&self) -> bool {
        self.boy.is_some() || self.sweeper.is_some()
    }

    pub fn summon_boy(&mut self, g: usize, sc: &Rect) {
        self.boy = Some(Boy::new(g, sc));
    }

    pub fn summon_sweeper(&mut self, g: usize, sc: &Rect, gr: &Pile) {
        self.sweeper = Some(Sweeper::new(g, sc, gr));
    }

    pub fn update(&mut self, dt: f64, grounds: &mut [Pile], screens: &[Rect], parts: &mut Vec<Particle>, dirty: &mut Vec<Rect>) {
        if let Some(b) = &mut self.boy {
            let g = b.g;
            dirty.push(b.bbox());
            let done = b.update(dt, &mut grounds[g], &screens[g], &mut self.snowmen);
            dirty.push(b.bbox());
            if done {
                self.boy = None;
            }
        }
        if let Some(s) = &mut self.sweeper {
            let g = s.g;
            dirty.push(s.bbox(grounds[g].surface_at(s.x)));
            let done = s.update(dt, &mut grounds[g], &screens[g], parts);
            dirty.push(s.bbox(grounds[g].surface_at(s.x)));
            if done {
                self.sweeper = None;
            }
        }
        self.snowmen.retain_mut(|sm| {
            let gr = &mut grounds[sm.g];
            sm.settle(dt);
            sm.age += dt;
            dirty.push(sm.bbox(gr.surface_at(sm.x)));
            if sm.age > MELT_START {
                // Langzaam wegsmelten: de sneeuw zakt terug in de grond.
                let p = ((sm.age - MELT_START) / MELT_TIME).min(1.0);
                let ns = 1.0 - p;
                let area: f64 = sm.r.iter().map(|r| PI * r * r).sum();
                let lost = area * (sm.scale * sm.scale - ns * ns);
                if lost > 0.0 {
                    gr.deposit(sm.x, lost as f32 * 0.5, sm.r[0] * 1.5);
                }
                sm.scale = ns;
                return p < 1.0;
            }
            true
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snowman_never_stays() {
        let sc = Rect::new(0.0, 0.0, 1200.0, 800.0);
        let mut grounds = vec![Pile::new(sc.x, sc.y + sc.h, sc.w, true)];
        for c in &mut grounds[0].cells {
            *c = 12.0;
        }
        let mut a = Actors::default();
        a.summon_boy(0, &sc);
        let (mut parts, mut dirty) = (Vec::new(), Vec::new());
        let mut built = false;
        for _ in 0..(30 * 240) {
            a.update(1.0 / 30.0, &mut grounds, &[sc], &mut parts, &mut dirty);
            built |= !a.snowmen.is_empty();
        }
        assert!(built, "het jongetje had een sneeuwpop moeten bouwen");
        assert!(a.boy.is_none() && a.snowmen.is_empty(), "er staat nog een sneeuwpop");
    }
}
