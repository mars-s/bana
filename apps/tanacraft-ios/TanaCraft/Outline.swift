import SwiftUI

/// The page of the engine's zoomed node: title, tags, fields and the outline (or a view's items).
struct PageView: View {
    @Environment(Store.self) private var store
    @Environment(\.colorScheme) private var scheme
    let page: Page
    @Binding var tagPickerFor: Int64?

    var body: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    header
                        .padding(.bottom, T.q3)
                    if page.view.isView {
                        ViewItems(page: page)
                    }
                    ForEach(page.rows.items, id: \.key) { row in
                        RowView(row: row, page: page, tagPickerFor: $tagPickerFor)
                            .id(row.key)
                    }
                    addLine
                }
                .padding(.horizontal, T.q4)
                .padding(.bottom, 120)
            }
            .scrollDismissesKeyboard(.interactively)
            .onChange(of: page.focus) { _, f in
                guard let f else { return }
                withAnimation(nil) { proxy.scrollTo("n\(f)") }
            }
        }
    }

    // MARK: title

    @ViewBuilder private var header: some View {
        VStack(alignment: .leading, spacing: T.q2) {
            HStack(alignment: .firstTextBaseline, spacing: T.q2) {
                if page.kind == "tag" {
                    TagSquare(color: page.tagColor ?? "grey")
                }
                if page.titleEditable {
                    LineEditor(
                        text: page.name, refs: page.refs, focused: false, caret: 0,
                        size: T.fsTitle, weight: T.fwTitle, color: .label, identifier: "title",
                        actions: LineActions(
                            changed: { name, caret in store.run("node.setName", ["node": page.zoom, "name": name, "caret": caret]) },
                            began: { _ in },
                            returned: { _ in addFirstLine() }
                        )
                    )
                } else {
                    Text(page.title)
                        .font(Fonts.font(T.fsTitle, T.fwTitle))
                        .foregroundStyle(T.cStrong)
                        .accessibilityIdentifier("title")
                        .accessibilityAddTraits(.isHeader)
                }
                Spacer(minLength: 0)
                if page.kind == "day" {
                    Button { store.run("nav.previousDay") } label: { Image(systemName: "chevron.left") }
                        .accessibilityIdentifier("day.previous")
                    Button { store.run("nav.nextDay") } label: { Image(systemName: "chevron.right") }
                        .accessibilityIdentifier("day.next")
                }
            }
            .tint(T.cSecondary)
            if !page.tags.isEmpty {
                ChipRow(tags: page.tags)
            }
            if !page.description.isEmpty {
                Text(page.description).font(Fonts.font(Phone.dense, Fonts.textWeight(scheme))).foregroundStyle(T.cMuted)
            }
        }
        .padding(.top, T.q4)
    }

    /// A tap below the last line adds a line at the end (an empty page starts this way).
    private var addLine: some View {
        Button {
            addLastLine()
        } label: {
            HStack {
                Text(page.rows.items.isEmpty ? "Tap to start writing" : " ")
                    .font(Fonts.font(Phone.body, Fonts.textWeight(scheme)))
                    .foregroundStyle(T.cFaint)
                Spacer()
            }
            .frame(minHeight: Phone.rowH * 2)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("page.addLine")
    }

    private func addLastLine() {
        // An empty last line is reused instead of piling up empty lines.
        if let last = page.rows.items.last(where: { $0.k == "node" && $0.depth == 0 }), let id = last.id, (last.name ?? "").isEmpty {
            store.run("focus.set", ["node": id, "caret": 0])
            return
        }
        guard let r = store.run("node.add", ["parent": page.target], redraw: false), let id = (r["id"] as? NSNumber)?.int64Value else { return }
        store.run("focus.set", ["node": id, "caret": 0])
    }

    private func addFirstLine() {
        guard let r = store.run("node.add", ["parent": page.target, "index": 0], redraw: false), let id = (r["id"] as? NSNumber)?.int64Value else { return }
        store.run("focus.set", ["node": id, "caret": 0])
    }
}

