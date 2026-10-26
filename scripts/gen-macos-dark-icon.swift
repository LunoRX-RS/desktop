// Renders the dark-appearance variant of the app icon from the light master: the light tile becomes a dark tile
// and the dark glyph becomes light ink, keeping the same contrast ratio in both appearances. Usage:
//   swift gen-dark.swift <light-512.png> <out.png>
import AppKit
let src = NSBitmapImageRep(data: try! Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[1])))!
let w = src.pixelsWide, h = src.pixelsHigh
let dst = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: w, pixelsHigh: h,
  bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
  colorSpaceName: .calibratedRGB, bytesPerRow: 4*w, bitsPerPixel: 32)!
NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: dst)
src.draw(in: NSRect(x: 0, y: 0, width: w, height: h))
NSGraphicsContext.restoreGraphicsState()
guard let base = dst.bitmapData else { exit(1) }
// Tile (20,21,24) and ink (230,232,236): the same 15:1 the light icon has, seen from the other side.
let tile: (Double, Double, Double) = (20, 21, 24)
let ink: (Double, Double, Double) = (230, 232, 236)
for i in 0..<(w*h) {
  let p = base + i*4
  let a = Double(p[3]) / 255.0
  let l = 0.2126*Double(p[0]) + 0.7152*Double(p[1]) + 0.0722*Double(p[2])
  let t = max(0, min(1, (l - 127.0) / 102.0))
  func mix(_ x: Double, _ y: Double) -> UInt8 { UInt8((x * (1-t) + y * t).rounded()) }
  p[0] = a > 0.001 ? mix(ink.0, tile.0) : 0
  p[1] = a > 0.001 ? mix(ink.1, tile.1) : 0
  p[2] = a > 0.001 ? mix(ink.2, tile.2) : 0
}
try! dst.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: CommandLine.arguments[2]))
print("wrote \(CommandLine.arguments[2])")
