// The two questions the app asks when the engine cannot tell what screen it is on, drawn in the
// e-ink tokens because that is the screen they are asked on.

package app.kaeri.eink

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import app.kaeri.i18n.Strings

/** Whichever question [state] needs, or nothing. */
@Composable
fun EinkQuestions(state: EinkState, model: EinkModel, modifier: Modifier = Modifier) {
    when (state.question) {
        EinkQuestion.EINK -> EinkQuestionScreen(onYes = { model.answerEink(true) }, onNo = { model.answerEink(false) }, modifier = modifier)
        EinkQuestion.COLOUR -> ColourQuestionScreen(
            onColour = { model.answerColour(true) },
            onGreys = { model.answerColour(false) },
            onSkip = model::skipColour,
            modifier = modifier,
        )
        null -> {}
    }
}

/** "Is this an e-ink screen?" */
@Composable
fun EinkQuestionScreen(onYes: () -> Unit, onNo: () -> Unit, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    QuestionPage(modifier) {
        Text(Strings.einkAskTitle(context), style = EinkType.PageTitle, modifier = Modifier.semantics { heading() })
        Spacer(Modifier.height(16.dp))
        Text(Strings.einkAskBody(context), style = EinkType.Body.copy(color = EinkColours.Secondary))
        Spacer(Modifier.height(36.dp))
        EinkSolidButton(Strings.einkAskYes(context), onYes, Modifier.fillMaxWidth())
        Spacer(Modifier.height(14.dp))
        EinkOutlinedButton(Strings.einkAskNo(context), onNo, Modifier.fillMaxWidth())
        Spacer(Modifier.height(24.dp))
        Text(Strings.einkAskLater(context), style = EinkType.Tertiary)
    }
}

/** "Does this screen show colour?", with the four squares to look at. */
@Composable
fun ColourQuestionScreen(onColour: () -> Unit, onGreys: () -> Unit, onSkip: () -> Unit, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    QuestionPage(modifier) {
        Text(Strings.einkColourTitle(context), style = EinkType.PageTitle, modifier = Modifier.semantics { heading() })
        Spacer(Modifier.height(16.dp))
        Text(Strings.einkColourBody(context), style = EinkType.Body.copy(color = EinkColours.Secondary))
        Spacer(Modifier.height(28.dp))
        val squares = Strings.einkColourSquares(context)
        Row(
            Modifier.fillMaxWidth().semantics(mergeDescendants = true) { contentDescription = squares },
            horizontalArrangement = Arrangement.spacedBy(16.dp, Alignment.CenterHorizontally),
        ) {
            EinkColours.Squares.forEach { colour ->
                Box(Modifier.size(92.dp).background(colour).border(EinkSizes.Rule, EinkColours.Ink))
            }
        }
        Spacer(Modifier.height(28.dp))
        Text(Strings.einkColourHint(context), style = EinkType.Secondary)
        Spacer(Modifier.height(28.dp))
        EinkSolidButton(Strings.einkColourYes(context), onColour, Modifier.fillMaxWidth())
        Spacer(Modifier.height(14.dp))
        EinkOutlinedButton(Strings.einkColourNo(context), onGreys, Modifier.fillMaxWidth())
        Spacer(Modifier.height(6.dp))
        EinkTextButton(Strings.einkColourSkip(context), onSkip, Modifier.align(Alignment.CenterHorizontally))
    }
}

/** The line shown where the question was, once it was skipped. */
@Composable
fun ColourSkippedLine(modifier: Modifier = Modifier) {
    Text(Strings.einkColourSkipped(LocalContext.current), style = EinkType.Tertiary, modifier = modifier)
}

@Composable
private fun QuestionPage(modifier: Modifier, content: @Composable ColumnScope.() -> Unit) {
    Box(modifier.fillMaxSize().background(EinkColours.Paper), contentAlignment = Alignment.TopCenter) {
        Column(
            Modifier.widthIn(max = 560.dp).fillMaxWidth().verticalScroll(rememberScrollState()).padding(horizontal = 44.dp, vertical = 92.dp),
            content = content,
        )
    }
}
