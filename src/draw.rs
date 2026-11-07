//! Tekent de wereld met CoreGraphics in een geflipte NSView (y naar beneden).

use std::f64::consts::PI;

use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_core_graphics::{CGContext, CGLineCap};

use crate::actors::{Ball, Boy, Pose, Snowman, Sweeper};
use crate::world::{Kind, Pile, Rect, World};

const SNOW: (f64, f64, f64) = (0.97, 0.98, 1.0);
const SHADE: (f64, f64, f64) = (0.55, 0.65, 0.80);
const EDGE: (f64, f64, f64) = (0.70, 0.78, 0.90);

struct P<'a>(&'a CGContext);

impl P<'_> {
    fn c(&self) -> Option<&CGContext> {
        Some(self.0)
    }
    fn fill(&self, (r, g, b): (f64, f64, f64), a: f64) {
        CGContext::set_rgb_fill_color(self.c(), r, g, b, a);
    }
    fn stroke(&self, (r, g, b): (f64, f64, f64), a: f64, w: f64) {
        CGContext::set_rgb_stroke_color(self.c(), r, g, b, a);
        CGContext::set_line_width(self.c(), w);
    }
    fn mv(&self, x: f64, y: f64) {
        CGContext::move_to_point(self.c(), x, y);
    }
    fn ln(&self, x: f64, y: f64) {
        CGContext::add_line_to_point(self.c(), x, y);
    }
    fn ellipse(&self, x: f64, y: f64, rx: f64, ry: f64) {
        CGContext::add_ellipse_in_rect(self.c(), CGRect::new(CGPoint::new(x - rx, y - ry), CGSize::new(2.0 * rx, 2.0 * ry)));
    }
    fn circle(&self, x: f64, y: f64, r: f64) {
        self.ellipse(x, y, r, r);
    }
    fn do_fill(&self) {
        CGContext::fill_path(self.c());
    }
    fn do_stroke(&self) {
        CGContext::stroke_path(self.c());
    }
    fn dot(&self, x: f64, y: f64, r: f64, col: (f64, f64, f64), a: f64) {
        self.fill(col, a);
        self.circle(x, y, r);
        self.do_fill();
    }
    fn line(&self, x0: f64, y0: f64, x1: f64, y1: f64, col: (f64, f64, f64), w: f64) {
        self.stroke(col, 1.0, w);
        self.mv(x0, y0);
        self.ln(x1, y1);
        self.do_stroke();
    }
    fn poly(&self, pts: &[(f64, f64)], col: (f64, f64, f64), a: f64) {
        self.fill(col, a);
        self.mv(pts[0].0, pts[0].1);
        for &(x, y) in &pts[1..] {
            self.ln(x, y);
        }
        CGContext::close_path(self.c());
        self.do_fill();
    }
    /// Een sneeuwbol met schaduwrand, zodat hij ook op witte vensters zichtbaar is.
    fn snowball(&self, x: f64, y: f64, r: f64) {
        self.dot(x + 0.6, y + 1.0, r + 0.4, SHADE, 0.55);
        self.dot(x, y, r, SNOW, 1.0);
        self.stroke(EDGE, 0.8, 0.7);
        self.circle(x, y, r);
        self.do_stroke();
    }
}

