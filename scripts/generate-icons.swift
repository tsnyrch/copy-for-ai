import AppKit

let root = URL(fileURLWithPath: CommandLine.arguments[1])
let assets = root.appendingPathComponent("crates/desktop/assets")
let iconset = root.appendingPathComponent("target/CopyAI.iconset")
try FileManager.default.createDirectory(at: iconset, withIntermediateDirectories: true)

func path(_ points: [NSPoint], close: Bool = false) -> NSBezierPath {
    let result = NSBezierPath()
    result.move(to: points[0])
    for point in points.dropFirst() { result.line(to: point) }
    if close { result.close() }
    return result
}

func render(size: Int, template: Bool) -> Data {
    let bitmap = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: size, pixelsHigh: size,
        bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
        colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: bitmap)
    let transform = AffineTransform(m11: CGFloat(size) / 128, m12: 0, m21: 0,
        m22: -CGFloat(size) / 128, tX: 0, tY: CGFloat(size))
    (transform as NSAffineTransform).concat()
    let green = NSColor(srgbRed: 23/255, green: 107/255, blue: 87/255, alpha: 1)
    let pale = NSColor(srgbRed: 182/255, green: 230/255, blue: 211/255, alpha: 1)
    if !template {
        green.setFill()
        NSBezierPath(roundedRect: NSRect(x: 4, y: 4, width: 120, height: 120), xRadius: 28, yRadius: 28).fill()
    }
    let back = NSBezierPath(roundedRect: NSRect(x: 30, y: 25, width: 48, height: 64), xRadius: 8, yRadius: 8)
    (template ? NSColor.black : pale).setStroke()
    back.lineWidth = 6
    back.stroke()
    let front = NSBezierPath(roundedRect: NSRect(x: 44, y: 38, width: 48, height: 64), xRadius: 8, yRadius: 8)
    if !template { green.setFill(); front.fill() }
    (template ? NSColor.black : NSColor.white).setStroke()
    front.lineWidth = 6
    front.stroke()
    let shield = path([NSPoint(x: 68, y: 53), NSPoint(x: 83, y: 59), NSPoint(x: 83, y: 71)])
    shield.curve(to: NSPoint(x: 68, y: 92), controlPoint1: NSPoint(x: 83, y: 81), controlPoint2: NSPoint(x: 76, y: 88))
    shield.curve(to: NSPoint(x: 53, y: 71), controlPoint1: NSPoint(x: 60, y: 88), controlPoint2: NSPoint(x: 53, y: 81))
    shield.line(to: NSPoint(x: 53, y: 59)); shield.close()
    (template ? NSColor.black : NSColor.white).setFill(); shield.fill()
    let check = path([NSPoint(x: 61, y: 71), NSPoint(x: 66, y: 76), NSPoint(x: 75, y: 65)])
    if template { NSGraphicsContext.current?.compositingOperation = .clear }
    else { green.setStroke() }
    check.lineWidth = 5; check.lineCapStyle = .round; check.lineJoinStyle = .round; check.stroke()
    NSGraphicsContext.restoreGraphicsState()
    return bitmap.representation(using: .png, properties: [:])!
}

try render(size: 1024, template: false).write(to: assets.appendingPathComponent("logo.png"))
try render(size: 72, template: true).write(to: assets.appendingPathComponent("menu-bar.png"))
for size in [16, 32, 128, 256, 512] {
    try render(size: size, template: false).write(to: iconset.appendingPathComponent("icon_\(size)x\(size).png"))
    try render(size: size * 2, template: false).write(to: iconset.appendingPathComponent("icon_\(size)x\(size)@2x.png"))
}

func bigEndian(_ value: Int) -> Data {
    var number = UInt32(value).bigEndian
    return withUnsafeBytes(of: &number) { Data($0) }
}

var entries = Data()
for (type, size) in [("icp4", 16), ("icp5", 32), ("icp6", 64), ("ic07", 128),
                     ("ic08", 256), ("ic09", 512), ("ic10", 1024)] {
    let png = render(size: size, template: false)
    entries.append(Data(type.utf8))
    entries.append(bigEndian(png.count + 8))
    entries.append(png)
}
var icns = Data("icns".utf8)
icns.append(bigEndian(entries.count + 8))
icns.append(entries)
try icns.write(to: assets.appendingPathComponent("CopyAI.icns"))
