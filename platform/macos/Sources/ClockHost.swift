import AppKit
import OpenFlipFFI

/// One openflip clock drawing into an NSView: the Rust renderer does the
/// work, this only manages the handle and converts sizes and coordinates.
final class ClockHost {
    private(set) var handle: OpaquePointer?
    private unowned let view: NSView
    private let settingsOnly: Bool

    init(view: NSView, settingsOnly: Bool) {
        self.view = view
        self.settingsOnly = settingsOnly
    }

    deinit {
        openflip_free(handle)
    }

    /// The view's size in pixels.
    var pixelSize: (width: UInt32, height: UInt32) {
        let size = view.convertToBacking(view.bounds).size
        return (UInt32(max(0, size.width.rounded())), UInt32(max(0, size.height.rounded())))
    }

    /// Draws a frame if one is due, creating the renderer on first use.
    func draw() {
        let size = pixelSize
        guard size.width > 0, size.height > 0 else { return }
        if handle == nil {
            let pointer = Unmanaged.passUnretained(view).toOpaque()
            handle = openflip_new_for_view(pointer, size.width, size.height, settingsOnly)
        }
        guard let handle else { return }
        openflip_resize(handle, size.width, size.height)
        _ = openflip_draw(handle)
    }

    /// Releases the renderer; the next `draw()` creates a new one.
    func stop() {
        openflip_free(handle)
        handle = nil
    }

    /// The event's position in pixels from the view's top-left corner.
    func position(of event: NSEvent) -> (x: Float, y: Float) {
        let point = view.convert(event.locationInWindow, from: nil)
        let top = view.isFlipped ? point.y : view.bounds.height - point.y
        let scale = view.bounds.width > 0 ? view.convertToBacking(view.bounds).width / view.bounds.width : 1
        return (Float(point.x * scale), Float(top * scale))
    }
}
