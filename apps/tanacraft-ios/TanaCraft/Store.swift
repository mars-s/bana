import Foundation
import Observation

// The snapshots the engine sends (the same JSON the web UI draws; see crates/server/src/snapshot.rs).
// Decoding is lenient: every field the phone does not need is left out, the rest are optional.

struct Chip: Decodable, Hashable {
    let id: Int64
    let name: String
    let color: String?
}

struct ValueCell: Decodable, Hashable {
    let text: String
    let node: Int64?
    let checked: Bool?
}

struct Row: Decodable, Hashable {
    /// `node`, `field`, `empty`, `more` or `gone`.
    let k: String
    let key: String
    let depth: Int
    // node rows
    let id: Int64?
    let target: Int64?
    let name: String?
    let kind: String?
    let tags: [Chip]?
    let done: Bool?
    let hasKids: Bool?
    let expanded: Bool?
    let heading: Bool?
    let description: String?
    let progress: [Int]?
    // field rows
    let owner: Int64?
    let field: Int64?
    let type: String?
    let values: [ValueCell]?
    // empty rows
    let parent: Int64?
}

struct Crumb: Decodable, Hashable {
    let id: Int64
    let name: String
}

struct Page: Decodable {
    struct Rows: Decodable {
        let total: Int
        let items: [Row]
    }
    struct ViewInfo: Decodable {
        let kind: String
        let isView: Bool
    }
    let zoom: Int64
    let target: Int64
    let title: String
    let name: String
    let titleEditable: Bool
    let kind: String
    let tagColor: String?
    let description: String
    let tags: [Chip]
    let crumbs: [Crumb]
    let canBack: Bool
    let focus: Int64?
    let caret: Int
    let rows: Rows
    let refs: [String: String]
    let view: ViewInfo
    let status: String?
    let version: UInt64?
}

struct SidebarEntry: Decodable, Hashable {
    let id: Int64
    let name: String
    let kind: String
    let color: String?
}

struct Workspace: Decodable, Hashable {
    let id: Int64
    let name: String
    let library: Int64
    let trash: Int64
    let main: Bool
    let entries: [SidebarEntry]
}

struct Sidebar: Decodable {
    let todayDate: String
    let pinned: [SidebarEntry]
    let workspaces: [Workspace]
    let journal: Int64
    let schema: Int64
    let recents: [SidebarEntry]
}

struct FoundNode: Decodable, Hashable {
    let id: Int64
    let title: String
    let path: String
    let kind: String
    let tags: [Chip]
}

/// The one link between the screens and the engine. Screens read `page` and call `run`; nothing
/// else holds note text (the engine saves within half a second, and at once on `save()`).
@Observable
final class Store {
    private(set) var page: Page?
    private(set) var error: String?
    /// Bumped whenever `page` is replaced (editors compare names against it).
    private(set) var generation = 0
    let engine: Engine?
    @ObservationIgnored private var shown: UInt64 = 0
    @ObservationIgnored private let decoder = JSONDecoder()

    init(engine: Engine?, error: String? = nil) {
        self.engine = engine
        self.error = error
        engine?.subscribe(listener: Listener(store: self))
        refresh()
    }

    /// Opens the graph in the app's Documents folder (`graph.tanacraft`, the desktop format).
    /// Launch arguments (tests, screenshots): `-sample YES` starts a missing file from the sample
    /// graph, `-today 2026-10-10` fixes the date, `-reset YES` deletes the file first.
    static func openDefault() -> Store {
        let defaults = UserDefaults.standard
        let docs = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first ?? URL(fileURLWithPath: NSTemporaryDirectory())
        let file = docs.appendingPathComponent(defaults.string(forKey: "graph") ?? "graph.tanacraft")
        if defaults.bool(forKey: "reset") {
            try? FileManager.default.removeItem(at: file)
            for key in ["captureTo", "theme"] { defaults.removeObject(forKey: key) }
        }
        do {
            let engine = try Engine.open(path: file.path, sample: defaults.bool(forKey: "sample"), today: defaults.string(forKey: "today"))
            return Store(engine: engine)
        } catch {
            return Store(engine: nil, error: "Could not open \(file.lastPathComponent): \(error)")
        }
    }

    // MARK: engine calls

    func rpc(_ method: String, _ params: [String: Any] = [:]) -> Any? {
        guard let engine else { return nil }
        let body = (try? JSONSerialization.data(withJSONObject: params)).flatMap { String(data: $0, encoding: .utf8) } ?? "{}"
        let out = engine.rpc(method: method, paramsJson: body)
        return try? JSONSerialization.jsonObject(with: Data(out.utf8))
    }

    func decode<T: Decodable>(_ type: T.Type, _ method: String, _ params: [String: Any] = [:]) -> T? {
        guard let engine else { return nil }
        let body = (try? JSONSerialization.data(withJSONObject: params)).flatMap { String(data: $0, encoding: .utf8) } ?? "{}"
        let out = engine.rpc(method: method, paramsJson: body)
        do {
            return try decoder.decode(T.self, from: Data(out.utf8))
        } catch {
            self.error = "\(method): \(error)"
            return nil
        }
    }

    /// Runs a command and redraws. Answers the command's `result` (nil on error, shown as status).
    @discardableResult
    func run(_ command: String, _ params: [String: Any] = [:], redraw: Bool = true) -> [String: Any]? {
        let out = rpc("run", ["command": command, "params": params]) as? [String: Any]
        if let e = out?["error"] as? String {
            error = e
        }
        if redraw {
            refresh()
        }
        return out?["result"] as? [String: Any]
    }

    func refresh() {
        guard engine != nil else { return }
        if let p = decode(Page.self, "page", ["count": 2000]) {
            page = p
            shown = p.version ?? shown
            generation += 1
        }
    }

    /// Folds or unfolds `node`; a caret inside what folds away moves to `node` (it would type
    /// into a line nobody sees).
    func toggleExpand(_ node: Int64) {
        run("node.toggleExpand", ["node": node])
        if let p = page, let f = p.focus, !p.rows.items.contains(where: { $0.id == f }) {
            run("focus.set", ["node": node])
        }
    }

    func save() {
        do {
            try engine?.save()
        } catch {
            self.error = "\(error)"
        }
    }

    func clearError() { error = nil }

    func sidebar() -> Sidebar? { decode(Sidebar.self, "sidebar") }

    func find(_ query: String) -> [FoundNode] { decode([FoundNode].self, "find", ["query": query, "limit": 50]) ?? [] }

    /// Another request changed the graph (the autosave thread, a future sync): redraw on main.
    fileprivate func changed(_ version: UInt64) {
        DispatchQueue.main.async { [weak self] in
            guard let self, version > self.shown else { return }
            self.refresh()
        }
    }

    // MARK: names as people read them

    /// A line's text with inline references (`[[id:42]]`) shown as their names.
    func shown(_ name: String) -> String {
        guard name.contains("[[id:"), let page else { return name }
        var out = name
        for (id, ref) in page.refs {
            out = out.replacingOccurrences(of: "[[id:\(id)]]", with: ref)
        }
        return out
    }
}

private final class Listener: ChangeListener, @unchecked Sendable {
    weak var store: Store?
    init(store: Store) { self.store = store }
    func changed(version: UInt64) { store?.changed(version) }
}
