// The room's reaction ticks on the seek bar: where people reacted in this episode, merged when
// they sit close together, with who reacted for the label when the pointer is over one.

import KaeriKit
import KaeriStrings

/// A tick on the seek bar: where one or more reactions were made.
public struct ReactionTick: Equatable, Sendable {
    public var at: Double
    public var x: Double
    public var names: [String]

    /// "12:04 · Ren and Mika", for the seek bar's time label.
    @MainActor public var label: String {
        L.roomTickLabel(time: RoomModel.clock(at), names: RoomCopy.reactors(names))
    }
}

extension RoomModel {
    /// The ticks on a seek line `width` points wide for an episode `duration` long: marks within
    /// `merge` points of the tick before them share it. None outside a room or with chat hidden.
    public func ticks(duration: Double, width: Double, merge: Double = 4) -> [ReactionTick] {
        guard inRoom, !chatOff, duration > 0, width > 0 else { return [] }
        var ticks: [ReactionTick] = []
        for mark in marks.sorted(by: { $0.at < $1.at }) where mark.at >= 0 && mark.at <= duration {
            let x = mark.at / duration * width
            if var last = ticks.last, x - last.x <= merge {
                if !last.names.contains(mark.from) { last.names.append(mark.from) }
                ticks[ticks.count - 1] = last
            } else {
                ticks.append(ReactionTick(at: mark.at, x: x, names: [mark.from]))
            }
        }
        return ticks
    }
}

extension RoomCopy {
    /// "Ren", "Ren and Mika", "Ren, Mika and Sora", "Ren, Mika and 2 others": who reacted.
    public static func reactors(_ names: [String]) -> String {
        switch names.count {
        case 0: ""
        case 1: names[0]
        case 2: L.roomNamesTwo(first: names[0], second: names[1])
        case 3: L.roomNamesThree(first: names[0], second: names[1], third: names[2])
        default: L.roomNamesOthers(first: names[0], second: names[1], count: names.count - 2)
        }
    }
}
