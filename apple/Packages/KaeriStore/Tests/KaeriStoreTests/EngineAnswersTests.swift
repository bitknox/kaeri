// The questions the notifications ask the engine wait as long as the engine may take to answer.

import KaeriKit
@testable import KaeriStore
import Testing

@Suite(.timeLimit(.minutes(1)))
struct EngineAnswersTests {
    /// The engine keeps what it says as said before it answers, and may answer after 23 seconds
    /// (AniList's 15 and the dubs' 8): an answer that late still reaches the check.
    @Test func newEpisodesWaitsForAnAnswerAsLateAsTheEngineMayGiveIt() async {
        let answers = EngineAnswers()
        let said = [NewEpisode(id: 900_001, title: "The Steel Gardener", episode: 4, airedAt: 1, coverUrl: nil, translation: .dub)]
        let questions = EngineQuestions(answers: answers) { command in
            guard case .checkNewEpisodes = command else { return }
            Task {
                try? await Task.sleep(for: .seconds(23))
                answers.deliver([.newEpisodes(said)])
            }
        }
        let answer = await questions.newEpisodes()
        #expect(answer?.map(\.id) == said.map(\.id))
    }
}