/// `particles`: ook de vallende vlokken tekenen (alleen voor snapshots; op het scherm zijn het laagjes).
pub fn draw(cg: &CGContext, w: &World, ox: f64, oy: f64, vw: f64, vh: f64, particles: bool) {
    let p = P(cg);
    let view = Rect::new(ox, oy, vw, vh);
    CGContext::save_g_state(p.c());
    CGContext::translate_ctm(p.c(), -ox, -oy);
    CGContext::set_line_cap(p.c(), CGLineCap::Round);

    for wp in w.piles.iter().rev() {
        if wp.pile.maxh > 0.02 && view.intersects(&wp.pile.bbox()) {
            draw_pile(&p, &wp.pile);
        }
    }
    for g in &w.grounds {
        if g.maxh > 0.02 && view.intersects(&g.bbox()) {
            draw_pile(&p, g);
        }
    }
    for sm in &w.actors.snowmen {
        draw_snowman(&p, sm, w.grounds[sm.g].surface_at(sm.x));
    }
    if let Some(s) = &w.actors.sweeper {
        draw_sweeper(&p, s, &w.grounds[s.g]);
    }
    if let Some(b) = &w.actors.boy {
        if let Some(sm) = &b.sm {
            draw_snowman(&p, sm, w.grounds[sm.g].surface_at(sm.x));
        }
        draw_boy(&p, b);
    }
    if particles {
        draw_particles(&p, w, &view);
    }
    CGContext::restore_g_state(p.c());
}

fn draw_particles(p: &P, w: &World, view: &Rect) {
    // Eerst alle zachte randjes in één pad, dan alle witte kernen: dat is snel.
    let on = |x: f64, y: f64| view.contains(x, y) || view.contains(x + 6.0, y + 6.0) || view.contains(x - 6.0, y - 6.0);
    p.fill((0.40, 0.50, 0.65), 0.22);
    for q in &w.particles {
        if on(q.x, q.y) {
            p.circle(q.x, q.y + 0.3, q.r + 0.7);
        }
    }
    p.do_fill();
    p.fill(SNOW, 0.92);
    for q in &w.particles {
        if on(q.x, q.y) {
            let squash = if q.kind == Kind::Flake { 1.0 } else { 1.0 + (q.vy.abs() / 2000.0).min(0.3) };
            p.ellipse(q.x, q.y, q.r / squash.sqrt(), q.r * squash);
        }
    }
    p.do_fill();
}

fn draw_pile(p: &P, pl: &Pile) {
    let n = pl.cells.len();
    let h = |i: usize| {
        let a = pl.cells[i.saturating_sub(1)];
        let b = pl.cells[(i + 1).min(n - 1)];
        ((a + 2.0 * pl.cells[i] + b) / 4.0) as f64
    };
    let base = |i: usize| pl.y + pl.bases[i] as f64;
    let mut i = 0;
    while i < n {
        if !pl.visible[i] {
            i += 1;
            continue;
        }
        let start = i;
        while i < n && pl.visible[i] {
            i += 1;
        }
        let end = i;
        if pl.cells[start..end].iter().all(|&c| c < 0.02) {
            continue;
        }
        for (off, col, a) in [(1.0, SHADE, 0.6), (0.0, SNOW, 0.98)] {
            p.fill(col, a);
            p.mv(pl.x + start as f64, base(start) + off);
            for j in start..end {
                p.ln(pl.x + j as f64 + 0.5, base(j) - h(j) + off);
            }
            p.ln(pl.x + end as f64, base(end - 1) + off);
            for j in (start..end).rev() {
                if pl.bases[j] != 0.0 || j == start || j == end - 1 {
                    p.ln(pl.x + j as f64 + 0.5, base(j) + off);
                }
            }
            CGContext::close_path(p.c());
            p.do_fill();
        }
        // Een fijn randje bovenop zodat de sneeuw ook op witte vensters opvalt.
        p.stroke(EDGE, 0.85, 0.6);
        let mut drawing = false;
        for j in start..end {
            let hj = h(j);
            if hj > 0.4 {
                let (x, y) = (pl.x + j as f64 + 0.5, base(j) - hj);
                if drawing { p.ln(x, y) } else { p.mv(x, y) }
                drawing = true;
            } else {
                drawing = false;
            }
        }
        p.do_stroke();
    }
}

const COAL: (f64, f64, f64) = (0.12, 0.12, 0.14);
const CARROT: (f64, f64, f64) = (0.98, 0.52, 0.12);
const STICK: (f64, f64, f64) = (0.45, 0.30, 0.18);

