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
            .navigationTitle("Home")
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

/// Quick capture: one line (and optionally a supertag) onto today's page.
struct CaptureSheet: View {
    @Environment(Store.self) private var store
    let close: () -> Void
    @State private var text = ""
    @State private var tag = ""
    @FocusState private var focused: Bool

    var body: some View {
        NavigationStack {
            Form {
                TextField("Capture a thought", text: $text, axis: .vertical)
                    .focused($focused)
                    .accessibilityIdentifier("capture.text")
                TextField("Supertag (optional)", text: $tag)
                    .textInputAutocapitalization(.never)
                    .accessibilityIdentifier("capture.tag")
            }
            .navigationTitle("Quick add to today")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel", action: close) }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Add") {
                        var p: [String: Any] = ["name": text]
                        let t = tag.trimmingCharacters(in: .whitespaces).trimmingCharacters(in: CharacterSet(charactersIn: "#"))
                        if !t.isEmpty { p["tag"] = t }
                        store.run("node.quickAdd", p)
                        store.save()
                        close()
                    }
                    .disabled(text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    .accessibilityIdentifier("capture.add")
                }
            }
            .onAppear { focused = true }
        }
        .presentationDetents([.medium])
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
