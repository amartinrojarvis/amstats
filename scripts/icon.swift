// Build-time vector icon. Swift is not part of the application or its runtime.
import AppKit
let fm = FileManager.default
let directory = "target/AppIcon.iconset"
try fm.createDirectory(atPath: directory, withIntermediateDirectories: true)
for size in [16, 32, 128, 256, 512] {
    for scale in [1, 2] {
        let pixels = size * scale
        let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: pixels, pixelsHigh: pixels, bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
        let context = NSGraphicsContext.current!.cgContext
        context.scaleBy(x: CGFloat(pixels)/512, y: CGFloat(pixels)/512)
        let background = NSBezierPath(roundedRect: NSRect(x: 28, y: 28, width: 456, height: 456), xRadius: 108, yRadius: 108)
        NSColor(srgbRed: 0.06, green: 0.12, blue: 0.20, alpha: 1).setFill()
        background.fill()
        for (i, height) in [110.0, 230.0, 170.0, 290.0].enumerated() {
            let bar = NSBezierPath(roundedRect: NSRect(x: 108 + Double(i)*78, y: 112, width: 44, height: height), xRadius: 22, yRadius: 22)
            NSColor(srgbRed: 0.25 + Double(i)*0.04, green: 0.87 - Double(i)*0.03, blue: 0.70 + Double(i)*0.07, alpha: 1).setFill()
            bar.fill()
        }
        NSGraphicsContext.restoreGraphicsState()
        let suffix = scale == 2 ? "@2x" : ""
        try rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: "\(directory)/icon_\(size)x\(size)\(suffix).png"))
    }
}
let task = Process()
task.executableURL = URL(fileURLWithPath: "/usr/bin/iconutil")
task.arguments = ["-c", "icns", directory, "-o", "resources/AppIcon.icns"]
try task.run(); task.waitUntilExit()
if task.terminationStatus != 0 { exit(task.terminationStatus) }
