import XCTest

/// Onboarding on first launch and Manage > Profile (Profile.swift).
final class OnboardingUITests: UITestCase {
    private func onboardingTitle() -> XCUIElement { app.staticTexts["onboard.title"] }

    func testFirstLaunchNameLookSampleThenNeverAgain() {
        app.launchArguments = ["-today", "2026-10-10", "-sample", "NO", "-reset", "YES", "-onboarded", "NO"]
        app.launch()
        dismissSystemAlerts()
        XCTAssertTrue(onboardingTitle().waitForExistence(timeout: 10))
        XCTAssertEqual(onboardingTitle().label, "Welcome to Bana")
        app.buttons["onboard.next"].tap()

        let name = app.textFields["onboard.name"]
        XCTAssertTrue(name.waitForExistence(timeout: 3))
        name.tap()
        name.typeText("Ada Lovelace")
        XCTAssertEqual(app.descendants(matching: .any)["profile.avatar"].label, "AL")
        app.buttons["onboard.next"].tap()

        XCTAssertTrue(app.segmentedControls["onboard.theme"].waitForExistence(timeout: 3))
        app.segmentedControls["onboard.theme"].buttons["Dark"].tap()
        app.buttons["onboard.next"].tap()

        XCTAssertTrue(app.buttons["onboard.sample"].waitForExistence(timeout: 3))
        app.buttons["onboard.sample"].tap()

        XCTAssertTrue(app.buttons["id.create"].waitForExistence(timeout: 3), "the sync step offers a Bana ID")
        XCTAssertTrue(app.buttons["id.pair"].exists)
        app.buttons["onboard.done"].tap()
        XCTAssertTrue(gone(onboardingTitle(), timeout: 5))
        // The sample is open: today's page has its lines.
        XCTAssertTrue(line("Seed swap at the library, bring spare bean seeds").waitForExistence(timeout: 10))

        // The name is the profile's, shown in Manage.
        app.buttons["nav.manage"].tap()
        XCTAssertTrue(app.staticTexts["Ada Lovelace"].waitForExistence(timeout: 5))
        app.buttons["Done"].firstMatch.tap()

        // Relaunched on the same files: no onboarding.
        app.terminate()
        app.launchArguments = ["-today", "2026-10-10", "-sample", "NO", "-reset", "NO", "-onboarded", "NO"]
        app.launch()
        XCTAssertTrue(app.descendants(matching: .any)["title"].waitForExistence(timeout: 10))
        XCTAssertFalse(onboardingTitle().waitForExistence(timeout: 2))
    }

    func testSkipSetup() {
        app.launchArguments = ["-today", "2026-10-10", "-sample", "NO", "-reset", "YES", "-onboarded", "NO"]
        app.launch()
        dismissSystemAlerts()
        XCTAssertTrue(onboardingTitle().waitForExistence(timeout: 10))
        app.buttons["onboard.skip"].tap()
        XCTAssertTrue(gone(onboardingTitle(), timeout: 5))
        XCTAssertTrue(app.descendants(matching: .any)["title"].waitForExistence(timeout: 5))
    }

    func testProfilePageSignedOut() {
        launch(sample: true)
        app.buttons["nav.manage"].tap()
        app.buttons["manage.profile"].tap()
        let name = app.textFields["profile.name"]
        XCTAssertTrue(name.waitForExistence(timeout: 5))
        name.tap()
        name.typeText("Grace Hopper\n")
        XCTAssertTrue(app.descendants(matching: .any).matching(NSPredicate(format: "identifier == 'profile.avatar' AND label == 'GH'")).firstMatch.waitForExistence(timeout: 5))
        // Signed out: create, sign in, recover or pair.
        XCTAssertTrue(app.buttons["id.create"].exists)
        XCTAssertTrue(app.buttons["id.signin"].exists)
        XCTAssertTrue(app.buttons["id.pair"].exists)
        let code = app.textFields["id.recoveryField"]
        code.tap()
        code.typeText("not a code")
        app.buttons["id.recover"].tap()
        let problem = app.staticTexts["id.problem"]
        XCTAssertTrue(problem.waitForExistence(timeout: 10))
        XCTAssertTrue(problem.label.contains("recovery code"), problem.label)
    }
}
