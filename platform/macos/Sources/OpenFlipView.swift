import AppKit
import ScreenSaver

/// The screensaver. macOS creates one per display, plus one for the small
/// preview in System Settings.
@objc(OpenFlipView)
final class OpenFlipView: ScreenSaverView {
    private lazy var host = ClockHost(view: self, settingsOnly: false, followsDisplay: !isPreview)
    /// Set when macOS dismissed the screensaver but kept this view alive.
    private var dismissed = false
    private var willStopObserver: NSObjectProtocol?

    override init?(frame: NSRect, isPreview: Bool) {
        super.init(frame: frame, isPreview: isPreview)
        setUp()
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
        setUp()
    }

    private func setUp() {
        // The clock only draws when something changes; asking often keeps
        // flips smooth and costs next to nothing in between.
        animationTimeInterval = 1.0 / 60.0
        wantsLayer = true
        // Since macOS 14 the screensaver host keeps views alive (and
        // animating) after the screensaver is dismissed. Release the GPU
        // resources when that happens.
        willStopObserver = DistributedNotificationCenter.default().addObserver(
            forName: Notification.Name("com.apple.screensaver.willstop"),
            object: nil,
            queue: .main
        ) { [weak self] _ in
            guard let self, !self.isPreview else { return }
            self.dismissed = true
            self.host.stop()
        }
    }

    deinit {
        if let willStopObserver {
            DistributedNotificationCenter.default().removeObserver(willStopObserver)
        }
    }

    override func startAnimation() {
        super.startAnimation()
        dismissed = false
    }

    override func stopAnimation() {
        super.stopAnimation()
        host.stop()
    }

    override func animateOneFrame() {
        guard !dismissed else { return }
        host.draw()
    }

    override func draw(_ rect: NSRect) {
        // The Metal layer shows the clock; nothing to draw with AppKit.
    }

    override var hasConfigureSheet: Bool { true }

    override var configureSheet: NSWindow? { SettingsSheet.make() }
}
