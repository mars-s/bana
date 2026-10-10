import XCTest

/// Core flows on the simulator. Each test starts from a fresh graph file (`-reset YES`) with the
/// date fixed to 2026-10-10; `sample` starts it from the built-in sample graph.
class UITestCase: XCTestCase {
    var app: XCUIApplication!

    override func setUp() {
        continueAfterFailure = false
        app = XCUIApplication()
    }

    func launch(sample: Bool, reset: Bool = true) {
        app.launchArguments = ["-today", "2026-10-10", "-sample", sample ? "YES" : "NO", "-reset", reset ? "YES" : "NO"]
        app.launch()
        dismissSystemAlerts()
        XCTAssertTrue(app.descendants(matching: .any)["title"].waitForExistence(timeout: 10))
    }

    /// The simulator sometimes shows a system prompt (another app's) over ours.
    func dismissSystemAlerts() {
        let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        for label in ["Don’t Allow", "Don't Allow", "Not Now", "OK"] {
            let b = springboard.buttons[label]
            if b.exists { b.tap() }
        }
    }

    func line(_ text: String) -> XCUIElement {
        app.textViews.matching(NSPredicate(format: "identifier == 'line' AND value == %@", text)).firstMatch
    }

    var title: String {
        let t = app.descendants(matching: .any)["title"]
        return (t.value as? String).flatMap { $0.isEmpty ? nil : $0 } ?? t.label
    }

    func btn(_ label: String) -> XCUIElement {
        app.buttons.matching(NSPredicate(format: "label == %@", label)).firstMatch
    }

    func waitTitle(_ want: String, file: StaticString = #filePath, line: UInt = #line) {
        let deadline = Date().addingTimeInterval(5)
        while Date() < deadline {
            if title == want { return }
            usleep(100_000)
        }
        XCTAssertEqual(title, want, file: file, line: line)
    }

    func gone(_ e: XCUIElement, timeout: TimeInterval = 3) -> Bool {
        e.waitForNonExistence(timeout: timeout)
    }

    func newLine(_ text: String) {
        app.buttons["page.addLine"].tap()
        app.typeText(text)
    }

}

final class TanaCraftUITests: UITestCase {
    // MARK: editing

    func testTypeReturnIndentCollapse() {
        launch(sample: false)
        newLine("Groceries")
        app.typeText("\n")
        app.typeText("Milk")
        app.buttons["kb.indent"].tap()
        XCTAssertTrue(line("Milk").waitForExistence(timeout: 3))
        // Collapse the parent: the child is hidden; expand: back.
        btn("Collapse Groceries").tap()
        XCTAssertTrue(gone(line("Milk")))
        btn("Expand Groceries").tap()
        XCTAssertTrue(line("Milk").waitForExistence(timeout: 3))
        // Outdent puts it back on the page level (it stays visible after collapsing Groceries).
        line("Milk").tap()
        app.buttons["kb.outdent"].tap()
        XCTAssertTrue(gone(btn("Collapse Groceries")))
        XCTAssertTrue(line("Milk").exists)
    }

    func testBackspaceOnEmptyLineGoesBackUp() {
        launch(sample: false)
        newLine("x")
        app.typeText("\n")
        XCTAssertTrue(line("").waitForExistence(timeout: 3))
        // Backspace at the start of the empty new line removes it and the caret goes to the end of "x".
        app.typeText(XCUIKeyboardKey.delete.rawValue)
        app.typeText("y")
        XCTAssertTrue(line("xy").waitForExistence(timeout: 3))
        XCTAssertFalse(line("").exists)
    }

    func testCheckboxAndTag() {
        launch(sample: false)
        newLine("Water the beds")
        app.buttons["kb.done"].tap()
        XCTAssertTrue(app.buttons["checkbox"].waitForExistence(timeout: 3))
        XCTAssertEqual(app.buttons["checkbox"].value as? String, "not done")
        app.buttons["kb.tag"].tap()
        let search = app.searchFields.firstMatch
        XCTAssertTrue(search.waitForExistence(timeout: 3))
        search.tap()
        search.typeText("chore")
        btn("Create #chore").tap()
        XCTAssertTrue(app.buttons["chip.chore"].waitForExistence(timeout: 3))
    }

    // MARK: navigation

    func testZoomInAndBack() {
        launch(sample: true)
        XCTAssertTrue(title.hasPrefix("Today"))
        btn("Open Volunteer meetup").tap()
        XCTAssertTrue(line("Agenda").waitForExistence(timeout: 3))
        waitTitle("Volunteer meetup")
        app.buttons["nav.back"].tap()
        waitTitle("Today, Sat, 10 Oct")
    }

    func testFieldsShow() {
        launch(sample: true)
        XCTAssertTrue(app.descendants(matching: .any)["field"].waitForExistence(timeout: 3))
        XCTAssertTrue(app.staticTexts["Outdoors"].exists)
    }

    func testDayStepping() {
        launch(sample: true)
        app.buttons["day.previous"].tap()
        waitTitle("Yesterday, Fri, 9 Oct")
        app.buttons["nav.today"].tap()
        waitTitle("Today, Sat, 10 Oct")
    }

