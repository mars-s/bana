import XCTest

/// Writes screenshots of the main screens to $SHOTS_DIR (set with TEST_RUNNER_SHOTS_DIR, see
/// apps/tanacraft-ios/README.md). Skipped when it is not set.
final class ScreenshotTests: UITestCase {
    func shot(_ name: String) {
        guard let dir = ProcessInfo.processInfo.environment["SHOTS_DIR"] else { return }
        sleep(1)
        let png = XCUIScreen.main.screenshot().pngRepresentation
        let url = URL(fileURLWithPath: dir).appendingPathComponent("\(name).png")
        XCTAssertNoThrow(try png.write(to: url))
    }

    /// A slow drag on the page (in fractions of the screen height), so it stops where it was let go.
    func drag(from: CGFloat, to: CGFloat) {
        let a = app.coordinate(withNormalizedOffset: CGVector(dx: 0.6, dy: from))
        a.press(forDuration: 0.05, thenDragTo: app.coordinate(withNormalizedOffset: CGVector(dx: 0.6, dy: to)), withVelocity: .slow, thenHoldForDuration: 0.3)
    }

    func testScreens() throws {
        guard ProcessInfo.processInfo.environment["SHOTS_DIR"] != nil else { throw XCTSkip("SHOTS_DIR not set") }
        let theme = ProcessInfo.processInfo.environment["SHOTS_THEME"] ?? "light"
        launch(sample: true)
        shot("today-\(theme)")
        // A longer page scrolled so the outline runs under the glass bars.
        for name in ["Community garden", "Raised beds", "Seed list", "Water rota", "Ask the hardware store about bulk compost"] {
            if btn("Expand \(name)").waitForExistence(timeout: 1) { btn("Expand \(name)").tap() }
        }
        drag(from: 0.75, to: 0.45)
        shot("scrolled-\(theme)")
        drag(from: 0.3, to: 0.9)
        btn("Open Volunteer meetup").tap()
        waitTitle("Volunteer meetup")
        line("Agree on the water rota").tap()
        shot("editing-\(theme)")
        app.buttons["kb.hide"].tap()
        app.buttons["nav.supertags"].tap()
        XCTAssertTrue(app.buttons["supertag.task"].waitForExistence(timeout: 3))
        shot("supertags-\(theme)")
        app.buttons["supertag.task"].tap()
        waitTitle("task")
        shot("supertag-\(theme)")
        app.buttons["nav.search"].tap()
        app.searchFields.firstMatch.tap()
        app.searchFields.firstMatch.typeText("com")
        shot("search-\(theme)")
        app.buttons.matching(identifier: "search.result").firstMatch.tap()
        waitTitle("Community garden")
        shot("project-\(theme)")
        app.buttons["nav.home"].tap()
        XCTAssertTrue(app.buttons["home.supertags"].waitForExistence(timeout: 3))
        shot("home-\(theme)")
        app.buttons["Done"].tap()
        app.buttons["nav.capture"].tap()
        shot("capture-\(theme)")
        app.buttons["Cancel"].tap()
        app.buttons["nav.manage"].tap()
        shot("manage-\(theme)")
    }
}