fn draw_snowman(p: &P, sm: &Snowman, gy: f64) {
    let c = sm.centers(gy);
    let s = sm.scale;
    for k in 0..sm.count {
        p.snowball(sm.x, c[k], sm.r[k] * s);
    }
    if sm.count < 3 || sm.deco <= 0.0 || s < 0.35 {
        return;
    }
    let d = sm.deco;
    let (hx, hy, hr) = (sm.x, c[2], sm.r[2] * s);
    let (mx, my, mr) = (sm.x, c[1], sm.r[1] * s);
    if d > 0.1 {
        p.dot(hx - hr * 0.35, hy - hr * 0.2, hr * 0.12 + 0.4, COAL, 1.0);
        p.dot(hx + hr * 0.35, hy - hr * 0.2, hr * 0.12 + 0.4, COAL, 1.0);
    }
    if d > 0.3 {
        p.poly(&[(hx, hy - hr * 0.05), (hx + hr * 0.95, hy + hr * 0.12), (hx, hy + hr * 0.2)], CARROT, 1.0);
    }
    if d > 0.45 {
        for k in 0..5 {
            let a = PI * (0.25 + 0.125 * k as f64);
            p.dot(hx + (a.cos()) * hr * 0.55, hy + a.sin() * hr * 0.5 + hr * 0.1, hr * 0.06 + 0.35, COAL, 1.0);
        }
    }
    if d > 0.6 {
        for k in 0..3 {
            p.dot(mx, my - mr * 0.45 + k as f64 * mr * 0.42, mr * 0.09 + 0.4, COAL, 1.0);
        }
    }
    if d > 0.8 {
        for side in [-1.0, 1.0] {
            let (sx, sy) = (mx + side * mr * 0.9, my - mr * 0.2);
            let (ex, ey) = (sx + side * mr * 1.1, sy - mr * 0.7);
            p.line(sx, sy, ex, ey, STICK, 1.4);
            p.line(sx + side * mr * 0.7, sy - mr * 0.45, sx + side * mr * 0.75, sy - mr * 0.85, STICK, 1.0);
        }
    }
    if d > 0.95 {
        p.line(hx - hr * 0.8, hy + hr * 0.85, hx + hr * 0.8, hy + hr * 0.85, (0.85, 0.15, 0.2), 2.6);
        p.line(hx + hr * 0.5, hy + hr * 0.85, hx + hr * 0.7, hy + hr * 1.6, (0.85, 0.15, 0.2), 2.4);
    }
}

fn draw_ball(p: &P, b: &Ball) {
    p.snowball(b.x, b.y, b.r);
    // Een paar vlekjes laten zien dat de bal rolt.
    for k in 0..3 {
        let a = b.rot + k as f64 * 2.1;
        p.dot(b.x + a.cos() * b.r * 0.55, b.y + a.sin() * b.r * 0.55, b.r * 0.09 + 0.3, SHADE, 0.45);
    }
}

const SKIN: (f64, f64, f64) = (0.98, 0.80, 0.66);
const JACKET: (f64, f64, f64) = (0.16, 0.45, 0.86);
const JACKET_DARK: (f64, f64, f64) = (0.11, 0.33, 0.66);
const PANTS: (f64, f64, f64) = (0.20, 0.22, 0.36);
const BOOT: (f64, f64, f64) = (0.40, 0.25, 0.14);
const HAT: (f64, f64, f64) = (0.88, 0.18, 0.22);
const SCARF: (f64, f64, f64) = (0.20, 0.65, 0.35);

