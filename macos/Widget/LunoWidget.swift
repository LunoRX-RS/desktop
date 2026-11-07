// LUNOR-X Now Playing widget: cover, title, artist and the lyric line being sung. Display only (a click opens the app).
// The timeline holds one entry per lyric line, so the line advances on its own without the app asking WidgetKit to reload.
// WidgetKit still decides when to rebuild the timeline (a new song waits for its next refresh), see macos/build.sh.

import SwiftUI
import WidgetKit

struct NowEntry: TimelineEntry {
    let date: Date
    let snapshot: NowPlayingSnapshot?
    let cover: NSImage?
}

struct NowProvider: TimelineProvider {
    private let bundle = Bundle.main

    func placeholder(in context: Context) -> NowEntry { NowEntry(date: .now, snapshot: nil, cover: nil) }

    func getSnapshot(in context: Context, completion: @escaping (NowEntry) -> Void) { completion(entry(at: .now)) }

    func getTimeline(in context: Context, completion: @escaping (Timeline<NowEntry>) -> Void) {
        guard let loaded = SharedStore.load(bundle: bundle), !loaded.snapshot.isIdle else {
            completion(Timeline(entries: [NowEntry(date: .now, snapshot: nil, cover: nil)], policy: .after(.now + 300)))
            return
        }
        let s = loaded.snapshot
        let cover = SharedStore.cover(s, in: loaded.dir)
        var entries = [NowEntry(date: .now, snapshot: s, cover: cover)]
        if s.playing {
            // One entry at the start of every line still to come, for the next 20 minutes.
            let nowMs = s.position(at: .now)
            let horizon = nowMs + 20 * 60_000
            let upcoming = s.lyrics.filter { $0.startMs > nowMs && $0.startMs <= min(horizon, s.durationMs > 0 ? s.durationMs : horizon) }
            for line in upcoming.prefix(200) {
                let when = Date(timeIntervalSinceNow: Double(line.startMs - nowMs) / 1000)
                entries.append(NowEntry(date: when, snapshot: s, cover: cover))
            }
            let refresh = Date(timeIntervalSinceNow: Double(min(horizon, s.durationMs > 0 ? s.durationMs : horizon) - nowMs) / 1000 + 1)
            completion(Timeline(entries: entries, policy: .after(max(refresh, .now + 30))))
        } else {
            // Paused or idle: a short refresh, so a resume shows within a minute even without a reload from the app.
            completion(Timeline(entries: entries, policy: .after(.now + 60)))
        }
    }

    private func entry(at date: Date) -> NowEntry {
        guard let loaded = SharedStore.load(bundle: bundle), !loaded.snapshot.isIdle else {
            return NowEntry(date: date, snapshot: nil, cover: nil)
        }
        return NowEntry(date: date, snapshot: loaded.snapshot, cover: SharedStore.cover(loaded.snapshot, in: loaded.dir))
    }
}

struct NowView: View {
    let entry: NowEntry
    @Environment(\.widgetFamily) private var family

    private var lines: (current: String?, next: String?) {
        guard let s = entry.snapshot, let i = s.lineIndex(at: s.position(at: entry.date)) else { return (nil, nil) }
        return (s.lyrics[i].text, i + 1 < s.lyrics.count ? s.lyrics[i + 1].text : nil)
    }

    var body: some View {
        if let s = entry.snapshot {
            content(s)
        } else {
            VStack(spacing: 6) {
                Image(systemName: "moon.stars.fill").font(.title2)
                Text("Nothing playing").font(.footnote.weight(.semibold))
            }
            .foregroundStyle(.white.opacity(0.7))
        }
    }

    @ViewBuilder
    private func content(_ s: NowPlayingSnapshot) -> some View {
        let big = family == .systemLarge
        VStack(alignment: .leading, spacing: big ? 16 : 8) {
            HStack(spacing: 10) {
                cover(size: big ? 88 : (family == .systemSmall ? 40 : 56))
                VStack(alignment: .leading, spacing: 2) {
                    Text(s.title).font(.system(size: big ? 18 : 13, weight: .bold)).lineLimit(1)
                    Text(s.artist).font(.system(size: big ? 14 : 11)).foregroundStyle(.white.opacity(0.65)).lineLimit(1)
                }
                Spacer(minLength: 0)
                if !s.playing { Image(systemName: "pause.fill").font(.caption).foregroundStyle(.white.opacity(0.6)) }
            }
            if let now = lines.current {
                VStack(alignment: .leading, spacing: 4) {
                    Text(now)
                        .font(.system(size: big ? 26 : (family == .systemSmall ? 14 : 18), weight: .bold))
                        .lineLimit(family == .systemSmall ? 3 : 2)
                        .minimumScaleFactor(0.7)
                    if family != .systemSmall, let next = lines.next {
                        Text(next)
                            .font(.system(size: big ? 18 : 13, weight: .semibold))
                            .foregroundStyle(.white.opacity(0.45))
                            .lineLimit(big ? 2 : 1)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            } else {
                Spacer(minLength: 0)
            }
        }
        .foregroundStyle(.white)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }

    @ViewBuilder
    private func cover(size: CGFloat) -> some View {
        if let image = entry.cover {
            Image(nsImage: image).resizable().scaledToFill()
                .frame(width: size, height: size).clipShape(RoundedRectangle(cornerRadius: size * 0.16))
        } else {
            RoundedRectangle(cornerRadius: size * 0.16).fill(.white.opacity(0.12)).frame(width: size, height: size)
                .overlay(Image(systemName: "music.note").foregroundStyle(.white.opacity(0.6)))
        }
    }
}

struct LunoNowPlayingWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: "LunoNowPlaying", provider: NowProvider()) { entry in
            NowView(entry: entry)
                .containerBackground(for: .widget) {
                    ZStack {
                        Color.black
                        if let image = entry.cover {
                            Image(nsImage: image).resizable().scaledToFill().blur(radius: 40).opacity(0.7)
                        }
                        Color.black.opacity(0.45)
                    }
                }
        }
        .configurationDisplayName("Now Playing")
        .description("The song and the lyric line being sung.")
        .supportedFamilies([.systemSmall, .systemMedium, .systemLarge])
    }
}

@main
struct LunoWidgets: WidgetBundle {
    var body: some Widget { LunoNowPlayingWidget() }
}
