import AVFoundation
import SwiftUI
import UIKit

/// Manage > Sync: join the desktop's sync by scanning its QR code (or pasting its code), see the
/// state and the devices, remove one, turn sync off. The code holds the key to the notes: it is
/// read from the camera or the clipboard straight into the engine, never logged or stored here.
struct SyncSection: View {
    @Environment(Store.self) private var store
    @State private var code = ""
    @State private var scanning = false
    @State private var problem: String?
    @State private var devices: [SyncDevice] = []
    @State private var busy = false

    private var state: String { store.sync?.state ?? "off" }

    var body: some View {
        Section {
            if state == "off" {
                Text("Keep this iPhone and your computer in step. On the computer: Settings, Sync, Add a device. Then scan its code here, or copy it and paste it below.")
                    .font(.footnote)
                    .foregroundStyle(T.cSecondary)
                Button {
                    scanning = true
                } label: {
                    Label("Scan the code", systemImage: "qrcode.viewfinder")
                }
                .accessibilityIdentifier("sync.scan")
                TextField("Or paste the code", text: $code, axis: .vertical)
                    .lineLimit(1...3)
                    .font(.footnote.monospaced())
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .accessibilityIdentifier("sync.code")
                Button("Join") { join(code) }
                    .disabled(busy || code.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    .accessibilityIdentifier("sync.join")
                Text("The notes on this iPhone are replaced by the ones on the computer (the old file stays beside them).")
                    .font(.caption)
                    .foregroundStyle(T.cMuted)
            } else {
                LabeledContent("Status") {
                    HStack(spacing: T.q2) {
                        Circle().fill(dot).frame(width: 8, height: 8)
                        Text(label).accessibilityIdentifier("sync.state")
                    }
                }
                if let m = store.sync?.message, !m.isEmpty {
                    Text(m).font(.footnote).foregroundStyle(T.cSecondary).accessibilityIdentifier("sync.message")
                }
                if let t = store.sync?.lastSynced {
                    LabeledContent("Last synced", value: Date(timeIntervalSince1970: t / 1000).formatted(.relative(presentation: .named)))
                }
                Button("Sync now") { Task { await store.syncNow() } }
                    .accessibilityIdentifier("sync.now")
            }
            if let p = problem {
                Text(p).font(.footnote).foregroundStyle(T.cDanger).accessibilityIdentifier("sync.problem")
            }
        } header: {
            Text("Sync")
        } footer: {
            if state != "off" {
                Text("End-to-end encrypted: the relay keeps only data it cannot read.")
            }
        }
        .sheet(isPresented: $scanning) {
            QRScanner { found in
                scanning = false
                join(found)
            }
        }
        .onAppear { devices = store.syncDevices() }
        .onChange(of: store.sync) { _, _ in devices = store.syncDevices() }
        if state != "off" {
            Section("Devices") {
                ForEach(devices) { d in
                    HStack {
                        Text(d.name + (d.me ? " (this iPhone)" : "") + (d.revoked ? " (removed)" : ""))
                            .foregroundStyle(d.revoked ? T.cMuted : T.cText)
                        Spacer()
                    }
                    .accessibilityIdentifier("sync.device")
                    .swipeActions {
                        if !d.me && !d.revoked {
                            Button("Remove", role: .destructive) {
                                problem = store.syncCall("revoke", ["device": d.device])
                                devices = store.syncDevices()
                            }
                        }
                    }
                }
                Button("Turn sync off on this iPhone", role: .destructive) {
                    problem = store.syncCall("leave")
                    devices = []
                }
            }
        }
    }

    private var label: String {
        switch state {
        case "synced": "Synced"
        case "syncing": "Syncing"
        case "offline": "Offline"
        case "error": "Stopped"
        default: state
        }
    }

    private var dot: Color {
        switch state {
        case "synced": T.cDone
        case "syncing": T.cLink
        case "error": T.cDanger
        default: T.cMuted
        }
    }

    private func join(_ text: String) {
        busy = true
        problem = store.syncCall("join", ["code": text.trimmingCharacters(in: .whitespacesAndNewlines), "name": UIDevice.current.name])
        busy = false
        if problem == nil {
            code = ""
            Task { await store.syncNow(wait: 15_000) }
        }
    }
}

/// The camera, looking for a QR code (AVFoundation). Without a camera (the simulator) it says so.
struct QRScanner: View {
    let found: (String) -> Void
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            Group {
                if AVCaptureDevice.default(for: .video) == nil {
                    ContentUnavailableView("No camera", systemImage: "camera", description: Text("Paste the code instead."))
                } else {
                    CameraView(found: found).ignoresSafeArea()
                }
            }
            .navigationTitle("Scan the code")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } } }
        }
    }
}

private struct CameraView: UIViewControllerRepresentable {
    let found: (String) -> Void

    func makeUIViewController(context: Context) -> ScannerController {
        let c = ScannerController()
        c.found = found
        return c
    }

    func updateUIViewController(_ c: ScannerController, context: Context) {}
}

final class ScannerController: UIViewController, AVCaptureMetadataOutputObjectsDelegate {
    var found: (String) -> Void = { _ in }
    private let session = AVCaptureSession()
    private var done = false

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .black
        guard let device = AVCaptureDevice.default(for: .video), let input = try? AVCaptureDeviceInput(device: device), session.canAddInput(input) else { return }
        session.addInput(input)
        let output = AVCaptureMetadataOutput()
        guard session.canAddOutput(output) else { return }
        session.addOutput(output)
        output.setMetadataObjectsDelegate(self, queue: .main)
        output.metadataObjectTypes = [.qr]
        let preview = AVCaptureVideoPreviewLayer(session: session)
        preview.videoGravity = .resizeAspectFill
        preview.frame = view.layer.bounds
        view.layer.addSublayer(preview)
        let s = session
        DispatchQueue.global(qos: .userInitiated).async { s.startRunning() }
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        view.layer.sublayers?.first?.frame = view.layer.bounds
    }

    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        let s = session
        DispatchQueue.global(qos: .userInitiated).async { s.stopRunning() }
    }

    func metadataOutput(_ output: AVCaptureMetadataOutput, didOutput objects: [AVMetadataObject], from connection: AVCaptureConnection) {
        guard !done, let text = (objects.first as? AVMetadataMachineReadableCodeObject)?.stringValue, text.hasPrefix("bana-sync:") else { return }
        done = true
        UINotificationFeedbackGenerator().notificationOccurred(.success)
        found(text)
    }
}
