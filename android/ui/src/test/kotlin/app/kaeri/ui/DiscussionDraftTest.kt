package app.kaeri.ui

import androidx.activity.ComponentActivity
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.performTextInput
import app.kaeri.design.KaeriTheme
import app.kaeri.engine.model.CommentView
import app.kaeri.engine.model.DiscussionState
import app.kaeri.engine.model.DiscussionView
import app.kaeri.engine.model.Msg
import app.kaeri.engine.model.PostState
import app.kaeri.store.AppStore
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * A comment being written in the discussion sheet stays in the field when Reply picks a comment
 * to answer, when Reply picks another, and when the reply is let go. Each of those once started a
 * new empty field and the typed text was gone.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class DiscussionDraftTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val store = AppStore(CoroutineScope(SupervisorJob() + Dispatchers.Main))

    private val view = DiscussionView(
        media = 900001u,
        episode = 1u,
        title = "Episode 1",
        commentCount = 2u,
        url = "https://forum.example/thread/1",
        state = DiscussionState.Ready(more = false),
        canPost = true,
        signedIn = true,
    )

    private fun comment(id: UInt, author: String) =
        CommentView(id = id, author = author, createdAt = 0L, age = Msg.CastEpisode("1"), likeCount = 0u, pending = PostState.POSTED)

    private val replying = mutableStateOf<CommentView?>(null)

    private val draft = "The tower scene at the end"

    private fun field() = compose.onNode(hasSetTextAction())

    @Test
    fun the_typed_text_stays_through_reply_and_letting_the_reply_go() {
        compose.setContent {
            KaeriTheme {
                CompositionLocalProvider(LocalAppStore provides store) {
                    Composer(view = view, replying = replying.value, editing = null, onDone = { replying.value = null }, onSignIn = {}, onTyped = {})
                }
            }
        }
        field().performTextInput(draft)
        compose.runOnIdle { replying.value = comment(7u, "Mirei") }
        field().assert(hasText(draft))
        compose.runOnIdle { replying.value = comment(8u, "Tobi") }
        field().assert(hasText(draft))
        compose.runOnIdle { replying.value = null }
        field().assert(hasText(draft))
    }
}
