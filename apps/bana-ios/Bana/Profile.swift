import AuthenticationServices
import PhotosUI
import SwiftUI
import UniformTypeIdentifiers

// The person (docs/bana-id.md): onboarding on first launch, Manage > Profile, and Bana ID, the
// optional sign-in for sync and recovery. Passkeys belong to the relay's host, so signing in runs
// on a page the relay serves, in the system sign-in sheet (ASWebAuthenticationSession: no
// Associated Domains needed, so a free Apple account works). The page comes back through
// `bana://auth/callback` with a one-time code; the engine exchanges it and unlocks the key.

struct Profile: Decodable, Equatable {
    let name: String
    let avatar: String?
    let onboarded: Bool
    let me: Int64?
}

struct IdStatus: Decodable, Equatable {
    let signedIn: Bool?
    let account: String?
    let relay: String?
    let defaultRelay: String?
    let syncOn: Bool?
    let pending: String?
    let note: String?
    let recoveryCode: String?
    let unavailable: Bool?
}

struct IdAccount: Decodable, Equatable {
    struct Passkey: Decodable, Equatable, Identifiable {
        let id: String
        let created: Double
        let used: Double
        let unlocks: Bool
    }

    struct Device: Decodable, Equatable, Identifiable {
        let sid: String
        let name: String
        let seen: Double
        let me: Bool
        var id: String { sid }
    }

    let recovery: Bool
    let relay: String?
    let credentials: [Passkey]
    let devices: [Device]
}

extension Store {
    func readProfile() {
        let p = decode(Profile.self, "profile.get")
        if p != profile { profile = p }
    }

    func readId() {
        let s = decode(IdStatus.self, "id.status")
        if s != bana { bana = s }
    }

    /// Saves part of the profile; answers the error, if any.
    @discardableResult
    func saveProfile(_ change: [String: Any]) -> String? {
        let out = rpc("profile.set", change) as? [String: Any]
        readProfile()
        if change["name"] != nil { refresh() }
        return out?["error"] as? String
    }

    /// An `id.*` request off the main thread (some talk to the relay); answers the reply.
    func idCall(_ method: String, _ params: [String: Any] = [:]) async -> [String: Any] {
        guard let engine else { return ["error": "no engine"] }
        let body = (try? JSONSerialization.data(withJSONObject: params)).flatMap { String(data: $0, encoding: .utf8) } ?? "{}"
        let out = await Task.detached { engine.rpc(method: method, paramsJson: body) }.value
        let v = (try? JSONSerialization.jsonObject(with: Data(out.utf8)) as? [String: Any]) ?? [:]
        readId()
        readSync()
        return v
    }

    /// Creates a Bana ID, signs in, or adds a passkey through the relay's page. Answers an error.
    func passkey(_ mode: String, relay: String?) async -> String? {
        var params: [String: Any] = ["mode": mode, "ret": "bana", "name": UIDevice.current.name, "person": profile?.name ?? ""]
        if let relay, !relay.isEmpty { params["relay"] = relay }
        let begun = await idCall("id.begin", params)
        if let e = begun["error"] as? String { return e }
        guard let s = begun["url"] as? String, let url = URL(string: s) else { return "the engine gave no address" }
        let callback: URL
        do {
            callback = try await WebAuth.shared.run(url)
        } catch {
            _ = await idCall("id.cancel")
            return (error as? ASWebAuthenticationSessionError)?.code == .canceledLogin ? nil : error.localizedDescription
        }
        let done = await idCall("id.finish", ["url": callback.absoluteString])
        if let e = done["error"] as? String { return e }
        await syncNow(wait: 15_000)
        return nil
    }
}

/// The system sign-in sheet. With `-idHelper <address>` (UI tests only, scripts/ios-test.sh id)
/// the page's address goes to scripts/id-helper.mjs instead, which runs the page's steps with a
/// software passkey: the simulator cannot make passkeys in a UI test.
@MainActor
final class WebAuth: NSObject, ASWebAuthenticationPresentationContextProviding {
    static let shared = WebAuth()
    private var session: ASWebAuthenticationSession?

