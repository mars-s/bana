import CoreText
import SwiftUI
import UIKit

/// Inter Variable (bundled, SIL OFL; see ATTRIBUTION.md) at the design's in-between weights
/// (375, 450, 550), set through the font's weight axis. Falls back to the system font.
enum Fonts {
    private static let base: CTFontDescriptor? = {
        guard let url = Bundle.main.url(forResource: "InterVariable", withExtension: "ttf"),
              let descs = CTFontManagerCreateFontDescriptorsFromURL(url as CFURL) as? [CTFontDescriptor]
        else { return nil }
        return descs.first
    }()

    /// 'wght'
    private static let weightAxis = 0x7767_6874

    static func ui(_ size: CGFloat, _ weight: CGFloat) -> UIFont {
        guard let base else {
            return .systemFont(ofSize: size, weight: weight >= 550 ? .semibold : weight >= 450 ? .medium : .regular)
        }
        let attrs = [kCTFontVariationAttribute: [weightAxis: weight]] as CFDictionary
        let desc = CTFontDescriptorCreateCopyWithAttributes(base, attrs)
        return CTFontCreateWithFontDescriptor(desc, size, nil) as UIFont
    }

    static func font(_ size: CGFloat, _ weight: CGFloat) -> Font { Font(ui(size, weight)) }

    /// Regular text weight: 375 in dark, 400 in light (DESIGN.md).
    static func textWeight(_ scheme: ColorScheme) -> CGFloat { scheme == .dark ? T.fwText : T.fwTextLight }
    static func textWeight(_ traits: UITraitCollection) -> CGFloat { traits.userInterfaceStyle == .dark ? T.fwText : T.fwTextLight }
}

/// The phone uses the desktop type scale (DESIGN.md) one step up so text reads at arm's length.
/// spec-gap: no phone measurements yet (tanacraft-spec/ios/ does not exist); 17 / 15 is the iOS
/// body size over the desktop body size.
enum Phone {
    static let scale: CGFloat = 17.0 / 15.0
    static var body: CGFloat { T.fsBody * scale }
    static var title: CGFloat { T.fsTitle }
    static var label: CGFloat { T.fsLabel * scale }
    static var dense: CGFloat { T.fsDense * scale }
    static var rowH: CGFloat { T.rowH * scale }
    static var bullet: CGFloat { T.bullet * scale }
    static var dot: CGFloat { T.dot * scale }
    /// Desktop indents 32.25 per level; a phone column is a third as wide.
    static var indent: CGFloat { T.q6 }
}
