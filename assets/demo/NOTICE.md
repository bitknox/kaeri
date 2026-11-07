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