    func run(_ url: URL) async throws -> URL {
        if let helper = UserDefaults.standard.string(forKey: "idHelper"), let h = URL(string: helper + "/complete") {
            var req = URLRequest(url: h)
            req.httpMethod = "POST"
            req.httpBody = try JSONSerialization.data(withJSONObject: ["url": url.absoluteString])
            let (data, _) = try await URLSession.shared.data(for: req)
            let v = try JSONSerialization.jsonObject(with: data) as? [String: Any]
            guard let c = (v?["callback"] as? String).flatMap(URL.init(string:)) else { throw URLError(.badServerResponse) }
            return c
        }
        return try await withCheckedThrowingContinuation { c in
            let s = ASWebAuthenticationSession(url: url, callback: .customScheme("bana")) { back, error in
                if let back { c.resume(returning: back) } else { c.resume(throwing: error ?? URLError(.cancelled)) }
            }
            s.presentationContextProvider = self
            session = s
            s.start()
        }
    }

    nonisolated func presentationAnchor(for session: ASWebAuthenticationSession) -> ASPresentationAnchor {
        MainActor.assumeIsolated {
            UIApplication.shared.connectedScenes.compactMap { ($0 as? UIWindowScene)?.keyWindow }.first ?? ASPresentationAnchor()
        }
    }
}

// MARK: pieces

func initials(_ name: String) -> String {
    let s = name.split(separator: " ").prefix(2).compactMap { $0.first.map { String($0).uppercased() } }.joined()
    return s.isEmpty ? "?" : s
}

struct AvatarView: View {
    let name: String
    let avatar: String?
    var size: CGFloat = 56

    var body: some View {
        ZStack {
            Circle().fill(T.cAvatarInner)
            Circle().strokeBorder(T.cAvatarRing, lineWidth: 1)
            if let a = avatar, let data = Data(base64Encoded: String(a.split(separator: ",").last ?? "")), let img = UIImage(data: data) {
                Image(uiImage: img).resizable().scaledToFill().clipShape(Circle())
            } else {
                Text(initials(name)).font(.system(size: size * 0.36, weight: .semibold)).foregroundStyle(T.cAvatarText)
            }
        }
        .frame(width: size, height: size)
        .accessibilityElement()
        .accessibilityIdentifier("profile.avatar")
        .accessibilityLabel(avatar == nil ? initials(name) : "Picture")
    }
}

/// A picked photo made small (128 px square JPEG) to keep beside the graph.
func avatarData(_ data: Data) -> String? {
    guard let img = UIImage(data: data) else { return nil }
    let side = min(img.size.width, img.size.height)
    let crop = CGRect(x: (img.size.width - side) / 2, y: (img.size.height - side) / 2, width: side, height: side)
    let out = UIGraphicsImageRenderer(size: CGSize(width: 128, height: 128)).image { _ in
        guard let cg = img.cgImage?.cropping(to: crop.applying(CGAffineTransform(scaleX: img.scale, y: img.scale))) else { return }
        UIImage(cgImage: cg).draw(in: CGRect(x: 0, y: 0, width: 128, height: 128))
    }
    return out.jpegData(compressionQuality: 0.85).map { "data:image/jpeg;base64," + $0.base64EncodedString() }
}

struct AvatarPicker: View {
    let name: String
    let avatar: String?
    let set: (String?) -> Void
    @State private var item: PhotosPickerItem?

    var body: some View {
        HStack(spacing: T.q4) {
            AvatarView(name: name, avatar: avatar)
            PhotosPicker(selection: $item, matching: .images) { Text("Pick a picture") }
                .accessibilityIdentifier("profile.pickPicture")
            if avatar != nil {
                Button("Use initials") { set(nil) }.accessibilityIdentifier("profile.useInitials")
            }
        }
        .buttonStyle(.borderless)
        .onChange(of: item) { _, it in
            Task {
                if let d = try? await it?.loadTransferable(type: Data.self), let a = avatarData(d) { set(a) }
                item = nil
            }
        }
    }
}

