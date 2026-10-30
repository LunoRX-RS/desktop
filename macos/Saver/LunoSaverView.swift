// LUNOR-X screen saver: the blurred cover as backdrop, the cover and song on the left, the lyric line being sung large beside
// it. Plain AppKit drawing, no web view. Reads the snapshot once a second and extrapolates the position between reads.
// Idle (nothing playing): the name drifts slowly across the screen so no pixel stays lit.

import AppKit
import CoreImage
import ScreenSaver

@objc(LunoSaverView)
final class LunoSaverView: ScreenSaverView {
    private var loaded: (snapshot: NowPlayingSnapshot, dir: URL)?
    private var cover: NSImage?
    private var coverKey = ""
    private var soft: NSImage?
    private var lastRead = Date.distantPast

    override init?(frame: NSRect, isPreview: Bool) {
        super.init(frame: frame, isPreview: isPreview)
        animationTimeInterval = 1.0 / 30.0
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
        animationTimeInterval = 1.0 / 30.0
    }

    override func animateOneFrame() {
        if Date().timeIntervalSince(lastRead) >= 1 { reload() }
        setNeedsDisplay(bounds)
    }

    private func reload() {
        lastRead = Date()
        loaded = SharedStore.load(bundle: Bundle(for: LunoSaverView.self))
        guard let s = loaded?.snapshot, !s.isIdle else { cover = nil; soft = nil; coverKey = ""; return }
        let key = "\(s.title)|\(s.artist)|\(s.artwork ?? "")|\(s.updatedAtMs)"
        guard key != coverKey || cover == nil, let dir = loaded?.dir else { return }
        coverKey = key
        cover = SharedStore.cover(s, in: dir)
        soft = cover.map(Self.softened)
    }

    /// A 160px copy blurred once per cover (not per frame); drawn scaled up it is the backdrop.
    private static func softened(_ image: NSImage) -> NSImage {
        let side: CGFloat = 160
        let small = NSImage(size: NSSize(width: side, height: side))
        small.lockFocus()
        NSGraphicsContext.current?.imageInterpolation = .high
        image.draw(in: NSRect(x: 0, y: 0, width: side, height: side), from: .zero, operation: .copy, fraction: 1)
        small.unlockFocus()
        guard let tiff = small.tiffRepresentation, let input = CIImage(data: tiff), let blur = CIFilter(name: "CIGaussianBlur") else { return small }
        blur.setValue(input.clampedToExtent(), forKey: kCIInputImageKey)
        blur.setValue(12, forKey: kCIInputRadiusKey)
        guard let output = blur.outputImage?.cropped(to: input.extent) else { return small }
        let rep = NSCIImageRep(ciImage: output)
        let result = NSImage(size: rep.size)
        result.addRepresentation(rep)
        return result
    }

    override func draw(_ rect: NSRect) {
        NSColor.black.setFill()
        bounds.fill()
        guard let s = loaded?.snapshot, !s.isIdle else { return drawIdle() }
        NSGraphicsContext.current?.imageInterpolation = .high
        if let soft {
            soft.draw(in: bounds.insetBy(dx: -bounds.width * 0.1, dy: -bounds.height * 0.1), from: .zero, operation: .sourceOver, fraction: 0.75)
        }
        NSColor.black.withAlphaComponent(0.55).setFill()
        bounds.fill()

        let unit = min(bounds.width, bounds.height) / 100
        let pad = unit * 8
        let side = unit * 34
        let coverRect = NSRect(x: pad, y: bounds.midY - side / 2 + unit * 6, width: side, height: side)
        if let cover {
            NSGraphicsContext.saveGraphicsState()
            NSBezierPath(roundedRect: coverRect, xRadius: unit * 2.2, yRadius: unit * 2.2).addClip()
            cover.draw(in: coverRect, from: .zero, operation: .sourceOver, fraction: 1)
            NSGraphicsContext.restoreGraphicsState()
        }
        text(s.title, at: NSPoint(x: pad, y: coverRect.minY - unit * 5.5), width: side, size: unit * 2.6, weight: .bold, alpha: 1)
        text(s.artist, at: NSPoint(x: pad, y: coverRect.minY - unit * 9), width: side, size: unit * 2, weight: .regular, alpha: 0.6)

        let lyricX = pad + side + unit * 8
        let lyricWidth = bounds.width - lyricX - pad
        let now = s.position(at: Date())
        guard let i = s.lineIndex(at: now) else { return }
        let rows: [(Int, CGFloat, CGFloat)] = [(i - 1, unit * 3.4, 0.28), (i, unit * 5.2, 1), (i + 1, unit * 3.4, 0.4), (i + 2, unit * 3.4, 0.22)]
        var y = bounds.midY + unit * 14
        for (index, size, alpha) in rows where index >= 0 && index < s.lyrics.count {
            let height = text(s.lyrics[index].text, at: NSPoint(x: lyricX, y: y), width: lyricWidth, size: size, weight: .bold, alpha: alpha, measureOnly: false)
            y -= height + unit * 2.4
        }
    }

    private func drawIdle() {
        let unit = min(bounds.width, bounds.height) / 100
        // One slow lap every five minutes keeps the text off any fixed pixels.
        let t = Date().timeIntervalSinceReferenceDate / 300
        let x = (sin(t * 2 * .pi) * 0.5 + 0.5) * (bounds.width - unit * 40)
        let y = (cos(t * 2 * .pi * 0.7) * 0.5 + 0.5) * (bounds.height - unit * 12) + unit * 4
        text("LUNOR-X", at: NSPoint(x: x, y: y + unit * 4), width: unit * 40, size: unit * 4.2, weight: .bold, alpha: 0.55)
        text("Nothing playing", at: NSPoint(x: x, y: y), width: unit * 40, size: unit * 2, weight: .regular, alpha: 0.3)
    }

    @discardableResult
    private func text(_ string: String, at point: NSPoint, width: CGFloat, size: CGFloat, weight: NSFont.Weight, alpha: CGFloat, measureOnly: Bool = false) -> CGFloat {
        let style = NSMutableParagraphStyle()
        style.lineBreakMode = .byWordWrapping
        let attrs: [NSAttributedString.Key: Any] = [
            .font: NSFont.systemFont(ofSize: size, weight: weight),
            .foregroundColor: NSColor.white.withAlphaComponent(alpha),
            .paragraphStyle: style,
        ]
        let attributed = NSAttributedString(string: string, attributes: attrs)
        let box = attributed.boundingRect(with: NSSize(width: width, height: .greatestFiniteMagnitude), options: [.usesLineFragmentOrigin])
        if !measureOnly {
            attributed.draw(with: NSRect(x: point.x, y: point.y - box.height, width: width, height: box.height), options: [.usesLineFragmentOrigin])
        }
        return box.height
    }
}
