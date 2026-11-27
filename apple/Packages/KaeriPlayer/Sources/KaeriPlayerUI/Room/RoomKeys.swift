// What Return does in the player's corner, with a room or without: the corner's own order, so a
// stall, a found-times question or Undo is never passed over for the room's Up next or Skip.

/// Return's action in the player.
enum ReturnKey: Equatable {
    case switchSource, roomUpNext, roomSkip, upNext, skip, none

    /// A stall switches source; a found-times question or Undo holds Return for its own buttons;
    /// then the room's Up next, the room's Skip opening, and the player's own Up next or Skip.
    static func action(corner: PlayerModel.Corner?, roomUpNext: Bool, roomSkip: Bool) -> ReturnKey {
        switch corner {
        case .stall: return .switchSource
        case .prompt, .undo: return .none
        case .upNext, .skip, nil: break
        }
        if roomUpNext { return .roomUpNext }
        if roomSkip { return .roomSkip }
        switch corner {
        case .upNext: return .upNext
        case .skip: return .skip
        default: return .none
        }
    }

    /// The corner is held by a card that outranks the room's.
    static func cornerTaken(_ corner: PlayerModel.Corner?) -> Bool {
        switch corner {
        case .stall, .prompt, .undo: true
        case .upNext, .skip, nil: false
        }
    }
}