struct RecoveryCodeView: View {
    let code: String
    let saved: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: T.q3) {
            Text("Your recovery code").font(.headline)
            Text("Write it down or keep it somewhere safe. It is the only way back in if you lose every passkey or the relay's address changes. Bana shows it once.")
                .font(.footnote).foregroundStyle(T.cSecondary)
            Text(code)
                .font(.body.monospaced())
                .textSelection(.enabled)
                .accessibilityIdentifier("id.recoveryCode")
            HStack {
                Button("Copy") { UIPasteboard.general.string = code }
                Spacer()
                Button("I saved it", action: saved).accessibilityIdentifier("id.recoverySaved")
            }
            .buttonStyle(.borderless)
        }
    }
}

/// Create a Bana ID, sign in, recover, or pair with a code instead.
struct BanaIdStart: View {
    @Environment(Store.self) private var store
    var pair: () -> Void
    @State private var relay = ""
    @State private var recovery = ""
    @State private var busy = false
    @State private var problem: String?

    private var syncOn: Bool { store.bana?.syncOn ?? false }

    var body: some View {
        Group {
            Button {
                go("create")
            } label: {
                Label("Create Bana ID", systemImage: "person.badge.key")
            }
            .accessibilityIdentifier("id.create")
            if !syncOn {
                Button {
                    go("signin")
                } label: {
                    Label("Sign in with a passkey", systemImage: "key")
                }
                .accessibilityIdentifier("id.signin")
                TextField("Recovery code", text: $recovery)
                    .font(.body.monospaced())
                    .textInputAutocapitalization(.characters)
                    .autocorrectionDisabled()
                    .accessibilityIdentifier("id.recoveryField")
                Button("Recover with the code") {
                    busy = true
                    Task {
                        let r = await store.idCall("id.recover", ["relay": relay, "code": recovery, "name": UIDevice.current.name])
                        problem = r["error"] as? String
                        if problem == nil { await store.syncNow(wait: 15_000) }
                        busy = false
                    }
                }
                .disabled(busy || recovery.trimmingCharacters(in: .whitespaces).isEmpty)
                .accessibilityIdentifier("id.recover")
                TextField(store.bana?.defaultRelay ?? "Relay", text: $relay)
                    .font(.footnote)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .keyboardType(.URL)
                    .accessibilityIdentifier("id.relay")
            }
            Button {
                pair()
            } label: {
                Label("Pair with a code instead", systemImage: "qrcode")
            }
            .accessibilityIdentifier("id.pair")
            if busy {
                HStack { ProgressView(); Text("Use your passkey in the sheet…").font(.footnote) }
            }
            if let p = problem {
                Text(p).font(.footnote).foregroundStyle(T.cDanger).accessibilityIdentifier("id.problem")
            }
            Text("Signing in replaces this iPhone's notes with your synced ones (the old file stays beside them). Where a passkey cannot unlock notes (no PRF), pair with a code from your computer instead.")
                .font(.caption).foregroundStyle(T.cMuted)
        }
        .disabled(busy)
    }

    private func go(_ mode: String) {
        busy = true
        problem = nil
        Task {
            problem = await store.passkey(mode, relay: syncOn ? nil : relay)
            busy = false
        }
    }
}

// MARK: Manage > Profile

struct ProfileView: View {
    @Environment(Store.self) private var store
    var pair: () -> Void
    @State private var name = ""
    @State private var account: IdAccount?
    @State private var problem: String?
    @State private var newCode: String?
    @State private var renaming: IdAccount.Device?
    @State private var newName = ""
    @State private var confirmSignOut = false
    @State private var confirmDelete = false
    @State private var confirmNewCode = false

    private var signedIn: Bool { store.bana?.signedIn ?? false }
    private var host: String { URL(string: store.bana?.relay ?? "")?.host() ?? "" }

