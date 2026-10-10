import XCTest

/// Pairing with the desktop through a relay on this Mac (`wrangler dev`), run by
/// `scripts/ios-test.sh sync`: the script starts the relay and a desktop engine (`bana serve`),
/// sets sync up there and passes a fresh pairing code, the engine's address and its local token
/// (SYNC_CODE, SYNC_DESK, SYNC_TOKEN). Without them (a plain `scripts/ios-test.sh`) it is skipped.
final class SyncUITests: UITestCase {
    private var env: [String: String] { ProcessInfo.processInfo.environment }

    /// One request to the desktop engine, as a local tool sends it (with the install's token).
    private func desk(_ method: String, _ params: [String: Any] = [:]) throws -> [String: Any] {
        let url = try XCTUnwrap(URL(string: try XCTUnwrap(env["SYNC_DESK"]) + "rpc"))
        var req = URLRequest(url: url)
        req.httpMethod = "POST"
        req.setValue("Bearer \(env["SYNC_TOKEN"] ?? "")", forHTTPHeaderField: "Authorization")
        req.httpBody = try JSONSerialization.data(withJSONObject: ["method": method, "params": params])
        var out: Data?
        let done = expectation(description: method)
        URLSession.shared.dataTask(with: req) { data, _, _ in
            out = data
            done.fulfill()
        }.resume()
        wait(for: [done], timeout: 10)
        return try XCTUnwrap(JSONSerialization.jsonObject(with: try XCTUnwrap(out)) as? [String: Any])
    }

    func testPairWithAPastedCodeAndSeeADesktopEditArrive() throws {
        guard let code = env["SYNC_CODE"], !code.isEmpty else { throw XCTSkip("run with scripts/ios-test.sh sync") }
        launch(sample: false)
        app.buttons["nav.manage"].tap()
        let field = app.textFields["sync.code"].exists ? app.textFields["sync.code"] : app.textViews["sync.code"]
        XCTAssertTrue(field.waitForExistence(timeout: 5))
        // Pasted, as from the clipboard (typed here: the simulator's pasteboard is not the test's).
        field.tap()
        field.typeText(code)
        app.buttons["sync.join"].tap()
        let state = app.staticTexts["sync.state"]
        XCTAssertTrue(state.waitForExistence(timeout: 20), "joined")
        let deadline = Date().addingTimeInterval(20)
        while !state.label.hasSuffix("Synced") && Date() < deadline { usleep(200_000) }
        XCTAssertTrue(state.label.hasSuffix("Synced"), state.label)
        XCTAssertGreaterThanOrEqual(app.descendants(matching: .any).matching(identifier: "sync.device").count, 2)
        app.buttons["Done"].firstMatch.tap()
        // The phone now has the desktop's notes: today's page holds the desktop's lines.
        XCTAssertTrue(line("Seed swap at the library, bring spare bean seeds").waitForExistence(timeout: 10))

        // An edit made on the desktop now arrives while the phone is open.
        _ = try desk("run", ["command": "nav.today", "params": [:]])
        let page = try desk("page", [:])
        let today = try XCTUnwrap(page["zoom"] as? Int)
        _ = try desk("run", ["command": "node.add", "params": ["name": "Typed on the desktop", "parent": today]])
        XCTAssertTrue(line("Typed on the desktop").waitForExistence(timeout: 20), "the desktop's edit arrived")

        // And one made here reaches the desktop.
        newLine("Typed on the phone")
        let back = Date().addingTimeInterval(20)
        var names: [String] = []
        while Date() < back {
            let kids = try desk("run", ["command": "graph.inspect", "params": ["node": today]])
            names = ((kids["result"] as? [String: Any])?["children"] as? [[String: Any]] ?? []).compactMap { $0["name"] as? String }
            if names.contains("Typed on the phone") { break }
            usleep(300_000)
        }
        XCTAssertTrue(names.contains("Typed on the phone"), "\(names)")
    }
}