/// One display row: a node line, a field row, the empty line under an open node, or "+ field".
struct RowView: View {
    @Environment(Store.self) private var store
    @Environment(\.colorScheme) private var scheme
    let row: Row
    let page: Page
    @Binding var tagPickerFor: Int64?

    var body: some View {
        switch row.k {
        case "node": nodeRow
        case "field": FieldRowView(row: row)
        case "empty": emptyRow
        default: EmptyView()
        }
    }

    private var indent: CGFloat { CGFloat(row.depth) * Phone.indent }

    private var nodeRow: some View {
        let id = row.id ?? 0
        let name = row.name ?? ""
        let focused = page.focus == id
        return HStack(alignment: .top, spacing: 0) {
            Color.clear.frame(width: indent, height: 1)
            Button {
                store.toggleExpand(id)
            } label: {
                Image(systemName: row.expanded == true ? "chevron.down" : "chevron.right")
                    .font(.system(size: 10, weight: .semibold))
                    .foregroundStyle(T.cMuted)
                    .frame(width: T.q5, height: Phone.rowH)
                    .contentShape(Rectangle())
                    .opacity(row.hasKids == true ? 1 : 0)
            }
            .buttonStyle(.plain)
            .disabled(row.hasKids != true)
            .accessibilityIdentifier("expand")
            .accessibilityLabel(row.hasKids != true ? "No children" : row.expanded == true ? "Collapse \(store.shown(name))" : "Expand \(store.shown(name))")
            Button {
                store.run("nav.open", ["node": id])
            } label: {
                Bullet(kind: row.kind ?? "plain", collapsed: row.hasKids == true && row.expanded != true, tag: row.tags?.first?.color)
                    .frame(width: Phone.rowH, height: Phone.rowH)
            }
            .buttonStyle(.plain)
            .contextMenu { nodeMenu(id) }
            .accessibilityIdentifier("bullet")
            .accessibilityLabel("Open \(store.shown(name))")
            if let done = row.done {
                Button {
                    store.run("node.toggleDone", ["node": id])
                } label: {
                    CheckBox(on: done).frame(height: Phone.rowH)
                }
                .buttonStyle(.plain)
                .padding(.trailing, T.q2)
                .accessibilityIdentifier("checkbox")
                .accessibilityValue(done ? "done" : "not done")
            }
            LineEditor(
                text: name,
                refs: page.refs,
                focused: focused,
                caret: page.caret,
                weight: row.heading == true ? T.fwSemibold : nil,
                color: row.done == true ? UIColor(T.cMuted) : (row.kind == "reference" || row.kind == "missing" ? UIColor(T.cLink) : UIColor(T.cText)),
                strike: row.done == true,
                actions: actions(id)
            )
            .padding(.vertical, (Phone.rowH - Fonts.ui(Phone.body, 400).lineHeight) / 2)
            if let tags = row.tags, !tags.isEmpty {
                ChipRow(tags: tags)
                    .frame(height: Phone.rowH)
                    .padding(.leading, T.q1)
                    .fixedSize()
            }
        }
        .frame(minHeight: Phone.rowH)
    }

    /// The node menu (long press on the bullet).
    @ViewBuilder private func nodeMenu(_ id: Int64) -> some View {
        Button { store.run("nav.open", ["node": id]) } label: { Label("Open", systemImage: "arrow.up.right") }
        Button { tagPickerFor = id } label: { Label("Add supertag", systemImage: "number") }
        Button { store.run("node.toggleDone", ["node": id]) } label: { Label("Checkbox", systemImage: "checkmark.square") }
        Button { store.run("node.addToDay", ["node": id, "date": "today"]) } label: { Label("Add to today", systemImage: "calendar.badge.plus") }
        Button { store.run("node.togglePin", ["node": id]) } label: { Label("Pin or unpin", systemImage: "pin") }
        Divider()
        Button(role: .destructive) { store.run("node.delete", ["node": id]) } label: { Label("Delete", systemImage: "trash") }
    }