    var body: some View {
        Form {
            Section {
                AvatarPicker(name: name, avatar: store.profile?.avatar) { store.saveProfile(["avatar": $0 as Any]) }
                TextField("Your name", text: $name)
                    .accessibilityIdentifier("profile.name")
                    .onSubmit { if !name.trimmingCharacters(in: .whitespaces).isEmpty { store.saveProfile(["name": name]) } }
            } header: {
                Text("Name and picture")
            } footer: {
                Text("Shown as you in the app and in user fields; your #person node has this name.")
            }
            Section {
                if signedIn {
                    LabeledContent("Status") { Text("Signed in").accessibilityIdentifier("id.signedIn") }
                    LabeledContent("Relay", value: host)
                } else {
                    BanaIdStart(pair: pair)
                }
            } header: {
                Text("Bana ID")
            } footer: {
                if signedIn {
                    Text("Passkeys are tied to this address (\(host)). Renaming the Cloudflare workers.dev subdomain or the Worker would break them; the recovery code is the way back in.")
                } else {
                    Text("Optional: Bana works fully without it. A Bana ID uses passkeys (no passwords) to bring your encrypted notes to a new device and to get them back if you lose one.")
                }
            }
            if let code = store.bana?.recoveryCode ?? newCode {
                Section {
                    RecoveryCodeView(code: code) {
                        newCode = nil
                        Task { _ = await store.idCall("id.recoveryShown") }
                    }
                }
            }
            if signedIn, let a = account {
                Section("Passkeys") {
                    ForEach(Array(a.credentials.enumerated()), id: \.element.id) { i, c in
                        VStack(alignment: .leading) {
                            Text("Passkey \(i + 1)")
                            Text("Added \(Date(timeIntervalSince1970: c.created / 1000).formatted(date: .abbreviated, time: .omitted))" + (c.unlocks ? "" : " · cannot unlock notes (no PRF)"))
                                .font(.caption).foregroundStyle(T.cMuted)
                        }
                        .accessibilityIdentifier("id.passkey")
                        .swipeActions {
                            if a.credentials.count > 1 {
                                Button("Remove", role: .destructive) { call("id.removePasskey", ["id": c.id]) }
                            }
                        }
                    }
                    Button("Add another passkey") {
                        Task {
                            problem = await store.passkey("add", relay: nil)
                            load()
                        }
                    }
                }
                Section("Devices") {
                    ForEach(a.devices) { d in
                        HStack {
                            Text(d.name + (d.me ? " (this iPhone)" : ""))
                            Spacer()
                            Button("Rename") {
                                newName = d.name
                                renaming = d
                            }
                            .buttonStyle(.borderless)
                        }
                        .accessibilityIdentifier("id.device")
                        .swipeActions {
                            if !d.me {
                                Button("Remove", role: .destructive) { call("id.removeDevice", ["sid": d.sid]) }
                            }
                        }
                    }
                }
                Section {
                    Button("Make a new recovery code") { confirmNewCode = true }
                    Button("Sign out of this iPhone") { confirmSignOut = true }.accessibilityIdentifier("id.signOut")
                    Button("Delete Bana ID", role: .destructive) { confirmDelete = true }.accessibilityIdentifier("id.delete")
                } footer: {
                    Text("Signing out keeps the notes on this iPhone; it stops syncing. Deleting removes your Bana ID and its encrypted sync data from the relay; notes on your devices stay.")
                }
            }
            if let p = problem {
                Section { Text(p).font(.footnote).foregroundStyle(T.cDanger).accessibilityIdentifier("id.problem") }
            }
        }
        .scrollContentBackground(.hidden)
        .navigationTitle("Profile")
        .onAppear {
            name = store.profile?.name ?? ""
            store.readId()
            load()
        }
        .onChange(of: store.bana?.signedIn) { _, _ in load() }
        .alert("Rename device", isPresented: Binding(get: { renaming != nil }, set: { if !$0 { renaming = nil } })) {
            TextField("Name", text: $newName)
            Button("Save") { if let d = renaming { call("id.renameDevice", ["sid": d.sid, "name": newName]) } }
            Button("Cancel", role: .cancel) {}
        }
        .confirmationDialog("Make a new recovery code? The old one stops working.", isPresented: $confirmNewCode, titleVisibility: .visible) {
            Button("Make a new code") {
                Task {
                    let r = await store.idCall("id.newRecoveryCode")
                    problem = r["error"] as? String
                    newCode = r["recoveryCode"] as? String
                    load()
                }
            }
        }
        .confirmationDialog("Sign out of Bana ID on this iPhone? Your notes stay here.", isPresented: $confirmSignOut, titleVisibility: .visible) {
            Button("Sign out", role: .destructive) { call("id.signOut") }
        }
        .confirmationDialog("Delete your Bana ID and its sync data from the relay? This cannot be undone. Notes on your devices stay.", isPresented: $confirmDelete, titleVisibility: .visible) {
            Button("Delete Bana ID", role: .destructive) { call("id.delete", ["confirm": true]) }
        }
    }

