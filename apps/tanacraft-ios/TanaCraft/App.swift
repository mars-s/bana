import SwiftUI

@main
struct TanaCraftApp: App {
    @State private var store = Store.openDefault()
    @Environment(\.scenePhase) private var phase

    var body: some Scene {
        WindowGroup {
            RootView()
                .environment(store)
                .onReceive(NotificationCenter.default.publisher(for: UIApplication.willTerminateNotification)) { _ in store.save() }
        }
        // Leaving the foreground writes the file at once (the engine also autosaves within half a second).
        .onChange(of: phase) { _, p in
            if p != .active { store.save() }
        }
    }
}

enum Sheet: String, Identifiable {
    case home, search, capture
    var id: String { rawValue }
}

struct RootView: View {
    @Environment(Store.self) private var store
    @Environment(\.colorScheme) private var scheme
    @State private var sheet: Sheet?
    @State private var tagPickerFor: Int64?

    var body: some View {
        VStack(spacing: 0) {
            topBar
            if let page = store.page {
                PageView(page: page, tagPickerFor: $tagPickerFor)
                    .background(T.cPanel)
            } else {
                Spacer()
                Text(store.error ?? "Opening…").foregroundStyle(T.cMuted).padding()
                Spacer()
            }
            bottomBar
        }
        .background(T.cChrome.ignoresSafeArea())
        .overlay(alignment: .top) { errorBanner }
        .sheet(item: $sheet) { s in
            switch s {
            case .home: HomeSheet(close: { sheet = nil })
            case .search: SearchSheet(close: { sheet = nil })
            case .capture: CaptureSheet(close: { sheet = nil })
            }
        }
        .sheet(item: Binding(get: { tagPickerFor.map(NodeRef.init) }, set: { tagPickerFor = $0?.id })) { ref in
            TagPicker(node: ref.id, close: { tagPickerFor = nil })
        }
    }

    // MARK: bars

    private var topBar: some View {
        HStack(spacing: T.q2) {
            Button { sheet = .home } label: { Image(systemName: "sidebar.left") }
                .accessibilityIdentifier("nav.home")
                .accessibilityLabel("Home")
            Button { store.run("nav.back") } label: { Image(systemName: "chevron.backward") }
                .disabled(!(store.page?.canBack ?? false))
                .accessibilityIdentifier("nav.back")
                .accessibilityLabel("Back")
            crumbs
            Spacer(minLength: 0)
            Button { sheet = .search } label: { Image(systemName: "magnifyingglass") }
                .accessibilityIdentifier("nav.search")
                .accessibilityLabel("Search")
        }
        .font(.system(size: 17))
        .tint(T.cSecondary)
        .padding(.horizontal, T.q4)
        .frame(height: T.topbarH)
        .background(T.cChrome)
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
            .font(Fonts.font(Phone.label, Fonts.textWeight(scheme)))
            .foregroundStyle(T.cMuted)
        }
        .defaultScrollAnchor(.trailing)
    }

    private var bottomBar: some View {
        HStack {
            Button { store.run("nav.today") } label: { Label("Today", systemImage: "calendar") }
                .accessibilityIdentifier("nav.today")
            Spacer()
            Button { store.run("nav.supertags") } label: { Label("Supertags", systemImage: "number") }
                .accessibilityIdentifier("nav.supertags")
            Spacer()
            Button { sheet = .capture } label: { Label("Capture", systemImage: "plus.circle.fill") }
                .accessibilityIdentifier("nav.capture")
                .tint(T.cQuickAdd)
        }
        .labelStyle(.iconOnly)
        .font(.system(size: 22))
        .tint(T.cSecondary)
        .padding(.horizontal, T.q8)
        .padding(.vertical, T.q2)
        .background(T.cChrome)
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
