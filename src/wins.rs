//! Leest de posities van alle zichtbare vensters van andere apps uit via CoreGraphics.
//! Hiervoor is geen schermopname-toestemming nodig: alleen bounds, laag en eigenaar.

use std::collections::HashSet;
use std::ffi::c_void;

use objc2_core_foundation::{CFDictionary, CFNumber, CFString, CGRect};
use objc2_core_graphics::{
    kCGNullWindowID, kCGWindowAlpha, kCGWindowBounds, kCGWindowLayer, kCGWindowNumber,
    kCGWindowOwnerPID, CGRectMakeWithDictionaryRepresentation, CGWindowListCopyWindowInfo,
    CGWindowListOption,
};

#[derive(Clone, Copy, Debug)]
pub struct WinInfo {
    pub id: u32,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

fn value<'a>(d: &'a CFDictionary, key: &CFString) -> *const c_void {
    unsafe { d.value(key as *const CFString as *const c_void) }
}

fn int(d: &CFDictionary, key: &CFString) -> Option<i64> {
    let v = value(d, key) as *const CFNumber;
    if v.is_null() { None } else { unsafe { &*v }.as_i64() }
}

fn float(d: &CFDictionary, key: &CFString) -> Option<f64> {
    let v = value(d, key) as *const CFNumber;
    if v.is_null() { None } else { unsafe { &*v }.as_f64() }
}

fn bounds(d: &CFDictionary) -> Option<CGRect> {
    let v = value(d, unsafe { kCGWindowBounds }) as *const CFDictionary;
    if v.is_null() {
        return None;
    }
    let mut r = CGRect::default();
    unsafe { CGRectMakeWithDictionaryRepresentation(Some(&*v), &mut r) }.then_some(r)
}

/// Gewone vensters op het huidige scherm/Space, van voor naar achter.
pub fn onscreen(own_pid: i32) -> Vec<WinInfo> {
    let opt = CGWindowListOption::OptionOnScreenOnly | CGWindowListOption::ExcludeDesktopElements;
    let Some(arr) = CGWindowListCopyWindowInfo(opt, kCGNullWindowID) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for i in 0..arr.count() {
        let d = unsafe { &*(arr.value_at_index(i) as *const CFDictionary) };
        let (Some(id), Some(pid), Some(layer)) = (
            int(d, unsafe { kCGWindowNumber }),
            int(d, unsafe { kCGWindowOwnerPID }),
            int(d, unsafe { kCGWindowLayer }),
        ) else {
            continue;
        };
        if layer != 0 || pid == own_pid as i64 {
            continue;
        }
        if float(d, unsafe { kCGWindowAlpha }).unwrap_or(1.0) < 0.1 {
            continue;
        }
        let Some(r) = bounds(d) else { continue };
        if r.size.width < 60.0 || r.size.height < 40.0 {
            continue;
        }
        out.push(WinInfo { id: id as u32, x: r.origin.x, y: r.origin.y, w: r.size.width, h: r.size.height });
    }
    out
}

/// Alle vensters die nog bestaan (ook geminimaliseerd of op een andere Space).
pub fn all_ids(own_pid: i32) -> HashSet<u32> {
    let Some(arr) = CGWindowListCopyWindowInfo(CGWindowListOption::OptionAll, kCGNullWindowID) else {
        return HashSet::new();
    };
    let mut out = HashSet::new();
    for i in 0..arr.count() {
        let d = unsafe { &*(arr.value_at_index(i) as *const CFDictionary) };
        if int(d, unsafe { kCGWindowOwnerPID }) == Some(own_pid as i64) {
            continue;
        }
        if let Some(id) = int(d, unsafe { kCGWindowNumber }) {
            out.insert(id as u32);
        }
    }
    out
}
