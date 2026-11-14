// The iPhone's calendar: one agenda of days from the floor to the ceiling, opening on today,
// with the week as a strip above it.

#if os(iOS)
import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

/// The calendar on an iPhone and in a narrow window: days one under the other, past above and
/// later below, each loaded as it is reached. The strip names the week and follows the scroll.
struct PhoneCalendar: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.accessibilityVoiceOverEnabled) private var voiceOver
    @Namespace private var rotor
    @State private var position = ScrollPosition(idType: Date.self)
    /// The day at the top of the agenda, and the days on screen.
    @State private var topDay: Date?
    @State private var visible: Set<Date> = []
    /// The Monday of the week the strip shows.
    @State private var stripWeek: Date? = Self.monday(of: .now)
    /// A day asked for that is not loaded yet, to scroll to once it is.
    @State private var pending: Date?
    /// The day at the top when earlier days were asked for, or the day the calendar opened on,
    /// to stay on whatever loads until it is back at the top or the person moves.
    @State private var holding: Date?
    /// The first day drawn: the day the calendar opens on, until the person scrolls or picks an
    /// earlier day. With nothing above it the day opens at the top with no jump to make, so it
    /// stays there whatever loads. `nil` draws every day loaded; VoiceOver, which moves the agenda
    /// with no scroll by hand, has every day from the start and jumps to the day instead.
    @State private var shownFrom: Date? = Calendar.current.startOfDay(for: .now)
    /// The day at the top when the earlier days were drawn, to stay on once they are.
    @State private var revealAt: Date?
    /// Draws the earlier days once the scroll by hand under way comes to rest.
    @State private var earlierAtRest = false
    /// The day scrolled to as the earlier days were drawn, until it has been on screen a moment.
    /// The first frames after they come in can still report the agenda's first day, and a scroll
    /// among days whose height the agenda guessed can be lost, so meanwhile the agenda asks for no
    /// more past, the strip stays on its week, and the agenda never stays on its first day.
    @State private var revealing: Date?
    /// Whether that day has shown: after that, the agenda goes wherever the person takes it.
    @State private var revealSeen = false
    @State private var revealer: Task<Void, Never>?
    /// Scrolls to the day held again while it is short of the top.
    @State private var keeper: Task<Void, Never>?
    @State private var opened = false

    private static func monday(of day: Date) -> Date {
        weekDays(offset: 0, now: day)[0]
    }

    var body: some View {
        TimelineView(.everyMinute) { context in
            agenda(now: context.date)
        }
        // The title stays in the bar: the strip sits under it, and the agenda opens at its top.
        .toolbarTitleDisplayMode(.inline)
        .pullToRefresh(store, .calendar)
        .toolbar {
            ToolbarItem(placement: .topBarLeading) {
                if !todayShown {
                    Button(L.calendarToday) { returnToToday() }
                        .transition(.opacity)
                }
            }
            ToolbarItem(placement: .topBarTrailing) {
                Picker(L.filterShow, selection: Binding(get: { store.calendarMine }, set: { store.setSwitch(.calendarMine, on: $0) })) {
                    Text(L.calendarFilterAll).tag(false)
                    Text(L.calendarFilterMine).tag(true)
                }
                .pickerStyle(.segmented)
                .fixedSize()
            }
        }
        .task {
            if !opened, voiceOver { shownFrom = nil }
            if navigator.launchCalendarMine {
                navigator.launchCalendarMine = false
                store.setSwitch(.calendarMine, on: true)
            }
            if store.calendar.list.state == .idle { store.loadCalendar() }
        }
    }

    /// Whether any of today is on screen.
    private var todayShown: Bool {
        visible.isEmpty || visible.contains(Calendar.current.startOfDay(for: .now))
    }

    // MARK: Agenda

    private func agenda(now: Date) -> some View {
        let calendar = Calendar.current
        let today = calendar.startOfDay(for: now)
        let bounds = store.calendarBounds
        let broadcasts = store.broadcasts(mine: store.calendarMine)
        let byDay = Dictionary(grouping: broadcasts) { calendar.startOfDay(for: $0.airs) }
        let model = store.calendar
        let anchor = TimeInterval(model.anchor ?? 0)
        let airs = model.rows.compactMap { $0.airingIn.map { Date(timeIntervalSince1970: anchor + TimeInterval($0)) } }
        let first = min(airs.first.map { calendar.startOfDay(for: $0) } ?? today, today)
        let last = max(airs.last.map { calendar.startOfDay(for: $0) } ?? today, today)
        // My list with nothing from today on, once every day ahead is loaded: one line.
        let mineBare = store.calendarMine && !model.moreAhead
            && !broadcasts.contains { $0.airs >= today }
        let start = max(first, bounds.floor)
        let end = mineBare ? today : min(last, bounds.ceiling)
        let days = Self.days(from: shownFrom.map { min(max($0, start), end) } ?? start, to: end)
        let key = Need(count: days.count, first: days.first, last: days.last, mine: store.calendarMine, pages: model.pages + model.aheadPages)

        return ScrollView {
            ListStates(
                state: model.list.state,
                placeholder: AnyView(PhoneCalendarPlaceholder()),
                activity: "loading the calendar", // the engine job this list waits on
                failure: L.calendarLoadFailed,
                retry: { store.loadCalendar() }
            ) { _ in
                agendaDays(days, byDay: byDay, today: today, now: now, bounds: bounds, mineBare: mineBare)
            } empty: {
                agendaDays(days, byDay: byDay, today: today, now: now, bounds: bounds, mineBare: mineBare)
            }
            .padding(.horizontal, 16)
            .padding(.bottom, 24)
        }
        .scrollPosition($position, anchor: .top)
        // A scroll by hand lets go of a day held, and once it comes to rest draws the days before
        // the one opened on: drawn under a moving finger, the scroll back to the day at the top
        // can land days off it, among days whose height the agenda guessed, with no way to
        // correct it without fighting the finger.
        .onScrollPhaseChange { _, phase in
            if phase == .interacting {
                holding = nil
                if shownFrom != nil { earlierAtRest = true }
            } else if phase == .idle, earlierAtRest {
                earlierAtRest = false
                showEarlier()
            }
        }
        .onScrollTargetVisibilityChange(idType: Date.self, threshold: 0.1) { ids in
            let wasShown = todayShown
            // Today's button fades in (0.2 s) once today has left, and out (0.3 s) as it returns.
            withAnimation(wasShown ? .easeOut(duration: 0.2) : .easeIn(duration: 0.3)) {
                visible = Set(ids)
            }
            topDay = ids.min()
            checkReveal(ids: ids, days: days)
            hold(ids: ids, days: days)
            prefetch(ids: ids, days: days, bounds: bounds)
        }
        .safeAreaInset(edge: .top, spacing: 0) {
            strip(today: today, now: now, bounds: bounds, broadcasts: broadcasts)
        }
        .daysRotor(days, broadcasts: broadcasts, in: rotor)
        .task(id: model.list.state == .loading) { await open(today: today) }
        .onDisappear {
            keeper?.cancel()
            revealer?.cancel()
        }
        .onChange(of: key) { _, _ in settle(days: days, today: today) }
        .onChange(of: navigator.calendarToToday) { _, _ in returnToToday() }
        .onChange(of: voiceOver) { _, on in if on { showEarlier() } }
        .onChange(of: topDay) { _, _ in followTop() }
        .accessibilityElement(children: .contain)
    }

    private struct Need: Hashable {
        let count: Int
        let first: Date?
        let last: Date?
        let mine: Bool
        let pages: UInt32
    }

    static func days(from first: Date, to last: Date) -> [Date] {
        let calendar = Calendar.current
        var days: [Date] = []
        var day = first
        while day <= last, days.count < 200 {
            days.append(day)
            day = calendar.date(byAdding: .day, value: 1, to: day)!
        }
        return days
    }

    @ViewBuilder
    private func agendaDays(_ days: [Date], byDay: [Date: [Broadcast]], today: Date, now: Date, bounds: CalendarBounds, mineBare: Bool) -> some View {
        LazyVStack(alignment: .leading, spacing: 20) {
            top(days: days, bounds: bounds)
            ForEach(days, id: \.self) { day in
                let rows = byDay[day] ?? []
                VStack(alignment: .leading, spacing: 0) {
                    heading(day, today: today)
                        .accessibilityRotorEntry(id: day, in: rotor)
                    if day == today, mineBare {
                        VStack(alignment: .leading, spacing: 12) {
                            Text(L.calendarEmptyMineAheadApple).font(.ui(15)).foregroundStyle(.haze)
                            Button(L.calendarShowAll) { store.setSwitch(.calendarMine, on: false) }
                                .buttonStyle(SecondaryCapsuleStyle(height: 36, fontSize: 15))
                        }
                        .padding(.top, 8)
                    } else if rows.isEmpty {
                        Text(L.calendarDayEmptyApple)
                            .font(.ui(15))
                            .foregroundStyle(.haze)
                            .padding(.top, 4)
                    } else {
                        let split = day == today ? rows.firstIndex { $0.airs > now } ?? rows.count : -1
                        if day == today, split == 0 { nowLine(now) }
                        ForEach(Array(rows.enumerated()), id: \.element.id) { index, broadcast in
                            if day == today, index == split, index > 0 { nowLine(now) }
                            PhoneBroadcastRow(broadcast: broadcast, now: now)
                            if index < rows.count - 1 {
                                Rectangle().fill(.faintRule).frame(height: 1).padding(.leading, 66)
                            }
                        }
                        if day == today, split == rows.count { nowLine(now) }
                    }
                }
                // When a broadcast airs, the line moves below its row.
                .animation(reduceMotion ? .easeInOut(duration: 0.2) : .standard, value: day == today ? aired(rows, now: now) : 0)
                .id(day)
            }
            bottom(days: days, bounds: bounds, mineBare: mineBare)
        }
        .scrollTargetLayout()
        .padding(.top, 8)
    }

    /// How many of the day's rows have aired: where the line for now stands.
    private func aired(_ rows: [Broadcast], now: Date) -> Int {
        rows.firstIndex { $0.airs > now } ?? rows.count
    }

    /// The line for now, over the rule between two rows and taking no room.
    private func nowLine(_ now: Date) -> some View {
        Color.clear.frame(height: 0)
            .overlay { NowLine(now: now, size: 13, labelWidth: 52).frame(height: 16) }
            .padding(.vertical, 1)
    }

    private func heading(_ day: Date, today: Date) -> some View {
        let calendar = Calendar.current
        let tomorrow = calendar.date(byAdding: .day, value: 1, to: today)!
        let date = "\(CalendarWords.weekday(day)) \(CalendarWords.dayMonth(day))"
        return Group {
            if day == today || day == tomorrow {
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    PhoneHeading(day == today ? L.calendarToday : L.dayTomorrow)
                    Text(date).font(.ui(15)).foregroundStyle(.haze)
                }
                .accessibilityElement(children: .combine)
                .accessibilityAddTraits(.isHeader)
            } else {
                PhoneHeading(date)
            }
        }
    }

    // MARK: Edges

    @ViewBuilder
    private func top(days: [Date], bounds: CalendarBounds) -> some View {
        let model = store.calendar
        if model.morePast {
            if model.pastFailed {
                CalendarFailedLine(text: L.calendarLoadEarlierFailed, retry: { retry() }, size: 15)
            } else {
                StagedLoad(loading: model.loadingPast, activity: L.calendarLoadingEarlier) {
                    AgendaPlaceholder()
                } answer: {
                    Color.clear.frame(height: 1)
                }
            }
        } else if let first = days.first, first <= bounds.floor {
            CalendarEdgeLine(L.calendarFloor, size: 15)
        }
    }

    @ViewBuilder
    private func bottom(days: [Date], bounds: CalendarBounds, mineBare: Bool) -> some View {
        let model = store.calendar
        if mineBare {
            EmptyView()
        } else if model.moreAhead {
            if model.aheadFailed {
                CalendarFailedLine(text: L.calendarLoadLaterFailed, retry: { retry() }, size: 15)
            } else {
                StagedLoad(loading: model.loadingAhead, activity: L.calendarLoadingLater) {
                    AgendaPlaceholder()
                } answer: {
                    Color.clear.frame(height: 1)
                }
            }
        } else if let last = days.last {
            CalendarEdgeLine(last >= bounds.ceiling ? L.calendarCeiling : L.calendarEnd, size: 15)
        }
    }

    private func retry() {
        let bounds = store.calendarBounds
        store.retryCalendar(from: bounds.floor, to: bounds.ceiling)
    }

    // MARK: Strip

    private func strip(today: Date, now: Date, bounds: CalendarBounds, broadcasts: [Broadcast]) -> some View {
        let calendar = Calendar.current
        let firstWeek = Self.monday(of: bounds.floor)
        let lastWeek = Self.monday(of: bounds.ceiling)
        var weeks: [Date] = []
        var week = firstWeek
        while week <= lastWeek, weeks.count < 40 {
            weeks.append(week)
            week = calendar.date(byAdding: .day, value: 7, to: week)!
        }
        let current = topDay.map { calendar.startOfDay(for: $0) }
        let fresh = Set(broadcasts.filter { !$0.watched }.map { calendar.startOfDay(for: $0.airs) })
        return ScrollView(.horizontal) {
            LazyHStack(spacing: 0) {
                ForEach(weeks, id: \.self) { monday in
                    HStack(spacing: 0) {
                        ForEach(0..<7, id: \.self) { offset in
                            let day = calendar.date(byAdding: .day, value: offset, to: monday)!
                            dayButton(day, today: today, current: current ?? today, inRange: bounds.contains(day), fresh: fresh.contains(day))
                                .frame(maxWidth: .infinity)
                        }
                    }
                    .containerRelativeFrame(.horizontal)
                }
            }
            .scrollTargetLayout()
        }
        .scrollTargetBehavior(.paging)
        .scrollPosition(id: $stripWeek)
        .scrollIndicators(.never)
        // As tall as a row of days: a horizontal scroll view takes all the height it is offered.
        .fixedSize(horizontal: false, vertical: true)
        // Seven to a row: no larger than `.xxxLarge`, as controls are.
        .dynamicTypeSize(...DynamicTypeSize.xxxLarge)
        .padding(.horizontal, 16)
        .padding(.vertical, 4)
        .background(.ground)
    }

    private func dayButton(_ day: Date, today: Date, current: Date, inRange: Bool, fresh: Bool) -> some View {
        let selected = day == current
        let isToday = day == today
        return Button {
            go(to: day)
        } label: {
            VStack(spacing: 4) {
                Text(CalendarWords.shortWeekday(day))
                    .font(.ui(11, weight: selected || isToday ? .semibold : .medium))
                    .foregroundStyle(selected ? .amber : isToday ? .silk : .haze)
                Text(CalendarWords.dayNumber(day))
                    .font(.ui(17, weight: selected ? .semibold : .regular))
                    .foregroundStyle(selected ? .amber : .silk)
                    .frame(width: 30, height: 30)
                    // Today, while another day is current: a 1 pt silk ring round the numeral.
                    .overlay { if isToday, !selected { Circle().strokeBorder(.silk, lineWidth: 1) } }
                Circle().fill(fresh ? Palette.water : .clear).frame(width: 5, height: 5)
            }
            .frame(width: 44)
            .frame(minHeight: 64)
            .background(selected ? Palette.selection : .clear, in: .capsule)
            .contentShape(.capsule)
        }
        .buttonStyle(.plain)
        .pointerHover()
        .disabled(!inRange)
        .opacity(inRange ? 1 : 0.32)
        .accessibilityLabel("\(CalendarWords.weekday(day)) \(CalendarWords.dayMonth(day))")
        .accessibilityAddTraits(selected ? .isSelected : [])
    }

    // MARK: Moving

    /// Opens on today, or the day the launch route names: as the first day drawn, or with
    /// VoiceOver by a jump with no animation.
    private func open(today: Date) async {
        guard !opened, store.calendar.list.state != .loading, store.calendar.list.state != .idle else { return }
        var target = today
        if navigator.calendarOpensOn != 0 {
            target = Calendar.current.date(byAdding: .day, value: navigator.calendarOpensOn, to: today) ?? today
            navigator.calendarOpensOn = 0
        }
        if shownFrom != nil {
            shownFrom = target
            topDay = target
            opened = true
            return
        }
        try? await Task.sleep(for: .milliseconds(80))
        position.scrollTo(id: target, anchor: .top)
        // Held like a day that earlier days came in above: the agenda guesses the height of the
        // days it has not drawn, so the first jump can land days short of the target.
        holding = target
        // A jump without animation can land before the visibility callback reports the new days,
        // which leaves the strip on the agenda's first week; the strip follows the target now.
        topDay = target
        opened = true
        keepHolding()
    }

    /// Scrolls to the day held again every quarter second until it is at the top. The visibility
    /// callback runs only when the days on screen change, and a scroll that lands short, among
    /// days whose height the agenda guessed, can stay put there. After 8 s it stops scrolling
    /// but still holds the day, so no day short of it is held instead.
    private func keepHolding() {
        guard keeper == nil else { return }
        keeper = Task { @MainActor in
            defer { keeper = nil }
            for _ in 0..<32 {
                try? await Task.sleep(for: .milliseconds(250))
                guard !Task.isCancelled, let day = holding else { return }
                if visible.min() == day {
                    holding = nil
                    return
                }
                if !store.calendar.loadingPast { position.scrollTo(id: day, anchor: .top) }
            }
        }
    }

    /// Scrolls to a day that arrived after it was asked for, and stays where the person was when
    /// earlier days come in above.
    private func settle(days: [Date], today: Date) {
        if let revealAt, days.contains(revealAt) {
            // The earlier days drawn above the day at the top: back to it, held there until it is
            // at the top, or on to a day picked.
            self.revealAt = nil
            position.scrollTo(id: revealAt, anchor: .top)
            if let pending, days.contains(pending) {
                self.pending = nil
                watchReveal(pending, days: days)
                Task { @MainActor in
                    try? await Task.sleep(for: .milliseconds(30))
                    withAnimation(reduceMotion ? nil : .standard) { position.scrollTo(id: pending, anchor: .top) }
                }
            } else {
                watchReveal(revealAt, days: days)
                holding = revealAt
                keepHolding()
            }
        } else if let pending, days.contains(pending) {
            self.pending = nil
            withAnimation(reduceMotion ? nil : .standard) { position.scrollTo(id: pending, anchor: .top) }
        } else if let holding, days.contains(holding) {
            position.scrollTo(id: holding, anchor: .top)
            keepHolding()
        }
    }

    /// Lets go of the day held once it is back at the top, or as high as it can go: on screen with
    /// the last day, and nothing later to load. A scroll to it, on opening or right after days
    /// came in above, can land short of it, among days whose height the agenda guessed, and on a
    /// first open the rows still loading move it again and again. Letting go there would leave
    /// those days asking for more past and one of them held instead, so it scrolls to the day
    /// again, here and from `keepHolding`, until it is at the top or the person scrolls or picks
    /// a day.
    private func hold(ids: [Date], days: [Date]) {
        guard let holding, days.contains(holding), !store.calendar.loadingPast else { return }
        let highest = ids.contains(holding) && !store.calendar.moreAhead && days.last.map(ids.contains) == true
        if ids.min() == holding || highest {
            self.holding = nil
        } else {
            position.scrollTo(id: holding, anchor: .top)
            keepHolding()
        }
    }

    /// Draws the days before the one the calendar opened on, staying on the day at the top.
    private func showEarlier() {
        guard let shownFrom else { return }
        revealAt = topDay ?? shownFrom
        self.shownFrom = nil
    }

    /// Watches the day scrolled to as the earlier days come in, at least 0.6 s and until it has
    /// shown, 2 s at most, then lets the strip follow the agenda again. A scroll by hand meanwhile
    /// goes on as usual: a programmatic scroll during a drag can start the drag's phase over, so
    /// that is no sign the reports have settled.
    private func watchReveal(_ day: Date, days: [Date]) {
        revealing = day
        revealSeen = false
        revealer?.cancel()
        revealer = Task { @MainActor in
            for step in 1...10 {
                try? await Task.sleep(for: .milliseconds(200))
                guard !Task.isCancelled, revealing == day else { return }
                checkReveal(ids: Array(visible), days: days)
                if step >= 3, revealSeen { break }
            }
            if revealing == day {
                revealing = nil
                followTop()
                prefetch(ids: Array(visible), days: days, bounds: store.calendarBounds)
            }
        }
    }

    /// Moves the strip to the week of the day at the top. Not while earlier days come in above
    /// it: their first frames can report the agenda's first day for a moment, and the strip
    /// paging there and straight back can stop on the wrong week.
    private func followTop() {
        guard let topDay, revealAt == nil, revealing == nil else { return }
        let week = Self.monday(of: topDay)
        if week != stripWeek { withAnimation(reduceMotion ? nil : .standard) { stripWeek = week } }
    }

    /// Scrolls back to the day when the agenda sits on its first day without it having shown: a
    /// scroll that was lost. Anywhere else the agenda is on its way.
    private func checkReveal(ids: [Date], days: [Date]) {
        guard let revealing, !revealSeen else { return }
        if ids.contains(revealing) {
            revealSeen = true
        } else if let first = days.first, ids.contains(first) {
            position.scrollTo(id: revealing, anchor: .top)
        }
    }

    /// Asks for the next stretch when the loaded edge is a few days from the screen.
    private func prefetch(ids: [Date], days: [Date], bounds: CalendarBounds) {
        let model = store.calendar
        guard opened, !days.isEmpty else { return }
        let near = Set(days.prefix(3))
        let far = Set(days.suffix(3))
        // While the day held is not back at the top, the earlier days that just came in sit on
        // screen above it: they ask for nothing more, and the day held stays the one held.
        if holding == nil, shownFrom == nil, revealing == nil, model.morePast, !model.pastFailed, !model.loadingPast, !near.isDisjoint(with: ids) {
            holding = ids.min()
            store.loadCalendarPast()
        }
        if model.moreAhead, !model.aheadFailed, !model.loadingAhead, !far.isDisjoint(with: ids) {
            store.loadCalendarAhead()
        }
    }

    /// A tap in the strip: scrolls to the day, asking for it first when it is not loaded.
    private func go(to day: Date) {
        holding = nil
        let model = store.calendar
        let calendar = Calendar.current
        let anchor = TimeInterval(model.anchor ?? 0)
        let airs = model.rows.compactMap { $0.airingIn.map { Date(timeIntervalSince1970: anchor + TimeInterval($0)) } }
        let last = airs.last.map { calendar.startOfDay(for: $0) } ?? calendar.startOfDay(for: .now)
        let first = airs.first.map { calendar.startOfDay(for: $0) } ?? calendar.startOfDay(for: .now)
        let end = calendar.date(byAdding: .day, value: 1, to: day)!
        if let shownFrom, day < shownFrom {
            showEarlier()
            pending = day
            if day < first, model.morePast { store.loadCalendarPast(through: day) }
            return
        }
        if day > last, model.moreAhead {
            pending = day
            store.loadCalendarAhead(through: end)
        } else if day < first, model.morePast {
            pending = day
            store.loadCalendarPast(through: day)
        } else {
            withAnimation(reduceMotion ? nil : .standard) { position.scrollTo(id: day, anchor: .top) }
        }
    }

    /// Today, from wherever: a jump to a screen short of it, then the last screen scrolled.
    private func returnToToday() {
        holding = nil
        let calendar = Calendar.current
        let today = calendar.startOfDay(for: .now)
        if let shownFrom, today < shownFrom {
            showEarlier()
            pending = today
            return
        }
        guard !reduceMotion else {
            withAnimation(.standard) { position.scrollTo(id: today, anchor: .top) }
            return
        }
        // A screen short of today on the side the agenda is on, inside the days it holds.
        let above = (topDay ?? today) > today
        let model = store.calendar
        let bounds = store.calendarBounds
        let anchor = TimeInterval(model.anchor ?? 0)
        let airs = model.rows.compactMap { $0.airingIn.map { Date(timeIntervalSince1970: anchor + TimeInterval($0)) } }
        let first = max(min(airs.first.map { calendar.startOfDay(for: $0) } ?? today, today), bounds.floor, shownFrom ?? .distantPast)
        let last = min(max(airs.last.map { calendar.startOfDay(for: $0) } ?? today, today), bounds.ceiling)
        let short = min(max(calendar.date(byAdding: .day, value: above ? 2 : -2, to: today)!, first), last)
        if short != today { position.scrollTo(id: short, anchor: .top) }
        Task { @MainActor in
            try? await Task.sleep(for: .milliseconds(30))
            withAnimation(.standard) { position.scrollTo(id: today, anchor: .top) }
        }
    }
}

/// Rows where an edge's next stretch will stand, static and redacted.
struct AgendaPlaceholder: View {
    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            ForEach(0..<3, id: \.self) { _ in
                HStack(spacing: 14) {
                    Text("17:30").font(.ui(15)).frame(minWidth: 52, alignment: .leading) // i18n-skip
                    RoundedRectangle(cornerRadius: 6).fill(.raised).frame(width: 50, height: 75)
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Natsukaze Relay").font(.showTitle(size: 17, relativeTo: .body)) // i18n-skip
                        Text("Episode 9 in 1h 30m").font(.ui(15)) // i18n-skip
                    }
                    Spacer(minLength: 0)
                }
                .padding(.vertical, 8)
            }
        }
        .redacted(reason: .placeholder)
        .foregroundStyle(.haze)
        .accessibilityHidden(true)
    }
}
#endif