    private func actions(_ id: Int64) -> LineActions {
        LineActions(
            changed: { name, caret in store.run("node.setName", ["node": id, "name": name, "caret": caret]) },
            began: { caret in store.run("focus.set", ["node": id, "caret": caret]) },
            returned: { caret in store.run("node.enter", ["node": id, "caret": caret]) },
            backspaceAtStart: { store.run("node.backspace", ["node": id]) },
            ended: {
                if store.page?.focus == id { store.run("focus.clear") }
            },
            accessory: [
                AccessoryItem(id: "outdent", symbol: "decrease.indent", label: "Outdent") { store.run("node.outdent", ["node": id]) },
                AccessoryItem(id: "indent", symbol: "increase.indent", label: "Indent") { store.run("node.indent", ["node": id]) },
                AccessoryItem(id: "up", symbol: "arrow.up", label: "Move up") { store.run("node.moveUp", ["node": id]) },
                AccessoryItem(id: "down", symbol: "arrow.down", label: "Move down") { store.run("node.moveDown", ["node": id]) },
                AccessoryItem(id: "collapse", symbol: "chevron.up.chevron.down", label: "Expand or collapse") { store.toggleExpand(id) },
                AccessoryItem(id: "done", symbol: "checkmark.square", label: "Checkbox") { store.run("node.toggleDone", ["node": id]) },
                AccessoryItem(id: "tag", symbol: "number", label: "Add supertag") { tagPickerFor = id },
                AccessoryItem(id: "undo", symbol: "arrow.uturn.backward", label: "Undo") { store.run("edit.undo") },
                AccessoryItem(id: "redo", symbol: "arrow.uturn.forward", label: "Redo") { store.run("edit.redo") },
                AccessoryItem(id: "hide", symbol: "keyboard.chevron.compact.down", label: "Hide keyboard") {
                    UIApplication.shared.sendAction(#selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil)
                },
            ]
        )
    }

    /// The empty line under an open node with no children: a tap starts its first child.
    private var emptyRow: some View {
        Button {
            guard let parent = row.parent,
                  let r = store.run("node.add", ["parent": parent], redraw: false),
                  let id = (r["id"] as? NSNumber)?.int64Value else { return }
            store.run("focus.set", ["node": id, "caret": 0])
        } label: {
            HStack(spacing: 0) {
                Color.clear.frame(width: CGFloat(row.depth) * Phone.indent + T.q5, height: 1)
                Bullet(kind: "plain", collapsed: false, tag: nil).opacity(0.4).frame(width: Phone.rowH, height: Phone.rowH)
                Spacer()
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("emptyLine")
    }
}

/// A field row: name on the left, values on the right, in a soft box (DESIGN.md field-row).
struct FieldRowView: View {
    @Environment(Store.self) private var store
    @Environment(\.colorScheme) private var scheme
    let row: Row
    @State private var editing = false
    @State private var draft = ""
    @State private var picking = false
    @State private var options: [String] = []

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: T.q2) {
            Color.clear.frame(width: CGFloat(row.depth) * Phone.indent + T.q5, height: 1)
            Text(row.name ?? "")
                .font(Fonts.font(Phone.dense, T.fwMedium))
                .foregroundStyle(T.cSecondary)
                .frame(width: 110, alignment: .leading)
                .lineLimit(2)
            values
            Spacer(minLength: 0)
        }
        .padding(.vertical, T.q2)
        .padding(.trailing, T.q2)
        .background(RoundedRectangle(cornerRadius: T.rLg).fill(T.cRaised).padding(.leading, CGFloat(row.depth) * Phone.indent + T.q3))
        .padding(.vertical, T.q1 / 2)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("field")
    }

    @ViewBuilder private var values: some View {
        let vals = row.values ?? []
        if row.type == "options" || row.type == "options_from_tag" || row.type == "user" {
            Button {
                options = store.decode(Choices.self, "choices", ["mode": "choice", "node": row.owner ?? 0, "field": row.field ?? 0])?.items.compactMap(\.label) ?? []
                picking = true
            } label: {
                valueText(vals)
            }
            .buttonStyle(.plain)
            .accessibilityIdentifier("fieldValue")
            .confirmationDialog(row.name ?? "", isPresented: $picking) {
                ForEach(options, id: \.self) { o in Button(o) { set(o) } }
                if !(vals.first?.text ?? "").isEmpty { Button("Clear", role: .destructive) { set("") } }
            }
        } else if row.type == "checkbox" {
            let on = vals.first?.checked ?? false
            Button { set(on ? "false" : "true") } label: { CheckBox(on: on) }.buttonStyle(.plain)
        } else if editing {
            TextField("Empty", text: $draft)
                .font(Fonts.font(Phone.body, Fonts.textWeight(scheme)))
                .submitLabel(.done)
                .onChange(of: draft) { _, v in set(v) }
                .onSubmit { editing = false }
                .accessibilityIdentifier("fieldValue")
        } else {
            Button {
                draft = vals.first?.text ?? ""
                editing = true
            } label: {
                valueText(vals)
            }
            .buttonStyle(.plain)
            .accessibilityIdentifier("fieldValue")
        }
    }

    private func valueText(_ vals: [ValueCell]) -> some View {
        let empty = vals.allSatisfy { $0.text.isEmpty }
        return Text(empty ? "Empty" : vals.map(\.text).joined(separator: ", "))
            .font(Fonts.font(Phone.body, Fonts.textWeight(scheme)))
            .foregroundStyle(empty ? T.cFaint : T.cText)
            .multilineTextAlignment(.leading)
    }

    struct Choices: Decodable {
        struct Item: Decodable { let label: String? }
        let items: [Item]
    }

    /// Every keystroke of a value goes to the engine at once (`keep`: text that does not parse
    /// yet as a date or number is kept as typed).
    private func set(_ v: String) {
        guard let owner = row.owner, let field = row.field else { return }
        store.run("field.set", ["node": owner, "field": field, "value": v, "keep": true])
    }
}

/// Items of a view page (a supertag's tagged nodes, a search node's results), as a list.
struct ViewItems: View {
    @Environment(Store.self) private var store
    @Environment(\.colorScheme) private var scheme
    let page: Page

