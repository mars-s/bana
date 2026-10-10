import BackgroundTasks
import SwiftUI

/// The background refresh that catches up with the other devices while the app is closed.
let syncTask = "org.bana.app.sync"

@main
struct BanaApp: App {
    @State private var store = Store.openDefault()
    @AppStorage("theme") private var theme = "system"
    @Environment(\.scenePhase) private var phase

    var body: some Scene {
        WindowGroup {
            RootView()
                .environment(store)
                .preferredColorScheme(theme == "dark" ? .dark : theme == "light" ? .light : nil)
                .onReceive(NotificationCenter.default.publisher(for: UIApplication.willTerminateNotification)) { _ in store.save() }
                // Sync on launch, then keep the status fresh while the app is open (edits go out
                // as they are made; the engine's sync thread sends them).
                .task {
                    await store.syncNow()
                    while !Task.isCancelled {
                        try? await Task.sleep(for: .seconds(3))
                        store.readSync()
                    }
                }
        }
        // Leaving the foreground writes the file at once (the engine also autosaves within half a
        // second) and asks for a background refresh; coming back syncs at once.
        .onChange(of: phase) { _, p in
            if p != .active {
                store.save()
                scheduleSync()
            } else {
                Task { await store.syncNow() }
            }
        }
        .backgroundTask(.appRefresh(syncTask)) {
            scheduleSync()
            await store.syncNow(wait: 25_000)
            store.save()
        }
    }

    private func scheduleSync() {
        guard store.sync?.state != nil, store.sync?.state != "off" else { return }
        let r = BGAppRefreshTaskRequest(identifier: syncTask)
        r.earliestBeginDate = Date(timeIntervalSinceNow: 15 * 60)
        try? BGTaskScheduler.shared.submit(r)
    }
}

enum Sheet: String, Identifiable {
    case home, search, capture, supertags, manage
    var id: String { rawValue }
}

struct RootView: View {
    @Environment(Store.self) private var store
    @Environment(\.colorScheme) private var scheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var sheet: Sheet?
    @State private var tagPickerFor: Int64?
    @Namespace private var glass

    /// A line has the caret: the bottom bar turns into the editing bar.
    private var editing: Int64? { store.page?.focus }

    var body: some View {
        ZStack {
            T.cPanel.ignoresSafeArea()
            if let page = store.page {
                PageView(page: page, tagPickerFor: $tagPickerFor)
            } else {
                Text(store.error ?? "Opening…").foregroundStyle(T.cMuted).padding()
            }
        }
        // Chrome floats over the page in Liquid Glass; the outline scrolls under it (the bars
        // bring the system's scroll edge effect). The page itself stays flat (DESIGN.md).
        .safeAreaBar(edge: .top) { topBar }
        .safeAreaBar(edge: .bottom) { bottomBar }
        .overlay(alignment: .top) { errorBanner }
        .sheet(item: $sheet) { s in
            switch s {
            case .home: HomeSheet(close: { sheet = nil })
            case .search: SearchSheet(close: { sheet = nil })
            case .capture: CaptureSheet(close: { sheet = nil })
            case .supertags: SupertagsSheet(close: { sheet = nil })
            case .manage: ManageSheet(close: { sheet = nil })
            }
        }
        .sheet(item: Binding(get: { tagPickerFor.map(NodeRef.init) }, set: { tagPickerFor = $0?.id })) { ref in
            TagPicker(node: ref.id, close: { tagPickerFor = nil })
        }
    }

    // MARK: top: Back, breadcrumbs, Today

    private var topBar: some View {
        GlassEffectContainer(spacing: T.q3) {
            HStack(spacing: T.q2) {
                Button { store.run("nav.back") } label: { Image(systemName: "chevron.backward").frame(width: 22, height: 22) }
                    .buttonStyle(.glass)
                    .buttonBorderShape(.circle)
                    .disabled(!(store.page?.canBack ?? false))
                    .accessibilityIdentifier("nav.back")
                    .accessibilityLabel("Back")
                if !(store.page?.crumbs.isEmpty ?? true) {
                    crumbs
                        .padding(.horizontal, T.q3)
                        .frame(height: 44)
                        .glassEffect(.regular.interactive(), in: .capsule)
                }
                Spacer(minLength: 0)
                Button { store.run("nav.today") } label: { Image(systemName: "calendar").frame(width: 22, height: 22) }
                    .buttonStyle(.glass)
                    .buttonBorderShape(.circle)
                    .accessibilityIdentifier("nav.today")
                    .accessibilityLabel("Today")
            }
        }
        .font(.system(size: 17, weight: .medium))
        .tint(T.cStrong)
        .padding(.horizontal, T.q4)
        .padding(.vertical, T.q1)
    }

