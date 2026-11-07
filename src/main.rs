//! Sneeuw: een macOS-app die sneeuw laat vallen op je vensters en in je Dock.

mod actors;
mod draw;
mod flakes;
mod snapshot;
mod wins;
mod world;

use std::cell::{Cell, RefCell};
use std::ptr::NonNull;
use std::time::Instant;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{define_class, msg_send, sel, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSColor, NSControlStateValueOff,
    NSControlStateValueOn, NSGraphicsContext, NSMenu, NSMenuItem, NSScreen, NSStatusBar, NSVariableStatusItemLength,
    NSView, NSWindow, NSWindowCollectionBehavior, NSWindowStyleMask,
};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_core_graphics::CGContext;
use objc2_foundation::{ns_string, NSObject, NSRect, NSRunLoop, NSRunLoopCommonModes, NSString, NSTimer};

use world::{Rect, World};

/// Boven de menubalk en uitklapmenu's, zodat de sneeuw altijd zichtbaar is,
/// ook met "Toon achtergrond van menubalk" aan.
const OVERLAY_LEVEL: isize = 102;
const FPS: f64 = 30.0;
const TILE_W: f64 = 128.0;
const TILE_H: f64 = 32.0;
/// Hoe vaak per seconde we vensters checken als er niets beweegt.
const IDLE_POLL: f64 = 8.0;

struct ViewIvars {
    ox: Cell<f64>,
    oy: Cell<f64>,
}

define_class!(
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "SneeuwView"]
    #[ivars = ViewIvars]
    struct SnowView;

    impl SnowView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let Some(gc) = NSGraphicsContext::currentContext() else { return };
            let cg = gc.CGContext();
            let b = self.bounds();
            CGContext::clear_rect(Some(&cg), b);
            STATE.with(|s| {
                if let Ok(s) = s.try_borrow() {
                    if let Some(st) = s.as_ref() {
                        let iv = self.ivars();
                        draw::draw(&cg, &st.world, iv.ox.get(), iv.oy.get(), b.size.width, b.size.height, false);
                    }
                }
            });
        }
    }
);

impl SnowView {
    fn new(mtm: MainThreadMarker, size: CGSize, ox: f64, oy: f64) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(ViewIvars { ox: Cell::new(ox), oy: Cell::new(oy) });
        let frame = CGRect::new(CGPoint::new(0.0, 0.0), size);
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "SneeuwController"]
    struct Controller;

    impl Controller {
        #[unsafe(method(summonSnowman:))]
        fn summon_snowman(&self, _sender: Option<&AnyObject>) {
            with_world(|w| w.summon_sweeper());
        }

        #[unsafe(method(summonBoy:))]
        fn summon_boy(&self, _sender: Option<&AnyObject>) {
            with_world(|w| w.summon_boy());
        }

        #[unsafe(method(toggleStorm:))]
        fn toggle_storm(&self, sender: Option<&NSMenuItem>) {
            with_world(|w| {
                let storm = w.speed <= 1.0;
                (w.speed, w.flake_mult) = if storm { (12.0, 3.0) } else { (1.0, 1.0) };
                if let Some(item) = sender {
                    item.setState(if storm { NSControlStateValueOn } else { NSControlStateValueOff });
                }
            });
        }

        #[unsafe(method(clearSnow:))]
        fn clear_snow(&self, _sender: Option<&AnyObject>) {
            with_world(|w| w.clear_all());
        }
    }
);

impl Controller {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(());
        unsafe { msg_send![super(this), init] }
    }
}

struct Overlay {
    window: Retained<NSWindow>,
    view: Retained<SnowView>,
    flakes: flakes::FlakeLayers,
    rect: Rect,
}

