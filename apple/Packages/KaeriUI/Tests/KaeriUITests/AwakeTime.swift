// Test waits count the Mac's awake time. The wall clock (Date) and the continuous clock run on
// while the Mac sleeps, so a wait or deadline that spans a sleep ends at once on wake, before
// what it waits for has run. A run on battery that slept through its waits (27 November) failed
// with seven issues in three KaeriStore suites, then KaeriUI with one. The suspending clock
// stops while the Mac sleeps.

import Foundation

/// The main run loop turned for `seconds` of awake time, a little at a time.
func runAwake(_ seconds: Double) {
    let end = SuspendingClock.now + .seconds(seconds)
    while SuspendingClock.now < end { RunLoop.main.run(until: Date().addingTimeInterval(0.01)) }
}
