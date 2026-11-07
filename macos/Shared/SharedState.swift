// Reader for the Now Playing snapshot the app writes (src-tauri/src/platform/shared_state.rs). Shared by the Widget and the
// Screen Saver. Position is extrapolated from the wall clock while `playing`, so the writer never needs a per-tick update.

import AppKit
import Foundation

struct LyricLine: Decodable {
    let startMs: Int
    let text: String
}

struct NowPlayingSnapshot: Decodable {
    let version: Int
    let title: String
    let artist: String
    let playing: Bool
    let positionMs: Int
    let durationMs: Int
    let updatedAtMs: Double
    let artwork: String?
    let lyrics: [LyricLine]

    var isIdle: Bool { title.isEmpty }

    /// A playing snapshot whose song should have ended long ago: the app quit or crashed without writing idle. The writer
    /// only writes on change, so a long song that is still playing is not stale until its own end (plus a minute).
    func isExpired(at date: Date) -> Bool {
        guard playing, durationMs > 0 else { return false }
        let remaining = max(0, durationMs - positionMs)
        return date.timeIntervalSince1970 * 1000 > updatedAtMs + Double(remaining) + 60_000
    }

    func position(at date: Date) -> Int {
        guard playing else { return positionMs }
        let elapsed = max(0, Int(date.timeIntervalSince1970 * 1000 - updatedAtMs))
        let moved = positionMs + elapsed
        return durationMs > 0 ? min(moved, durationMs) : moved
    }

    /// Index of the line being sung at `ms`, or nil before the first line.
    func lineIndex(at ms: Int) -> Int? {
        lyrics.lastIndex { $0.startMs <= ms }
    }
}

enum SharedStore {
    /// Set in each target's Info.plist (`LunoRXAppGroup`) by build.sh; must match the app's `LUNORX_APP_GROUP`.
    private static func group(_ bundle: Bundle) -> String {
        (bundle.object(forInfoDictionaryKey: "LunoRXAppGroup") as? String) ?? "VH66R8S4AU.io.lunorx.shared"
    }

    /// Where the file may be, most specific first. A sandboxed host sees only some of them.
    static func directories(bundle: Bundle) -> [URL] {
        var dirs: [URL] = []
        if let container = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: group(bundle)) {
            dirs.append(container)
        }
        // The Screen Saver host is sandboxed: its own `~/Library` is the container the app writes into.
        dirs.append(URL(fileURLWithPath: NSHomeDirectory()).appendingPathComponent("Library/Application Support/LunoRX"))
        if let pw = getpwuid(getuid()), let home = pw.pointee.pw_dir {
            let real = URL(fileURLWithPath: String(cString: home))
            dirs.append(real.appendingPathComponent("Library/Group Containers/\(group(bundle))"))
            dirs.append(real.appendingPathComponent("Library/Application Support/io.lunorx.desktop/shared"))
        }
        return dirs
    }

    /// The newest readable snapshot and the folder it came from (the cover lives next to it).
    static func load(bundle: Bundle) -> (snapshot: NowPlayingSnapshot, dir: URL)? {
        var best: (NowPlayingSnapshot, URL)?
        for dir in directories(bundle: bundle) {
            guard let data = try? Data(contentsOf: dir.appendingPathComponent("now-playing.json")),
                  let snapshot = try? JSONDecoder().decode(NowPlayingSnapshot.self, from: data),
                  snapshot.version == 1 else { continue }
            // An expired playing snapshot reads as nothing playing.
            let live = snapshot.isExpired(at: .now)
                ? NowPlayingSnapshot(version: 1, title: "", artist: "", playing: false, positionMs: 0, durationMs: 0, updatedAtMs: snapshot.updatedAtMs, artwork: nil, lyrics: [])
                : snapshot
            if best == nil || live.updatedAtMs > best!.0.updatedAtMs { best = (live, dir) }
        }
        return best.map { (snapshot: $0.0, dir: $0.1) }
    }

    static func cover(_ snapshot: NowPlayingSnapshot, in dir: URL) -> NSImage? {
        guard let name = snapshot.artwork else { return nil }
        return NSImage(contentsOf: dir.appendingPathComponent(name))
    }
}
