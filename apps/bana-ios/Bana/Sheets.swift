import SwiftUI

/// The sidebar as a sheet: Today, Daily notes, Supertags, pinned nodes, each workspace's Library
/// entries and Trash.
struct HomeSheet: View {
    @Environment(Store.self) private var store
    let close: () -> Void

    var body: some View {
        let bar = store.sidebar()
        NavigationStack {
            List {
                Section {
                    item("Today", "calendar", id: "home.today") { store.run("nav.today") }
                    item("Daily notes", "calendar.day.timeline.left", id: "home.daily") { store.run("nav.dailyNotes") }
                    item("Supertags", "number", id: "home.supertags") { store.run("nav.supertags") }
                    item("Library", "books.vertical", id: "home.library") { store.run("nav.home") }
                }
                if let pinned = bar?.pinned, !pinned.isEmpty {
                    Section("Pinned") {
                        ForEach(pinned, id: \.id) { e in entry(e) }
                    }
                }
                ForEach(bar?.workspaces ?? [], id: \.id) { ws in
                    Section(ws.name) {
                        ForEach(ws.entries, id: \.id) { e in entry(e) }
                        item("Trash", "trash", id: "home.trash") { store.run("nav.open", ["node": ws.trash]) }
                    }
                }
            }
            .navigationTitle("Browse")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done", action: close) } }
        }
        .presentationDetents([.large])
    }

    private func entry(_ e: SidebarEntry) -> some View {
        Button {
            store.run("nav.open", ["node": e.id])
            close()
        } label: {
            HStack {
                Bullet(kind: e.kind, collapsed: true, tag: e.color).frame(width: 22)
                Text(e.name).foregroundStyle(T.cText)
            }
        }
        .accessibilityIdentifier("home.entry")
    }

    private func item(_ title: String, _ symbol: String, id: String, _ go: @escaping () -> Void) -> some View {
        Button {
            go()
            close()
        } label: {
            Label(title, systemImage: symbol).foregroundStyle(T.cText)
        }
        .accessibilityIdentifier(id)
    }
}

/// Search and open: fuzzy matches by name (recent pages while the box is empty).
struct SearchSheet: View {
    @Environment(Store.self) private var store
    let close: () -> Void
    @State private var query = ""
    @FocusState private var focused: Bool

    var body: some View {
        NavigationStack {
            List(store.find(query), id: \.id) { r in
                Button {
                    store.run("nav.open", ["node": r.id])
                    close()
                } label: {
                    VStack(alignment: .leading, spacing: 2) {
                        HStack {
                            Text(AttributedString(Format.styled(r.title, refs: store.page?.refs ?? [:], size: Phone.body, weight: 400, color: UIColor(T.cText))))
                            ChipRow(tags: r.tags)
                        }
                        if !r.path.isEmpty {
                            Text(r.path).font(Fonts.font(Phone.label, 400)).foregroundStyle(T.cMuted)
                        }
                    }
                }
                .accessibilityIdentifier("search.result")
            }
            .searchable(text: $query, placement: .navigationBarDrawer(displayMode: .always), prompt: "Search nodes")
            .navigationTitle("Search")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Close", action: close).accessibilityIdentifier("sheet.close") } }
        }
    }
}

/// Quick capture: one line, optionally with a supertag, to Today, Tomorrow, the Inbox or a pinned
/// node (Tana's public mobile docs name these destinations). The last destination is remembered.
struct CaptureSheet: View {
    @Environment(Store.self) private var store
    let close: () -> Void
    @State private var text = ""
    @State private var tag = ""
    @AppStorage("captureTo") private var destination = "today"
    @FocusState private var focused: Bool

    var body: some View {
        let pinned = store.sidebar()?.pinned.filter { $0.kind != "tag" } ?? []
        NavigationStack {
            Form {
                TextField("Capture a thought", text: $text, axis: .vertical)
                    .focused($focused)
                    .accessibilityIdentifier("capture.text")
                TextField("Supertag (optional)", text: $tag)
                    .textInputAutocapitalization(.never)
                    .accessibilityIdentifier("capture.tag")
                Picker("Add to", selection: $destination) {
                    Text("Today").tag("today")
                    Text("Tomorrow").tag("tomorrow")
                    Text("Inbox").tag("inbox")
                    ForEach(pinned, id: \.id) { p in Text(p.name).tag("node:\(p.id)") }
                }
                .accessibilityIdentifier("capture.destination")
            }
            // A partial-height sheet is Liquid Glass on iOS 26: let it show through.
            .scrollContentBackground(.hidden)
            .navigationTitle("Capture")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel", action: close) }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Add", action: add)
                        .disabled(text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                        .accessibilityIdentifier("capture.add")
                }
            }
            .onAppear { focused = true }
        }
        .presentationDetents([.medium, .large])
    }

    private func add() {
        var p: [String: Any] = ["name": text]
        let t = tag.trimmingCharacters(in: .whitespaces).trimmingCharacters(in: CharacterSet(charactersIn: "#"))
        if !t.isEmpty { p["tag"] = t }
        switch destination {
        case "inbox": store.run("node.capture", p)
        case "tomorrow":
            p["date"] = "tomorrow"
            store.run("node.quickAdd", p)
        case let d where d.hasPrefix("node:"):
            p["parent"] = Int64(d.dropFirst(5)) ?? 0
            store.run("node.quickAdd", p)
        default: store.run("node.quickAdd", p)
        }
        store.save()
        close()
    }
}

