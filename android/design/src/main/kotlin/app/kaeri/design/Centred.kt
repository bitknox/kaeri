// Chapter lists that open on their own open with the current chapter's row in the middle
// (docs/native-ux.md, Reading).

package app.kaeri.design

/**
 * The first visible item's offset that puts a row of [row] pixels in the middle of a list
 * [viewport] pixels high, when the row is the first visible item: negative, so the rows before
 * it show above. The list keeps its ends, so a row near either end sits as near the middle as
 * they allow, and a list that fits stays at its top.
 */
fun centredOffset(viewport: Int, row: Int): Int = -((viewport - row) / 2).coerceAtLeast(0)
