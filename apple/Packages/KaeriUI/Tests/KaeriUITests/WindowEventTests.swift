#if os(macOS)
// The suites that send mouse and key events through offscreen windows, or walk a window's
// accessibility tree, run one at a time. Run side by side, one suite's held mouse button and
// the main thread's turns between them lose another suite's clicks.

import Testing

@MainActor
@Suite(.serialized)
enum WindowEventTests {}
#endif