/// Every supertag with a filter, pinned ones first; a tap opens its page.
struct SupertagsSheet: View {
    @Environment(Store.self) private var store
    let close: () -> Void
    @State private var query = ""

    struct Tag: Decodable, Hashable {
        let id: Int64
        let name: String
        let color: String
        let pinned: Bool
        let count: Int
    }
    struct Tags: Decodable { let tags: [Tag] }

    var body: some View {
        let all = store.decode(Tags.self, "tags")?.tags ?? []
        let q = query.lowercased().trimmingCharacters(in: CharacterSet(charactersIn: "# "))
        let shown = all.filter { q.isEmpty || $0.name.lowercased().contains(q) }.sorted { ($0.pinned ? 0 : 1, $0.name.lowercased()) < ($1.pinned ? 0 : 1, $1.name.lowercased()) }
        NavigationStack {
            List(shown, id: \.id) { t in
                Button {
                    store.run("nav.open", ["node": t.id])
                    close()
                } label: {
                    HStack {
                        ChipLabel(name: t.name, color: t.color)
                        if t.pinned { Image(systemName: "pin.fill").font(.caption).foregroundStyle(T.cMuted) }
                        Spacer()
                        Text("\(t.count)").foregroundStyle(T.cMuted)
                    }
                }
                .accessibilityIdentifier("supertag.\(t.name)")
            }
            .searchable(text: $query, placement: .navigationBarDrawer(displayMode: .always), prompt: "Filter supertags")
            .navigationTitle("Supertags")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Close", action: close) }
                ToolbarItem(placement: .primaryAction) {
                    Button("Schema") {
                        store.run("nav.supertags")
                        close()
                    }
                    .accessibilityIdentifier("supertags.schema")
                }
            }
        }
    }
}

/// Theme, sync, the graph file and the version.
struct ManageSheet: View {
    @Environment(Store.self) private var store
    let close: () -> Void
    @AppStorage("theme") private var theme = "system"

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    NavigationLink {
                        ProfileView(pair: {})
                    } label: {
                        HStack(spacing: T.q3) {
                            AvatarView(name: store.profile?.name ?? "", avatar: store.profile?.avatar, size: 40)
                            VStack(alignment: .leading) {
                                Text(store.profile?.name.isEmpty == false ? store.profile?.name ?? "" : "Your profile")
                                Text(store.bana?.signedIn == true ? "Bana ID: signed in" : "Bana ID: optional").font(.caption).foregroundStyle(T.cMuted)
                            }
                        }
                    }
                    .accessibilityIdentifier("manage.profile")
                }
                Picker("Theme", selection: $theme) {
                    Text("System").tag("system")
                    Text("Light").tag("light")
                    Text("Dark").tag("dark")
                }
                .pickerStyle(.segmented)
                .accessibilityIdentifier("manage.theme")
                SyncSection()
                Section("Graph") {
                    LabeledContent("File", value: URL(fileURLWithPath: store.engine?.path() ?? "").lastPathComponent)
                    Button("Save now") { store.save() }
                }
                Section {
                    LabeledContent("Version", value: Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "")
                }
            }
            .navigationTitle("Manage")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done", action: close) } }
        }
    }
}

/// The "#" menu: pick a supertag for a line, or create one by name.
struct TagPicker: View {
    @Environment(Store.self) private var store
    let node: Int64
    let close: () -> Void
    @State private var query = ""

    struct Choice: Decodable, Hashable {
        let id: Int64?
        let label: String?
        let color: String?
        let create: String?
    }

    struct Choices: Decodable {
        let items: [Choice]
    }

    var body: some View {
        let choices = store.decode(Choices.self, "choices", ["mode": "tag", "node": node, "query": query])?.items ?? []
        NavigationStack {
            List {
                ForEach(choices, id: \.self) { c in
                    Button {
                        if let name = c.create ?? c.label {
                            store.run("tag.add", ["node": node, "tag": c.id.map { $0 as Any } ?? name])
                        }
                        close()
                    } label: {
                        if let name = c.create {
                            Label("Create #\(name)", systemImage: "plus")
                        } else {
                            ChipLabel(name: c.label ?? "", color: c.color)
                        }
                    }
                    .accessibilityIdentifier("tag.choice")
                }
            }
            .scrollContentBackground(.hidden)
            .searchable(text: $query, placement: .navigationBarDrawer(displayMode: .always), prompt: "Supertag")
            .navigationTitle("Add supertag")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Cancel", action: close) } }
        }
        .presentationDetents([.medium, .large])
    }
}

struct ChipLabel: View {
    let name: String
    let color: String?
    var body: some View {
        let tones = tagTones(color)
        Text("# \(name)")
            .font(Fonts.font(Phone.body, T.fwButton))
            .foregroundStyle(tones.fg)
            .padding(.horizontal, T.q2)
            .padding(.vertical, 2)
            .background(RoundedRectangle(cornerRadius: T.rSm).fill(tones.t1))
    }
}