    struct Item: Decodable, Hashable {
        let id: Int64
        let name: String
        let done: Bool?
        let tags: [Chip]
        let kind: String
    }
    struct Group: Decodable, Hashable {
        let name: String?
        let items: [Item]
    }
    struct Snapshot: Decodable {
        let groups: [Group]
    }

    var body: some View {
        let snap = store.decode(Snapshot.self, "view", ["node": page.zoom])
        let groups = snap?.groups ?? []
        VStack(alignment: .leading, spacing: 0) {
            ForEach(Array(groups.enumerated()), id: \.offset) { _, g in
                if let n = g.name, !n.isEmpty {
                    Text(n).font(Fonts.font(Phone.body, T.fwSemibold)).foregroundStyle(T.cStrong).padding(.vertical, T.q2)
                }
                ForEach(g.items, id: \.id) { it in
                    HStack(spacing: T.q2) {
                        if let d = it.done {
                            Button { store.run("node.toggleDone", ["node": it.id]) } label: { CheckBox(on: d) }.buttonStyle(.plain)
                        }
                        Button { store.run("nav.open", ["node": it.id]) } label: {
                            HStack {
                                Text(AttributedString(Format.styled(it.name, refs: page.refs, size: Phone.body, weight: Fonts.textWeight(scheme), color: UIColor(it.done == true ? T.cMuted : T.cText))))
                                    .strikethrough(it.done == true)
                                    .multilineTextAlignment(.leading)
                                Spacer(minLength: 0)
                                ChipRow(tags: it.tags).fixedSize()
                            }
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                        .accessibilityIdentifier("viewItem")
                    }
                    .frame(minHeight: Phone.rowH)
                    Divider().overlay(T.cDivider)
                }
            }
            if groups.allSatisfy({ $0.items.isEmpty }) {
                Text("Nothing here yet.").font(Fonts.font(Phone.dense, Fonts.textWeight(scheme))).foregroundStyle(T.cMuted).padding(.vertical, T.q2)
            }
        }
        .padding(.bottom, T.q4)
    }
}
