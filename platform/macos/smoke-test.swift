// Loads OpenFlip.saver the way macOS does and runs it briefly: the
// screensaver view, then the Options sheet, which is closed with Escape.
//
//   OPENFLIP_LOG=1 swift platform/macos/smoke-test.swift target/macos/OpenFlip.saver

import AppKit
import ScreenSaver

func fail(_ message: String) -> Never {
    FileHandle.standardError.write("smoke test failed: \(message)\n".data(using: .utf8)!)
    exit(1)
}

func pump(seconds: Double) {
    RunLoop.main.run(until: Date().addingTimeInterval(seconds))
}

guard CommandLine.arguments.count == 2 else { fail("usage: smoke-test.swift <OpenFlip.saver>") }
let app = NSApplication.shared
app.setActivationPolicy(.accessory)
app.finishLaunching()

guard let bundle = Bundle(path: CommandLine.arguments[1]) else { fail("not a bundle") }
guard bundle.load() else { fail("bundle does not load") }
guard let saverClass = bundle.principalClass as? ScreenSaverView.Type else {
    fail("principal class is not a ScreenSaverView")
}
print("loaded \(saverClass)")

let frame = NSRect(x: 0, y: 0, width: 640, height: 360)
let window = NSWindow(contentRect: frame, styleMask: [.titled], backing: .buffered, defer: false)
guard let view = saverClass.init(frame: frame, isPreview: true) else { fail("view init failed") }
window.contentView = view
window.orderFront(nil)
view.startAnimation()
for _ in 0..<90 {
    view.animateOneFrame()
    pump(seconds: 1.0 / 60.0)
}
view.stopAnimation()
print("screensaver view ran 90 frames")

guard view.hasConfigureSheet, let sheet = view.configureSheet else { fail("no configure sheet") }
sheet.orderFront(nil)
pump(seconds: 0.5)
guard let responder = sheet.firstResponder as? NSView else { fail("sheet has no first responder") }
let escape = NSEvent.keyEvent(
    with: .keyDown, location: .zero, modifierFlags: [], timestamp: 0,
    windowNumber: sheet.windowNumber, context: nil, characters: "\u{1b}",
    charactersIgnoringModifiers: "\u{1b}", isARepeat: false, keyCode: 53
)!
responder.keyDown(with: escape)
pump(seconds: 0.2)
guard !sheet.isVisible else { fail("Escape did not close the settings sheet") }
print("settings sheet opened and closed with Escape")
print("OK")
