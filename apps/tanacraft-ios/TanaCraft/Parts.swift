import SwiftUI

/// Tag colour tones by name (or a custom `#rrggbb`), grey for anything unknown.
func tagTones(_ name: String?) -> (fg: Color, t1: Color, t2: Color) {
    if let n = name, n.hasPrefix("#"), n.count == 7, let v = Int(n.dropFirst(), radix: 16) {
        let c = Color(red: Double((v >> 16) & 255) / 255, green: Double((v >> 8) & 255) / 255, blue: Double(v & 255) / 255)
        return (c, c.opacity(0.22), c.opacity(0.32))
    }
    return T.tags[name ?? "grey"] ?? T.tags["grey"] ?? (T.cMuted, T.cHover, T.cPressed)
}

/// A bullet (DESIGN.md "Outline row"): a dot; a collapsed parent's dot sits in a grey halo; a
/// tagged node's halo takes the tag's strong tint with a ring and dot in the tag colour; a search
/// node shows a small magnifier.
struct Bullet: View {
    let kind: String
    let collapsed: Bool
    let tag: String?

    var body: some View {
        let tones = tag.map { tagTones($0) }
        ZStack {
            if collapsed || tones != nil {
                Circle().fill(tones?.t2 ?? T.cBulletHalo).frame(width: Phone.bullet, height: Phone.bullet)
            }
            if let tones, !collapsed {
                Circle().strokeBorder(tones.fg, lineWidth: 1).frame(width: Phone.bullet, height: Phone.bullet)
            }
            if kind == "search" {
                Image(systemName: "magnifyingglass").font(.system(size: 8, weight: .bold)).foregroundStyle(tones?.fg ?? T.cBullet)
            } else {
                Circle().fill(tones?.fg ?? T.cBullet).frame(width: Phone.dot, height: Phone.dot)
            }
        }
    }
}

/// An 18 pt checkbox: translucent when open, done green with a white tick when checked.
struct CheckBox: View {
    let on: Bool
    var body: some View {
        RoundedRectangle(cornerRadius: T.rSm)
            .fill(on ? T.cDone : T.cCheckbox)
            .frame(width: T.checkbox, height: T.checkbox)
            .overlay {
                if on { Image(systemName: "checkmark").font(.system(size: 11, weight: .bold)).foregroundStyle(T.cOnAccent) }
            }
    }
}

/// A tag chip: "#" and the name, the tag's colour on a tint of itself (the Chip Rule).
struct ChipView: View {
    @Environment(Store.self) private var store
    let chip: Chip
    var body: some View {
        let tones = tagTones(chip.color)
        Button {
            store.run("nav.open", ["node": chip.id])
        } label: {
            Text("# \(chip.name)")
                .font(Fonts.font(Phone.label, T.fwButton))
                .foregroundStyle(tones.fg)
                .padding(.horizontal, T.q1 * 1.5)
                .frame(minHeight: T.chipH * Phone.scale)
                .background(RoundedRectangle(cornerRadius: T.rSm).fill(tones.t1))
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("chip.\(chip.name)")
    }
}

struct ChipRow: View {
    let tags: [Chip]
    var body: some View {
        HStack(spacing: T.q1) {
            ForEach(tags, id: \.id) { ChipView(chip: $0) }
        }
    }
}

/// The coloured "#" square in front of a supertag's title.
struct TagSquare: View {
    let color: String
    var body: some View {
        let tones = tagTones(color)
        Text("#")
            .font(Fonts.font(T.fsSection, T.fwSemibold))
            .foregroundStyle(tones.fg)
            .frame(width: 28, height: 28)
            .background(RoundedRectangle(cornerRadius: 28 * 0.2).fill(tones.t2))
    }
}
