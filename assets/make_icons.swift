// Maakt het app-icoon (Sneuit.icns-bronnen) en het menubalk-icoontje uit de logo's.
// Gebruik: swift make_icons.swift <assets-map> <uitvoer-map>
import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers

let dir = CommandLine.arguments[1], out = CommandLine.arguments[2]

func load(_ name: String) -> CGImage {
    let src = CGImageSourceCreateWithURL(URL(fileURLWithPath: "\(dir)/\(name)") as CFURL, nil)!
    return CGImageSourceCreateImageAtIndex(src, 0, nil)!
}

func canvas(_ w: Int, _ h: Int) -> CGContext {
    CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: 0,
              space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
}

func save(_ ctx: CGContext, _ path: String) {
    let dest = CGImageDestinationCreateWithURL(URL(fileURLWithPath: path) as CFURL, UTType.png.identifier as CFString, 1, nil)!
    CGImageDestinationAddImage(dest, ctx.makeImage()!, nil)
    CGImageDestinationFinalize(dest)
}

/// Kleinste rechthoek met zichtbare pixels, zonder de dunne randlijntjes aan de zijkant.
func contentBox(_ img: CGImage) -> CGRect {
    let w = img.width, h = img.height
    let ctx = canvas(w, h)
    ctx.draw(img, in: CGRect(x: 0, y: 0, width: w, height: h))
    let px = ctx.data!.assumingMemoryBound(to: UInt8.self)
    let bpr = ctx.bytesPerRow
    var cols = [Int](repeating: 0, count: w), rows = [Int](repeating: 0, count: h)
    for y in 0..<h { for x in 0..<w where px[y * bpr + x * 4 + 3] > 40 { cols[x] += 1; rows[y] += 1 } }
    // Een kolom die bijna over de hele hoogte loopt is een randlijntje, geen sneeuwvlok.
    let xs = (0..<w).filter { cols[$0] > 0 && cols[$0] < h * 9 / 10 }
    let ys = (0..<h).filter { y in rows[y] > 4 }
    return CGRect(x: xs.first!, y: ys.first!, width: xs.last! - xs.first! + 1, height: ys.last! - ys.first! + 1)
}

// App-icoon: witte afgeronde tegel volgens het macOS-raster (824 van 1024) met de vlok erop.
let logo = load("sneuit_logo.png")
let icon = canvas(1024, 1024)
let tile = CGRect(x: 100, y: 100, width: 824, height: 824)
let shape = CGPath(roundedRect: tile, cornerWidth: 185, cornerHeight: 185, transform: nil)
icon.saveGState()
icon.setShadow(offset: CGSize(width: 0, height: -10), blur: 24, color: CGColor(gray: 0, alpha: 0.3))
icon.addPath(shape); icon.setFillColor(CGColor(gray: 1, alpha: 1)); icon.fillPath()
icon.restoreGState()
icon.addPath(shape); icon.clip()
icon.interpolationQuality = .high
icon.draw(logo, in: tile.insetBy(dx: -70, dy: -70))
save(icon, "\(out)/icon_1024.png")

// Menubalk: de doorzichtige vlok, bijgesneden en vierkant.
let dyn = load("sneuit_logo_dynamic.png")
let box = contentBox(dyn)
let side = max(box.width, box.height)
for px in [18, 36] {
    let ctx = canvas(px, px)
    ctx.interpolationQuality = .high
    let s = CGFloat(px) / side
    // CGImage-coördinaten beginnen linksonder; contentBox telde van boven.
    let flippedY = CGFloat(dyn.height) - box.maxY
    ctx.draw(dyn, in: CGRect(x: (side - box.width) / 2 * s - box.minX * s,
                             y: (side - box.height) / 2 * s - flippedY * s,
                             width: CGFloat(dyn.width) * s, height: CGFloat(dyn.height) * s))
    save(ctx, "\(out)/menubar_\(px).png")
}
print("vlok in dynamisch logo: \(box)")