fn draw_boy(p: &P, b: &Boy) {
    let f = b.dir;
    let (x, y) = (b.x, b.y);
    let crouch = if b.pose == Pose::Crouch { 7.0 } else { 0.0 };
    let walking = matches!(b.pose, Pose::Walk | Pose::Push);
    let hip = (x, y - 17.0 + crouch);
    let sw = if walking { b.step.sin() * 5.0 } else { 0.0 };
    let lean = if b.pose == Pose::Push { f * 3.0 } else { 0.0 };
    let sh = (x + lean, hip.1 - 12.0);
    let head = (x + lean * 1.3 + f, hip.1 - 20.0);
    let t = b.pt;

    let hands: [(f64, f64); 2] = match b.pose {
        Pose::Walk => [(sh.0 - sw * 0.6 + f * 2.0, sh.1 + 10.0), (sh.0 + sw * 0.6 + f * 2.0, sh.1 + 10.0)],
        Pose::Push => [(sh.0 + f * 10.0, sh.1 + 4.0), (sh.0 + f * 11.0, sh.1 + 6.0)],
        Pose::Lift => [(sh.0 + f * 7.0, sh.1 - 12.0), (sh.0 + f * 9.0, sh.1 - 11.0)],
        Pose::Crouch => [(x + f * 9.0, y - 3.0), (x + f * 11.0, y - 4.0)],
        Pose::Deco => [(sh.0 + f * 3.0, sh.1 + 9.0), (sh.0 + f * 11.0, sh.1 - 3.0 + (t * 9.0).sin() * 2.0)],
        Pose::Wave => [(sh.0 - f * 2.0, sh.1 + 10.0), (sh.0 + (t * 10.0).sin() * 4.0, sh.1 - 13.0)],
    };

    // Achterste arm en been eerst, iets donkerder.
    p.line(sh.0, sh.1, hands[0].0, hands[0].1, JACKET_DARK, 3.2);
    let feet = if crouch > 0.0 { [(x - f * 4.0, y), (x + f * 5.0, y)] } else { [(x - sw, y), (x + sw, y)] };
    for (k, foot) in feet.iter().enumerate() {
        if crouch > 0.0 {
            let knee = (hip.0 + f * 6.0 + k as f64 * 2.0, hip.1 + 2.0);
            p.line(hip.0, hip.1, knee.0, knee.1, PANTS, 4.0);
            p.line(knee.0, knee.1, foot.0, foot.1 - 1.5, PANTS, 4.0);
        } else {
            p.line(hip.0, hip.1, foot.0, foot.1 - 1.5, PANTS, 4.0);
        }
        p.dot(foot.0 + f * 1.5, foot.1 - 1.2, 2.4, BOOT, 1.0);
    }
    p.line(hip.0, hip.1 - 2.0, sh.0, sh.1 + 1.0, JACKET, 11.0);
    p.line(sh.0 - 4.0, sh.1 - 0.5, sh.0 + 4.0, sh.1 - 0.5, SCARF, 3.0);
    p.line(sh.0 - f * 3.0, sh.1, sh.0 - f * 5.0, sh.1 + 7.0, SCARF, 2.4);
    p.dot(head.0, head.1, 6.5, SKIN, 1.0);
    p.dot(head.0 + f * 2.8, head.1 - 0.3, 0.9, COAL, 1.0);
    p.dot(head.0 + f * 4.5, head.1 + 1.6, 1.2, (0.95, 0.55, 0.5), 0.6);
    let hat: Vec<(f64, f64)> =
        (0..=12).map(|k| PI + PI * k as f64 / 12.0).map(|a| (head.0 + 7.2 * a.cos(), head.1 - 0.5 + 7.6 * a.sin())).collect();
    p.poly(&hat, HAT, 1.0);
    p.line(head.0 - 6.8, head.1 - 0.6, head.0 + 6.8, head.1 - 0.6, (0.95, 0.95, 0.95), 2.0);
    p.dot(head.0 - f * 1.5, head.1 - 9.0, 2.4, SNOW, 1.0);

    if let Some(ball) = &b.ball {
        draw_ball(p, ball);
    }
    p.line(sh.0, sh.1, hands[1].0, hands[1].1, JACKET, 3.2);
    p.dot(hands[1].0, hands[1].1, 1.9, HAT, 1.0);
}