    private var crumbs: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: T.q1) {
                ForEach(Array((store.page?.crumbs ?? []).enumerated()), id: \.element.id) { i, c in
                    if i > 0 { Text("/").foregroundStyle(T.cFaint) }
                    Button { store.run("nav.open", ["node": c.id]) } label: {
                        Text(c.name).lineLimit(1)
                    }
                    .accessibilityIdentifier("crumb")
                }
            }
            .font(Fonts.font(Phone.label, T.fwButton))
            .foregroundStyle(T.cSecondary)
        }
        .defaultScrollAnchor(.trailing)
        .fixedSize(horizontal: false, vertical: true)
    }

    // MARK: bottom: the tabs and Capture, or the editing bar while a line has the caret

    /// One glass container, so the tab capsule and the editing capsule (and Capture and Done)
    /// morph into each other when editing starts and ends.
    private var bottomBar: some View {
        GlassEffectContainer(spacing: T.q3) {
            HStack(spacing: T.q3) {
                if let id = editing {
                    editBar(id)
                        .glassEffect(.regular.interactive(), in: .capsule)
                        .glassEffectID("bar", in: glass)
                    Button {
                        UIApplication.shared.sendAction(#selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil)
                        store.run("focus.clear")
                    } label: {
                        Image(systemName: "checkmark").font(.system(size: 19, weight: .semibold)).frame(width: 30, height: 30)
                    }
                    .buttonStyle(.glassProminent)
                    .buttonBorderShape(.circle)
                    .tint(T.cFocus)
                    .glassEffectID("action", in: glass)
                    .accessibilityIdentifier("kb.hide")
                    .accessibilityLabel("Done")
                } else {
                    HStack(spacing: 0) {
                        tab("Browse", "square.grid.2x2", "nav.home") { sheet = .home }
                        tab("Supertags", "number", "nav.supertags") { sheet = .supertags }
                        tab("Search", "magnifyingglass", "nav.search") { sheet = .search }
                        tab("Manage", "gearshape", "nav.manage") { sheet = .manage }
                    }
                    .padding(.horizontal, T.q1)
                    .glassEffect(.regular.interactive(), in: .capsule)
                    .glassEffectID("bar", in: glass)
                    Button { sheet = .capture } label: {
                        Image(systemName: "plus").font(.system(size: 22, weight: .semibold)).frame(width: 34, height: 34)
                    }
                    .buttonStyle(.glassProminent)
                    .buttonBorderShape(.circle)
                    .tint(T.cQuickAdd)
                    .glassEffectID("action", in: glass)
                    .accessibilityIdentifier("nav.capture")
                    .accessibilityLabel("Capture")
                }
            }
        }
        .animation(reduceMotion ? nil : .smooth(duration: 0.3), value: editing == nil)
        .padding(.horizontal, T.q4)
        .padding(.bottom, T.q1)
    }

    private func tab(_ title: String, _ symbol: String, _ id: String, _ go: @escaping () -> Void) -> some View {
        Button(action: go) {
            VStack(spacing: 2) {
                Image(systemName: symbol).font(.system(size: 18, weight: .medium)).frame(height: 24)
                Text(title).font(Fonts.font(T.fsMeta, T.fwButton))
            }
            .foregroundStyle(T.cStrong)
            .frame(maxWidth: .infinity, minHeight: 54)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier(id)
        .accessibilityLabel(title)
    }

    /// The outline moves a phone keyboard has no keys for (Tab, Shift+Tab, ...).
    private func editBar(_ id: Int64) -> some View {
        let items: [(String, String, String, () -> Void)] = [
            ("outdent", "decrease.indent", "Outdent", { store.run("node.outdent", ["node": id]) }),
            ("indent", "increase.indent", "Indent", { store.run("node.indent", ["node": id]) }),
            ("up", "arrow.up", "Move up", { store.run("node.moveUp", ["node": id]) }),
            ("down", "arrow.down", "Move down", { store.run("node.moveDown", ["node": id]) }),
            ("collapse", "chevron.up.chevron.down", "Expand or collapse", { store.toggleExpand(id) }),
            ("done", "checkmark.square", "Checkbox", { store.run("node.toggleDone", ["node": id]) }),
            ("tag", "number", "Add supertag", { tagPickerFor = id }),
            ("undo", "arrow.uturn.backward", "Undo", { store.run("edit.undo") }),
        ]
        return ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 0) {
                ForEach(items, id: \.0) { it in
                    Button(action: it.3) {
                        Image(systemName: it.1).font(.system(size: 17, weight: .medium)).frame(width: 36, height: 46).contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .foregroundStyle(T.cStrong)
                    .accessibilityIdentifier("kb.\(it.0)")
                    .accessibilityLabel(it.2)
                }
            }
            .padding(.horizontal, T.q1)
        }
        .frame(height: 54)
        .scrollClipDisabled(false)
    }

    @ViewBuilder private var errorBanner: some View {
        if let e = store.error {
            Text(e)
                .font(Fonts.font(Phone.dense, Fonts.textWeight(scheme)))
                .foregroundStyle(T.cSecondary)
                .padding(T.q3)
                .background(RoundedRectangle(cornerRadius: T.rXl).fill(T.cPopover))
                .padding(.top, T.topbarH + T.q2)
                .onTapGesture { store.clearError() }
                .task {
                    try? await Task.sleep(for: .seconds(4))
                    store.clearError()
                }
                .accessibilityIdentifier("status")
        }
    }
}

struct NodeRef: Identifiable {
    let id: Int64
}
