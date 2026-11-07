# Reading demo

The reading demo has two works. Kaeri reads both before anyone adds a source.

## Pepper&Carrot, episode 1

**Pepper&Carrot, Episode 1: The Potion of Flight** by
[David Revoy](https://www.peppercarrot.com/), licensed under
[CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). Kaeri doesn't bundle the pages. The
app reads the three English pages from Wikimedia Commons, as Commons holds them:

- https://commons.wikimedia.org/wiki/File:Pepper%26Carrot_en_episode_01_page_01.jpg
- https://commons.wikimedia.org/wiki/File:Pepper%26Carrot_en_episode_01_page_02.jpg
- https://commons.wikimedia.org/wiki/File:Pepper%26Carrot_en_episode_01_page_03.jpg

Each page is pinned by the SHA-1 Commons reports for it
(`crates/kaeri-core/src/reading_demo.rs`), and a page whose bytes differ is not shown under this
credit. Changes: none. The reader scales the pages to the screen.

## Kwaidan: Yuki-Onna

`kwaidan-yuki-onna.txt` is the story "Yuki-Onna" from *Kwaidan: Stories and Studies of Strange
Things* by Lafcadio Hearn (1904), which is in the public domain. The text comes from Project
Gutenberg ebook 1210. The Project Gutenberg header, footer and licence, the illustration line,
the footnote markers and the notes are removed, so the file is the story's text alone and carries
no Project Gutenberg trademark. The paragraphs are as Hearn wrote them.

## Kwaidan cover

`kwaidan-yuki-onna-cover.jpg` is the frontispiece of the same 1904 book, "Blowing her breath upon
him" by Keishū Takénouchi, which is in the public domain. It comes from Wikimedia Commons:

- https://commons.wikimedia.org/wiki/File:Kwaidan,_Stories_and_Studies_of_Strange_Things_-_Frontispiece.png

The source file has SHA-1 `3ef6a3b5d0878e01fa70b6d2b9b2a40031bfbb0c` (2,679,990 bytes, 1788 by
2078 pixels). Changes: scaled to 900 pixels tall (774 by 900) and saved as a JPEG at quality 85,
the size the demo's comic cover is shown at. The bundled file has SHA-1
`3ca8076ad8eb97419838eb2f71baf7979240da91`, which `crates/kaeri-core/build.rs` checks when the
engine builds. The app never fetches it; About the demo credits it.