fn draw_sweeper(p: &P, s: &Sweeper, gr: &Pile) {
    let f = s.dir;
    let gy = gr.surface_at(s.x);
    let by = gy + s.hop;
    let (sx, sy) = (1.0 + s.squash * 0.5, 1.0 - s.squash);
    let c0 = by - 12.0 * sy;
    let c1 = c0 - 12.0 * sy - 7.5 * sy;
    let c2 = c1 - 9.0 * sy - 5.5 * sy;
    let x = s.x;

    // De opgeveegde berg sneeuw voor de bezem uit.
    let xb = s.broom_x();
    let hr = s.heap_r();
    if hr > 1.0 {
        let hx = xb + f * (hr * 0.9 + 4.0);
        let hy = gr.surface_at(hx) + 1.0;
        let pts: Vec<(f64, f64)> =
            (0..=16).map(|k| PI + PI * k as f64 / 16.0).map(|a| (hx + hr * 1.3 * a.cos(), hy + hr * a.sin())).collect();
        let shadow: Vec<(f64, f64)> = pts.iter().map(|&(a, b)| (a + 0.6, b + 1.0)).collect();
        p.poly(&shadow, SHADE, 0.55);
        p.poly(&pts, SNOW, 1.0);
    }

    // Achterste stokarm.
    p.line(x - f * 7.0 * sx, c1, x - f * 17.0, c1 - 9.0, STICK, 1.5);
    p.line(x - f * 13.0, c1 - 5.5, x - f * 15.0, c1 - 11.0, STICK, 1.1);

    p.snowball(x, c0, 12.0);
    // Platgedrukt bij het landen: teken de bollen als ellipsen.
    if s.squash > 0.02 {
        p.fill(SNOW, 1.0);
        p.ellipse(x, c0, 12.0 * sx, 12.0 * sy);
        p.do_fill();
    }
    p.snowball(x, c1, 9.0);
    p.snowball(x, c2, 7.0);
    for k in 0..3 {
        p.dot(x + f * 1.5, c1 - 5.0 + k as f64 * 4.5, 0.9, COAL, 1.0);
    }
    p.dot(x + f * 1.0, c2 - 1.8, 0.95, COAL, 1.0);
    p.dot(x + f * 4.3, c2 - 1.8, 0.95, COAL, 1.0);
    p.poly(&[(x + f * 3.5, c2 + 0.2), (x + f * 11.0, c2 + 1.6), (x + f * 3.5, c2 + 2.4)], CARROT, 1.0);
    for k in 0..4 {
        p.dot(x + f * (0.5 + k as f64 * 1.3), c2 + 4.0 - (k as f64 - 1.5).abs() * -0.4, 0.5, COAL, 1.0);
    }
    // Hoge hoed.
    let ht = c2 - 6.0;
    p.poly(&[(x - 8.5, ht), (x + 8.5, ht), (x + 8.5, ht - 2.0), (x - 8.5, ht - 2.0)], COAL, 1.0);
    p.poly(&[(x - 5.5, ht - 2.0), (x + 5.5, ht - 2.0), (x + 5.5, ht - 11.0), (x - 5.5, ht - 11.0)], COAL, 1.0);
    p.poly(&[(x - 5.5, ht - 2.0), (x + 5.5, ht - 2.0), (x + 5.5, ht - 4.0), (x - 5.5, ht - 4.0)], HAT, 1.0);

    // Bezem van zijn hand naar de grond.
    let hand = (x + f * 9.0, c1 + 1.0);
    let bgy = gr.surface_at(xb);
    p.line(x + f * 7.0 * sx, c1, hand.0, hand.1, STICK, 1.5);
    p.line(hand.0, hand.1, xb, bgy - 5.0, STICK, 1.8);
    p.poly(&[(xb - 3.5, bgy - 6.0), (xb + 3.5, bgy - 6.0), (xb + 6.5, bgy + 0.5), (xb - 6.5, bgy + 0.5)], (0.85, 0.70, 0.35), 1.0);
    for k in -2..=2 {
        let k = k as f64;
        p.line(xb + k * 1.2, bgy - 5.0, xb + k * 2.4, bgy + 0.3, (0.65, 0.50, 0.22), 0.5);
    }
}