struct State {
    world: World,
    overlays: Vec<Overlay>,
    last: Instant,
    screen_check: f64,
    poll_acc: f64,
    active: f64,
    pid: i32,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

fn with_world(f: impl FnOnce(&mut World)) {
    STATE.with(|s| {
        if let Some(st) = s.borrow_mut().as_mut() {
            f(&mut st.world);
        }
    });
}

/// Schermen in CoreGraphics-coördinaten (linksboven van het hoofdscherm, y omlaag).
fn screens(mtm: MainThreadMarker) -> Vec<(Rect, NSRect)> {
    let list = NSScreen::screens(mtm);
    let main_h = list.firstObject().map(|s| s.frame().size.height).unwrap_or(0.0);
    list.iter()
        .map(|s| {
            let f = s.frame();
            (Rect::new(f.origin.x, main_h - (f.origin.y + f.size.height), f.size.width, f.size.height), f)
        })
        .collect()
}

fn make_overlay(mtm: MainThreadMarker, rect: Rect, frame: NSRect) -> Overlay {
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            mtm.alloc(),
            frame,
            NSWindowStyleMask::Borderless,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    unsafe { window.setReleasedWhenClosed(false) };
    window.setOpaque(false);
    window.setBackgroundColor(Some(&NSColor::clearColor()));
    window.setHasShadow(false);
    window.setIgnoresMouseEvents(true);
    window.setLevel(OVERLAY_LEVEL);
    window.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::Stationary
            | NSWindowCollectionBehavior::IgnoresCycle
            | NSWindowCollectionBehavior::FullScreenAuxiliary,
    );
    // Onder: getekende hoopjes en figuurtjes. Boven: vlokken als losse laagjes.
    let local = CGRect::new(CGPoint::new(0.0, 0.0), frame.size);
    let container = NSView::initWithFrame(mtm.alloc(), local);
    container.setWantsLayer(true);
    let view = SnowView::new(mtm, frame.size, rect.x, rect.y);
    view.setWantsLayer(true);
    container.addSubview(&view);
    let flakes = flakes::FlakeLayers::new(rect.w, rect.h);
    let flake_view = NSView::initWithFrame(mtm.alloc(), local);
    flake_view.setLayer(Some(&flakes.root));
    flake_view.setWantsLayer(true);
    container.addSubview(&flake_view);
    window.setContentView(Some(&container));
    window.setFrame_display(frame, false);
    window.orderFrontRegardless();
    Overlay { window, view, flakes, rect }
}

fn build_overlays(mtm: MainThreadMarker) -> (Vec<Overlay>, Vec<Rect>) {
    let sc = screens(mtm);
    let overlays = sc.iter().map(|&(r, f)| make_overlay(mtm, r, f)).collect();
    (overlays, sc.into_iter().map(|(r, _)| r).collect())
}

fn tick(mtm: MainThreadMarker) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let Some(st) = s.as_mut() else { return };
        let now = Instant::now();
        let dt = (now - st.last).as_secs_f64();
        st.last = now;

        st.screen_check += dt;
        if st.screen_check > 2.0 {
            st.screen_check = 0.0;
            let current: Vec<Rect> = screens(mtm).into_iter().map(|(r, _)| r).collect();
            if current != st.world.screens {
                for o in st.overlays.drain(..) {
                    o.window.orderOut(None);
                }
                let (overlays, rects) = build_overlays(mtm);
                st.overlays = overlays;
                st.world.set_screens(rects);
            }
        }

        let pid = st.pid;
        // De vensterlijst opvragen is relatief duur: snel als er iets beweegt, anders rustig.
        st.poll_acc += dt;
        st.active -= dt;
        let wins = if st.active > 0.0 || st.poll_acc >= 1.0 / IDLE_POLL {
            st.poll_acc = 0.0;
            Some(wins::onscreen(pid))
        } else {
            None
        };
        if st.world.step(dt, wins, &|| wins::all_ids(pid)) {
            st.active = 1.5;
        }

        // Alleen de stukjes die veranderd zijn opnieuw tekenen; dat scheelt veel energie.
        // Honderden losse rechthoekjes maken het clippen juist duur, dus voeg ze samen tot tegels.
        let dirty = std::mem::take(&mut st.world.dirty);
        let parts = &st.world.particles;
        for o in &mut st.overlays {
            let r = o.rect;
            o.flakes.update(parts, r.x, r.y, r.w, r.h);
            let cols = (r.w / TILE_W).ceil() as usize;
            let rows = (r.h / TILE_H).ceil() as usize;
            let mut grid = vec![false; cols * rows];
            for d in &dirty {
                if !d.intersects(&r) {
                    continue;
                }
                let c0 = (((d.x - r.x) / TILE_W).floor().max(0.0) as usize).min(cols - 1);
                let c1 = (((d.x + d.w - r.x) / TILE_W).floor().max(0.0) as usize).min(cols - 1);
                let r0 = (((d.y - r.y) / TILE_H).floor().max(0.0) as usize).min(rows - 1);
                let r1 = (((d.y + d.h - r.y) / TILE_H).floor().max(0.0) as usize).min(rows - 1);
                for row in r0..=r1 {
                    grid[row * cols + c0..=row * cols + c1].iter_mut().for_each(|g| *g = true);
                }
            }
            for row in 0..rows {
                let mut c = 0;
                while c < cols {
                    if !grid[row * cols + c] {
                        c += 1;
                        continue;
                    }
                    let start = c;
                    while c < cols && grid[row * cols + c] {
                        c += 1;
                    }
                    let local = CGRect::new(
                        CGPoint::new(start as f64 * TILE_W, row as f64 * TILE_H),
                        CGSize::new((c - start) as f64 * TILE_W, TILE_H),
                    );
                    o.view.setNeedsDisplayInRect(local);
                }
            }
        }
    });
}

