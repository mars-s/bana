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

    func testScreens() throws {
        guard ProcessInfo.processInfo.environment["SHOTS_DIR"] != nil else { throw XCTSkip("SHOTS_DIR not set") }
        let theme = ProcessInfo.processInfo.environment["SHOTS_THEME"] ?? "light"
        launch(sample: true)
        shot("today-\(theme)")
        btn("Open Volunteer meetup").tap()
        waitTitle("Volunteer meetup")
        line("Agree on the water rota").tap()
        shot("editing-\(theme)")
        app.buttons["kb.hide"].tap()
        app.buttons["nav.supertags"].tap()
        waitTitle("Supertags")
        btn("Open task").tap()
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
    }
}
