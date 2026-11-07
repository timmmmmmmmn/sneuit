//! Vallende vlokken en klonten als losse Core Animation-laagjes. De GPU schuift ze rond,
//! zodat de CPU niet elke frame het hele scherm opnieuw hoeft te tekenen.

use objc2::rc::Retained;
use objc2_core_foundation::{CFRetained, CGPoint, CGRect, CGSize};
use objc2_core_graphics::CGColor;
use objc2_quartz_core::{CALayer, CATransaction};

use crate::world::Particle;

struct Dot {
    layer: Retained<CALayer>,
    r: f64,
    hidden: bool,
}

pub struct FlakeLayers {
    pub root: Retained<CALayer>,
    pool: Vec<Dot>,
    fill: CFRetained<CGColor>,
    edge: CFRetained<CGColor>,
}

impl FlakeLayers {
    pub fn new(w: f64, h: f64) -> Self {
        let root = CALayer::new();
        root.setBounds(CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(w, h)));
        FlakeLayers {
            root,
            pool: Vec::new(),
            fill: CGColor::new_srgb(0.97, 0.98, 1.0, 0.92),
            edge: CGColor::new_srgb(0.40, 0.50, 0.65, 0.30),
        }
    }

    /// Zet een laagje op elk deeltje binnen dit scherm (oorsprong `ox`,`oy`, grootte `w`×`h`).
    pub fn update(&mut self, parts: &[Particle], ox: f64, oy: f64, w: f64, h: f64) {
        CATransaction::begin();
        CATransaction::setDisableActions(true);
        let mut k = 0;
        for p in parts {
            let (x, y) = (p.x - ox, p.y - oy);
            if x < -8.0 || x > w + 8.0 || y < -8.0 || y > h + 8.0 {
                continue;
            }
            if k == self.pool.len() {
                let layer = CALayer::new();
                layer.setBackgroundColor(Some(&self.fill));
                layer.setBorderColor(Some(&self.edge));
                layer.setBorderWidth(0.6);
                self.root.addSublayer(&layer);
                self.pool.push(Dot { layer, r: -1.0, hidden: false });
            }
            let dot = &mut self.pool[k];
            let r = ((p.r + 0.6) * 4.0).round() / 4.0;
            if r != dot.r {
                dot.layer.setBounds(CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(2.0 * r, 2.0 * r)));
                dot.layer.setCornerRadius(r);
                dot.r = r;
            }
            // Laagjes hebben hun oorsprong linksonder.
            dot.layer.setPosition(CGPoint::new(x, h - y));
            if dot.hidden {
                dot.layer.setHidden(false);
                dot.hidden = false;
            }
            k += 1;
        }
        for dot in &mut self.pool[k..] {
            if !dot.hidden {
                dot.layer.setHidden(true);
                dot.hidden = true;
            }
        }
        CATransaction::commit();
    }
}
