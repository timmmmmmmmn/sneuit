//! `sneeuw --snapshot <map>`: simuleert een nepscherm met nepvensters en schrijft een paar
//! frames als PPM-plaatjes weg. Handig om de tekeningen te controleren zonder schermopname.

use std::collections::HashSet;
use std::io::Write;

use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_core_graphics::{CGBitmapContextCreate, CGBitmapContextGetData, CGColorSpace, CGContext, CGImageAlphaInfo};

use crate::wins::WinInfo;
use crate::world::{Rect, World};

const W: usize = 900;
const H: usize = 520;
const SCALE: usize = 2;

pub fn run(dir: &str) {
    let screen = Rect::new(0.0, 0.0, W as f64, H as f64);
    let mut world = World::new(vec![screen]);
    world.speed = 60.0;
    world.flake_mult = 4.0;
    world.intensity = 1.0;
    let mut wins = vec![
        WinInfo { id: 1, x: 120.0, y: 140.0, w: 360.0, h: 260.0 },
        WinInfo { id: 2, x: 420.0, y: 230.0, w: 380.0, h: 200.0 },
    ];
    let ids: HashSet<u32> = [1, 2].into();
    let dt = 1.0 / 30.0;
    let mut frame = 0;
    let shot = |world: &World, wins: &[WinInfo], name: &str| {
        save(world, wins, &format!("{dir}/{name}.ppm"));
    };
    // Een "uur" sneeuw in een minuut gesimuleerde tijd.
    for _ in 0..(60 * 30) {
        world.step(dt, Some(wins.clone()), &|| ids.clone());
        frame += 1;
    }
    shot(&world, &wins, "1_na_een_uur");
    save_layers(&world, &format!("{dir}/1_laagjes.ppm"));
    // Venster smaller maken: de sneeuw buiten de rand valt eraf.
    wins[0].w = 250.0;
    for _ in 0..8 {
        world.step(dt, Some(wins.clone()), &|| ids.clone());
    }
    shot(&world, &wins, "2_resize_vallend");
    world.speed = 1.0;
    world.flake_mult = 1.0;
    world.summon_boy();
    let marks = [12.0, 24.0, 32.0, 40.0];
    let mut t = 0.0;
    let mut k = 0;
    while world.actors.boy.is_some() && t < 60.0 {
        world.step(dt, Some(wins.clone()), &|| ids.clone());
        t += dt;
        if k < marks.len() && t >= marks[k] {
            shot(&world, &wins, &format!("3_jongetje_{k}"));
            k += 1;
        }
    }
    shot(&world, &wins, "4_sneeuwpop");
    world.summon_sweeper();
    for i in 0..(30 * 20) {
        world.step(dt, Some(wins.clone()), &|| ids.clone());
        if i == 150 || i == 300 {
            shot(&world, &wins, &format!("5_veger_{i}"));
        }
    }
    eprintln!("{frame} frames gesimuleerd");
}

fn save(world: &World, wins: &[WinInfo], path: &str) {
    let (pw, ph) = (W * SCALE, H * SCALE);
    let space = CGColorSpace::new_device_rgb().unwrap();
    let ctx = unsafe {
        CGBitmapContextCreate(std::ptr::null_mut(), pw, ph, 8, pw * 4, Some(&space), CGImageAlphaInfo::PremultipliedLast.0)
    }
    .unwrap();
    let c = Some(&*ctx);
    CGContext::translate_ctm(c, 0.0, ph as f64);
    CGContext::scale_ctm(c, SCALE as f64, -(SCALE as f64));
    let rect = |x: f64, y: f64, w: f64, h: f64| CGRect::new(CGPoint::new(x, y), CGSize::new(w, h));
    CGContext::set_rgb_fill_color(c, 0.28, 0.34, 0.45, 1.0);
    CGContext::fill_rect(c, rect(0.0, 0.0, W as f64, H as f64));
    for (k, w) in wins.iter().enumerate().rev() {
        let shade = if k == 0 { 0.96 } else { 0.86 };
        CGContext::set_rgb_fill_color(c, shade, shade, shade, 1.0);
        CGContext::fill_rect(c, rect(w.x, w.y, w.w, w.h));
        CGContext::set_rgb_fill_color(c, 0.8, 0.8, 0.82, 1.0);
        CGContext::fill_rect(c, rect(w.x, w.y, w.w, 26.0));
    }
    crate::draw::draw(&ctx, world, 0.0, 0.0, W as f64, H as f64, true);
    let data = CGBitmapContextGetData(c) as *const u8;
    let px = unsafe { std::slice::from_raw_parts(data, pw * ph * 4) };
    let mut f = std::fs::File::create(path).unwrap();
    write!(f, "P6\n{pw} {ph}\n255\n").unwrap();
    let rgb: Vec<u8> = px.chunks(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
    f.write_all(&rgb).unwrap();
}

/// Controle van de laagjes-vlokken: zelfde deeltjes, maar via Core Animation gerenderd in
/// een niet-geflipte context (zoals op het scherm). Moet op dezelfde plekken staan als in `save`.
pub fn save_layers(world: &World, path: &str) {
    let (pw, ph) = (W * SCALE, H * SCALE);
    let space = CGColorSpace::new_device_rgb().unwrap();
    let ctx = unsafe {
        CGBitmapContextCreate(std::ptr::null_mut(), pw, ph, 8, pw * 4, Some(&space), CGImageAlphaInfo::PremultipliedLast.0)
    }
    .unwrap();
    let c = Some(&*ctx);
    CGContext::scale_ctm(c, SCALE as f64, SCALE as f64);
    CGContext::set_rgb_fill_color(c, 0.28, 0.34, 0.45, 1.0);
    CGContext::fill_rect(c, CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(W as f64, H as f64)));
    let mut fl = crate::flakes::FlakeLayers::new(W as f64, H as f64);
    fl.update(&world.particles, 0.0, 0.0, W as f64, H as f64);
    fl.root.setPosition(CGPoint::new(W as f64 / 2.0, H as f64 / 2.0));
    fl.root.renderInContext(&ctx);
    let data = CGBitmapContextGetData(c) as *const u8;
    let px = unsafe { std::slice::from_raw_parts(data, pw * ph * 4) };
    let mut f = std::fs::File::create(path).unwrap();
    write!(f, "P6\n{pw} {ph}\n255\n").unwrap();
    // Bitmapgeheugen begint bovenaan, net als PPM.
    let rgb: Vec<u8> = px.chunks(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
    f.write_all(&rgb).unwrap();
}