    func testSearchOpens() {
        launch(sample: true)
        app.buttons["nav.search"].tap()
        let search = app.searchFields.firstMatch
        XCTAssertTrue(search.waitForExistence(timeout: 3))
        search.tap()
        search.typeText("cedar")
        let hit = app.buttons.matching(identifier: "search.result").firstMatch
        XCTAssertTrue(hit.waitForExistence(timeout: 3))
        hit.tap()
        waitTitle("Order cedar boards")
    }

    /// Inline marks are hidden while a line is not being edited, and come back to edit.
    func testFormattedLineShowsWithoutMarks() {
        launch(sample: true)
        app.buttons["nav.search"].tap()
        app.searchFields.firstMatch.tap()
        app.searchFields.firstMatch.typeText("Reading notes")
        app.buttons.matching(identifier: "search.result").firstMatch.tap()
        waitTitle("Reading notes")
        let shown = line("Mulch keeps moisture in and weeds out, most of the time")
        XCTAssertTrue(shown.waitForExistence(timeout: 3))
        shown.tap()
        XCTAssertTrue(line("Mulch keeps **moisture** in and ==weeds== out, *most* of the time").waitForExistence(timeout: 3))
    }

    func testOptionsFieldPicker() {
        launch(sample: true)
        app.buttons["nav.search"].tap()
        app.searchFields.firstMatch.tap()
        app.searchFields.firstMatch.typeText("Community garden")
        app.buttons.matching(identifier: "search.result").firstMatch.tap()
        waitTitle("Community garden")
        btn("In progress").tap()
        app.buttons["Done"].tap()
        XCTAssertTrue(btn("Done").waitForExistence(timeout: 3))
        XCTAssertTrue(gone(btn("In progress")))
    }

    func testDeleteFromNodeMenu() {
        launch(sample: false)
        newLine("Throw me away")
        app.buttons["kb.hide"].tap()
        btn("Open Throw me away").press(forDuration: 1.2)
        app.buttons["Delete"].tap()
        XCTAssertTrue(gone(line("Throw me away")))
    }

    func testQuickCapture() {
        launch(sample: false)
        app.buttons["nav.capture"].tap()
        let field = app.textFields["capture.text"].exists ? app.textFields["capture.text"] : app.textViews["capture.text"]
        XCTAssertTrue(field.waitForExistence(timeout: 3))
        field.typeText("Call the plumber")
        app.buttons["capture.add"].tap()
        XCTAssertTrue(line("Call the plumber").waitForExistence(timeout: 3))
    }

    func testSupertagPage() {
        launch(sample: true)
        app.buttons["nav.supertags"].tap()
        XCTAssertTrue(app.buttons["supertag.task"].waitForExistence(timeout: 3))
        app.buttons["supertag.task"].tap()
        waitTitle("task")
        XCTAssertTrue(app.buttons.matching(identifier: "viewItem").firstMatch.waitForExistence(timeout: 3))
        XCTAssertGreaterThanOrEqual(app.buttons.matching(identifier: "viewItem").count, 4)
    }

    func testCaptureToTomorrow() {
        launch(sample: false)
        app.buttons["nav.capture"].tap()
        let field = app.textViews["capture.text"].exists ? app.textViews["capture.text"] : app.textFields["capture.text"]
        XCTAssertTrue(field.waitForExistence(timeout: 3))
        field.typeText("Buy seeds")
        app.buttons["capture.destination"].tap()
        app.buttons["Tomorrow"].tap()
        app.buttons["capture.add"].tap()
        XCTAssertFalse(line("Buy seeds").waitForExistence(timeout: 1), "not on today")
        app.buttons["day.next"].tap()
        waitTitle("Tomorrow, Sun, 11 Oct")
        XCTAssertTrue(line("Buy seeds").waitForExistence(timeout: 3))
    }

    func testManageTheme() {
        launch(sample: true)
        app.buttons["nav.manage"].tap()
        XCTAssertTrue(app.buttons["Dark"].waitForExistence(timeout: 3))
        app.buttons["Dark"].tap()
        XCTAssertTrue(app.buttons["Dark"].isSelected)
        app.buttons["System"].tap()
    }

    func testHomeSheet() {
        launch(sample: true)
        app.buttons["nav.home"].tap()
        XCTAssertTrue(app.buttons["home.supertags"].waitForExistence(timeout: 3))
        app.buttons["home.supertags"].tap()
        waitTitle("Supertags")
    }

    // MARK: data safety

    /// Type, send the app to the background, kill it, launch again: every character is there.
    func testTypedTextSurvivesBackgroundAndKill() {
        launch(sample: false)
        newLine("Do not lose me 123")
        XCUIDevice.shared.press(.home)
        sleep(1)
        app.terminate()
        launch(sample: false, reset: false)
        XCTAssertTrue(line("Do not lose me 123").waitForExistence(timeout: 5))
    }

    /// Killed in the foreground (no background save): the autosave has it within a second.
    func testTypedTextSurvivesKillInForeground() {
        launch(sample: false)
        newLine("Autosaved line")
        sleep(2)
        app.terminate()
        launch(sample: false, reset: false)
        XCTAssertTrue(line("Autosaved line").waitForExistence(timeout: 5))
    }
}
