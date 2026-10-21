import AppKit
import OpenFlipFFI
import UniformTypeIdentifiers

/// The "Options…" sheet: the clock's own settings panel plus a Done button.
enum SettingsSheet {
    static func make() -> NSWindow {
        let width: CGFloat = 720
        let height: CGFloat = 520
        let bar: CGFloat = 52
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: width, height: height),
            styleMask: [.titled],
            backing: .buffered,
            defer: false
        )
        let content = NSView(frame: NSRect(x: 0, y: 0, width: width, height: height))

        let settings = SettingsView(frame: NSRect(x: 0, y: bar, width: width, height: height - bar))
        settings.autoresizingMask = [.width, .height]
        settings.onClose = { [weak window] in
            guard let window else { return }
            if let parent = window.sheetParent {
                parent.endSheet(window)
            } else {
                window.close()
            }
        }

        let done = NSButton(title: doneTitle, target: settings, action: #selector(SettingsView.close))
        done.bezelStyle = .rounded
        done.frame = NSRect(x: width - 116, y: 12, width: 100, height: 28)
        done.autoresizingMask = [.minXMargin, .maxYMargin]

        content.addSubview(settings)
        content.addSubview(done)
        window.contentView = content
        window.initialFirstResponder = settings
        return window
    }

    private static var doneTitle: String {
        Locale.preferredLanguages.first?.hasPrefix("de") == true ? "Fertig" : "Done"
    }
}

/// Shows the clock in settings-only mode and forwards keyboard and mouse.
final class SettingsView: NSView {
    private lazy var host = ClockHost(view: self, settingsOnly: true)
    private var timer: Timer?
    private var tracking: NSTrackingArea?
    var onClose: (() -> Void)?

    override var acceptsFirstResponder: Bool { true }
    override var isFlipped: Bool { true }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        timer?.invalidate()
        timer = nil
        guard let window else {
            host.stop()
            return
        }
        wantsLayer = true
        window.acceptsMouseMovedEvents = true
        window.makeFirstResponder(self)
        let timer = Timer(timeInterval: 1.0 / 60.0, repeats: true) { [weak self] _ in
            self?.host.draw()
        }
        // .common keeps it running while the sheet runs modally.
        RunLoop.main.add(timer, forMode: .common)
        self.timer = timer
    }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let tracking { removeTrackingArea(tracking) }
        let area = NSTrackingArea(
            rect: bounds,
            options: [.mouseMoved, .activeAlways, .inVisibleRect],
            owner: self,
            userInfo: nil
        )
        addTrackingArea(area)
        tracking = area
    }

    override func keyDown(with event: NSEvent) {
        guard let key = Self.key(for: event) else {
            super.keyDown(with: event)
            return
        }
        handle(openflip_key(host.handle, key))
    }

    override func mouseMoved(with event: NSEvent) {
        let p = host.position(of: event)
        handle(openflip_mouse_moved(host.handle, p.x, p.y))
    }

    override func mouseDown(with event: NSEvent) {
        let p = host.position(of: event)
        handle(openflip_mouse_pressed(host.handle, p.x, p.y))
    }

    @objc func close() {
        timer?.invalidate()
        timer = nil
        host.stop()
        onClose?()
    }

    private func handle(_ result: UInt32) {
        switch result {
        case OPENFLIP_CHOOSE_FONT: chooseFont()
        case OPENFLIP_EXIT: close()
        default: break
        }
    }

    private func chooseFont() {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.font]
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false
        guard panel.runModal() == .OK, let url = panel.url else { return }
        let result = url.withUnsafeFileSystemRepresentation { path in
            openflip_set_font(host.handle, path)
        }
        handle(result)
    }

    private static func key(for event: NSEvent) -> UInt32? {
        switch event.keyCode {
        case 126: return OPENFLIP_KEY_UP
        case 125: return OPENFLIP_KEY_DOWN
        case 123: return OPENFLIP_KEY_LEFT
        case 124: return OPENFLIP_KEY_RIGHT
        case 36, 76, 49: return OPENFLIP_KEY_ACTIVATE  // Return, Enter, Space
        case 51, 117: return OPENFLIP_KEY_RESET  // Delete, Forward Delete
        case 53: return OPENFLIP_KEY_ESCAPE
        default: break
        }
        switch event.charactersIgnoringModifiers?.lowercased() {
        case "s": return OPENFLIP_KEY_SETTINGS
        case "q": return OPENFLIP_KEY_QUIT
        default: return nil
        }
    }
}
