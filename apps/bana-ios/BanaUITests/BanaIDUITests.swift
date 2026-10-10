import XCTest

/// Signing in with Bana ID on the phone, run by `scripts/ios-test.sh id`: the script starts the
/// relay (`wrangler dev`, relying party localhost), a desktop engine that created a Bana ID, and
/// scripts/id-helper.mjs, which stands in for the passkey sheet (the simulator cannot make
/// passkeys in a UI test). Everything after the sheet is real: the one-time code exchange, the
/// PRF unmasking, unwrapping the key and joining sync. Without the script it is skipped.
final class BanaIDUITests: UITestCase {
    private var env: [String: String] { ProcessInfo.processInfo.environment }

    func testSignInWithBanaIDAndGetTheDesktopsNotes() throws {
        guard let helper = env["ID_HELPER"], let relay = env["ID_RELAY"] else { throw XCTSkip("run with scripts/ios-test.sh id") }
        launch(sample: false, extra: ["-idHelper", helper])
        app.buttons["nav.manage"].tap()
        app.buttons["manage.profile"].tap()
        let field = app.textFields["id.relay"]
        XCTAssertTrue(field.waitForExistence(timeout: 5))
        field.tap()
        field.typeText(relay)
        app.buttons["id.signin"].tap()
        XCTAssertTrue(app.staticTexts["id.signedIn"].waitForExistence(timeout: 30), "signed in")
        // Both devices are on the account.
        let devices = app.descendants(matching: .any).matching(identifier: "id.device")
        let deadline = Date().addingTimeInterval(10)
        while devices.count < 2 && Date() < deadline { usleep(200_000) }
        XCTAssertGreaterThanOrEqual(devices.count, 2)
        app.navigationBars.buttons.element(boundBy: 0).tap()
        app.buttons["Done"].firstMatch.tap()
        // The desktop's notes arrived through sync.
        XCTAssertTrue(line("Seed swap at the library, bring spare bean seeds").waitForExistence(timeout: 30))
    }
}
