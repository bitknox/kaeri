// Credit pages on e-ink: a bordered box at the top in place of the phone's chip and snackbar. Page
// 1's box names the pages skipped with an underlined Show and leaves with the turn; over a page
// shown again the box offers Not a credit page, then Undo until the next turn. No fade and no
// timer, since a timed change costs a refresh.

package app.kaeri.eink

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import app.kaeri.i18n.Strings
import app.kaeri.reader.CreditNotice
import app.kaeri.reader.ReaderSession

/** "1 credit page skipped Show": a bordered box at the top centre over page 1. */
@Composable
fun EinkCreditLine(count: Int, onShow: () -> Unit, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    Row(
        modifier
            .statusBarsPadding()
            .padding(top = 16.dp, start = 20.dp, end = 20.dp)
            .background(EinkColours.Paper, RoundedCornerShape(12.dp))
            .border(EinkSizes.Rule, EinkColours.Ink, RoundedCornerShape(12.dp))
            .padding(start = 18.dp, end = 6.dp)
            .semantics { liveRegion = LiveRegionMode.Polite },
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(Strings.readerCreditPagesSkipped(context, count), style = EinkType.Body, modifier = Modifier.weight(1f, fill = false))
        EinkTextButton(Strings.readerCreditShow(context), onShow, underline = true)
    }
}

/**
 * The box over a credit page shown again: Skipped as a credit page with a bordered Not a credit
 * page, then This page will show from now on with a bordered Undo, held until the next turn.
 */
@Composable
fun EinkCreditNotice(notice: CreditNotice, session: ReaderSession, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    val (words, action) = when (notice) {
        CreditNotice.SHOWN -> Strings.readerCreditShown(context) to Strings.readerCreditNot(context)
        CreditNotice.KEPT -> Strings.readerCreditKept(context) to Strings.actionUndo(context)
    }
    Row(
        modifier
            .statusBarsPadding()
            .padding(top = 16.dp, start = 20.dp, end = 20.dp)
            .fillMaxWidth()
            .heightIn(min = 64.dp)
            .background(EinkColours.Paper, RoundedCornerShape(12.dp))
            .border(EinkSizes.Rule, EinkColours.Ink, RoundedCornerShape(12.dp))
            .padding(start = 18.dp, end = 8.dp, top = 8.dp, bottom = 8.dp)
            .semantics { liveRegion = LiveRegionMode.Polite },
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(words, style = EinkType.Body, modifier = Modifier.weight(1f).padding(end = 12.dp))
        EinkOutlinedButton(
            action,
            {
                when (notice) {
                    CreditNotice.SHOWN -> session.keepCredit()
                    CreditNotice.KEPT -> session.undoKeep()
                }
            },
            height = 46.dp,
        )
    }
}