fn add_item(menu: &NSMenu, mtm: MainThreadMarker, title: &str, action: objc2::runtime::Sel, target: &Controller) {
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(mtm.alloc(), &NSString::from_str(title), Some(action), ns_string!(""))
    };
    unsafe { item.setTarget(Some(target)) };
    menu.addItem(&item);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--snapshot") {
        snapshot::run(args.get(2).map(String::as_str).unwrap_or("."));
        return;
    }
    let mtm = MainThreadMarker::new().expect("moet op de hoofdthread draaien");
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    let (overlays, rects) = build_overlays(mtm);
    let mut world = World::new(rects);
    if let Some(v) = std::env::var("SNEEUW_SPEED").ok().and_then(|v| v.parse::<f64>().ok()) {
        world.speed = v.max(0.1);
        world.flake_mult = v.sqrt().clamp(1.0, 4.0);
    }
    STATE.with(|s| {
        *s.borrow_mut() = Some(State {
            world,
            overlays,
            last: Instant::now(),
            screen_check: 0.0,
            poll_acc: 1.0,
            active: 0.0,
            pid: std::process::id() as i32,
        })
    });

    // Sneeuwvlokje in de menubalk met wat knoppen.
    let controller = Controller::new(mtm);
    let item = NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength);
    if let Some(button) = item.button(mtm) {
        button.setTitle(ns_string!("❄"));
    }
    let menu = NSMenu::new(mtm);
    add_item(&menu, mtm, "Sneeuwman laten opruimen", sel!(summonSnowman:), &controller);
    add_item(&menu, mtm, "Jongetje een sneeuwpop laten maken", sel!(summonBoy:), &controller);
    add_item(&menu, mtm, "Sneeuwstorm", sel!(toggleStorm:), &controller);
    add_item(&menu, mtm, "Alle sneeuw weghalen", sel!(clearSnow:), &controller);
    menu.addItem(&NSMenuItem::separatorItem(mtm));
    let quit = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(mtm.alloc(), ns_string!("Stop Sneeuw"), Some(sel!(terminate:)), ns_string!("q"))
    };
    menu.addItem(&quit);
    item.setMenu(Some(&menu));
    std::mem::forget(item);
    std::mem::forget(controller);

    let block = RcBlock::new(move |_t: NonNull<NSTimer>| tick(mtm));
    let timer = unsafe { NSTimer::timerWithTimeInterval_repeats_block(1.0 / FPS, true, &block) };
    unsafe { NSRunLoop::currentRunLoop().addTimer_forMode(&timer, NSRunLoopCommonModes) };

    app.run();
}