    private func load() {
        guard signedIn else { account = nil; return }
        Task {
            let r = await store.idCall("id.account")
            if let e = r["error"] as? String { problem = e; return }
            account = (try? JSONSerialization.data(withJSONObject: r)).flatMap { try? JSONDecoder().decode(IdAccount.self, from: $0) }
        }
    }

    private func call(_ method: String, _ params: [String: Any] = [:]) {
        Task {
            let r = await store.idCall(method, params)
            problem = r["error"] as? String
            load()
        }
    }
}

// MARK: onboarding

/// Which importer reads a file: OPML, Roam JSON, Tana Paste, else Markdown.
func importCommand(_ name: String, _ text: String) -> String {
    let n = name.lowercased()
    if n.hasSuffix(".opml") { return "import.opml" }
    if n.hasSuffix(".json") { return "import.roam" }
    if text.trimmingCharacters(in: .whitespacesAndNewlines).hasPrefix("%%tana%%") { return "import.tanaPaste" }
    return "import.markdown"
}

/// First launch only (the profile says `onboarded`); every step can be skipped.
struct OnboardingView: View {
    @Environment(Store.self) private var store
    @AppStorage("theme") private var theme = "system"
    var pair: () -> Void
    @State private var step = 0
    @State private var name = ""
    @State private var avatar: String?
    @State private var importing = false
    private let steps = ["welcome", "you", "look", "start", "sync"]

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: T.q6) {
                    page
                }
                .padding(T.q6)
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            .scrollContentBackground(.hidden)
            .background(T.cPanel)
            .safeAreaBar(edge: .bottom) { actions }
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Skip setup") { finish() }.accessibilityIdentifier("onboard.skip")
                }
            }
        }
        .fileImporter(isPresented: $importing, allowedContentTypes: [.plainText, .json, .text, UTType(filenameExtension: "md") ?? .text, UTType(filenameExtension: "opml") ?? .xml], allowsMultipleSelection: true) { r in
            guard case let .success(urls) = r else { return }
            importFiles(urls)
            next()
        }
    }

    @ViewBuilder private var page: some View {
        switch steps[step] {
        case "welcome":
            title("Welcome to Bana")
            text("An outliner for your notes, supertags and daily pages. Everything stays on this iPhone: there is nothing to sign up for. A few quick choices, all of which you can change later in Manage.")
        case "you":
            title("What should Bana call you?")
            text("Your name shows as you on your notes and in user fields. It stays on your devices.")
            TextField("Your name", text: $name)
                .textFieldStyle(.roundedBorder)
                .accessibilityIdentifier("onboard.name")
            AvatarPicker(name: name, avatar: avatar) { avatar = $0 }
        case "look":
            title("Light or dark?")
            Picker("Appearance", selection: $theme) {
                Text("System").tag("system")
                Text("Light").tag("light")
                Text("Dark").tag("dark")
            }
            .pickerStyle(.segmented)
            .accessibilityIdentifier("onboard.theme")
        case "start":
            title("How do you want to start?")
            choice("Start fresh", "An empty Library and today's page.", id: "onboard.fresh") { next() }
            choice("Open the sample", "Projects, tasks and people to try things on.", id: "onboard.sample") {
                _ = store.rpc("onboarding.sample")
                store.refresh()
                next()
            }
            choice("Import files", "Tana Paste, Markdown (Notion or Logseq pages too), OPML or Roam JSON.", id: "onboard.import") { importing = true }
        default:
            title("Sync your devices?")
            text("Optional. With a Bana ID your notes reach your computer end-to-end encrypted, and you sign in with a passkey (Face ID), never a password. You can also pair with a code, or do this later in Manage.")
            if store.bana?.signedIn ?? false {
                Text("You are signed in.").accessibilityIdentifier("id.signedIn")
            } else {
                VStack(alignment: .leading, spacing: T.q3) { BanaIdStart(pair: { finish(); pair() }) }
                    .buttonStyle(.glass)
            }
            if let code = store.bana?.recoveryCode {
                RecoveryCodeView(code: code) { Task { _ = await store.idCall("id.recoveryShown") } }
            }
        }
    }

    private var actions: some View {
        GlassEffectContainer(spacing: T.q3) {
            HStack(spacing: T.q3) {
                if step > 0 {
                    Button("Back") { step -= 1 }.buttonStyle(.glass).accessibilityIdentifier("onboard.back")
                }
                Spacer()
                if step == steps.count - 1 {
                    Button("Done") { finish() }
                        .buttonStyle(.glassProminent)
                        .disabled(store.bana?.recoveryCode != nil)
                        .accessibilityIdentifier("onboard.done")
                } else {
                    Button(step == 0 ? "Get started" : "Continue") { next() }
                        .buttonStyle(.glassProminent)
                        .accessibilityIdentifier("onboard.next")
                }
            }
            .tint(T.cFocus)
            .padding(.horizontal, T.q4)
            .padding(.bottom, T.q1)
        }
    }

    private func title(_ s: String) -> some View {
        Text(s).font(.title2.weight(.semibold)).foregroundStyle(T.cStrong).accessibilityIdentifier("onboard.title")
    }

    private func text(_ s: String) -> some View {
        Text(s).font(.body).foregroundStyle(T.cSecondary)
    }

    private func choice(_ t: String, _ sub: String, id: String, _ go: @escaping () -> Void) -> some View {
        Button(action: go) {
            VStack(alignment: .leading, spacing: T.q1) {
                Text(t).font(.headline).foregroundStyle(T.cStrong)
                Text(sub).font(.footnote).foregroundStyle(T.cMuted)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(T.q4)
            .background(RoundedRectangle(cornerRadius: T.rLg).fill(T.cRaised))
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier(id)
    }

    private func next() { step = min(steps.count - 1, step + 1) }

    private func finish() {
        var change: [String: Any] = ["onboarded": true]
        if !name.trimmingCharacters(in: .whitespaces).isEmpty { change["name"] = name }
        if let avatar { change["avatar"] = avatar }
        store.saveProfile(change)
    }

    private func importFiles(_ urls: [URL]) {
        let lib = store.sidebar()?.workspaces.first?.library
        for url in urls {
            let scoped = url.startAccessingSecurityScopedResource()
            defer { if scoped { url.stopAccessingSecurityScopedResource() } }
            guard let text = try? String(contentsOf: url, encoding: .utf8) else { continue }
            let cmd = importCommand(url.lastPathComponent, text)
            // Several Markdown files (a Notion or Logseq export): a page each.
            if urls.count > 1, cmd == "import.markdown", let lib, let page = store.run("node.add", ["name": url.deletingPathExtension().lastPathComponent, "parent": lib], redraw: false)?["id"] {
                store.run(cmd, ["text": text, "parent": page], redraw: false)
            } else {
                store.run(cmd, ["text": text], redraw: false)
            }
        }
        store.refresh()
    }
}
