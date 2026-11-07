## The reading demo: a comic and a story their makers share, beside the watching demo. Its
## titles, authors and licences come from the engine and are never translated.

# Button in the first run that opens the reading demo.
onboarding-read-demo = Read the demo

# Under the first run's demo heading when the device is for reading.
onboarding-demo-books-detail = The demo reads a short comic and a short story their makers share under Creative Commons.

# Under the first run's demo heading when the device is for both.
onboarding-demo-both-detail = One short episode and one short comic, shared by their makers under Creative Commons.

# Home, under the spotlight, when the demo is the only thing to read.
home-demo-only-books = Only the demo is on. Add a folder, a server or a { -plugin-store } to read your own books.

# Primary action on Home's spotlight for a book.
home-spotlight-read = Read

# The demo comic's facts line on its series page.
# @arg pages: int
demo-series-facts = { $pages ->
    [one] Demo · Comic · one page
   *[other] Demo · Comic · { $pages } pages
 }

# The comic's credit on its series page and in About the demo. The licence is "CC BY 4.0".
# @arg title: text
# @arg author: text
# @arg licence: text
demo-credit-book = { $title } by { $author }. Licensed { $licence }. Read from Wikimedia Commons.

# The story's credit in About the demo.
# @arg title: text
# @arg author: text
demo-credit-story = { $title } by { $author }. Public domain. It comes with the app.

# Credit capsule in the reader's top bar on page 1.
# @arg title: text
# @arg author: text
# @arg licence: text
reader-demo-credit = { $title } · { $author } · { $licence }

# A demo result's line in Search.
search-demo-reads = Demo · Reads now

# Search, under the demo results, when only the demo is on.
search-demo-only-books = Only the demo is on. Other titles appear once you add a { -source }.

# Title of the card after the demo's last page.
reader-demo-end = That’s the end of the demo

# Line under it.
reader-demo-end-line = To read your own books, add a folder, a server or a { -plugin-store }.

# The card's primary action.
reader-demo-add-source = Add a { -source }

# The card's secondary action, back to the first page.
reader-demo-again = Back to page 1

# The demo row's line in Your sources when reading is on.
sources-demo-books-line = A comic and a story · CC BY 4.0 and public domain

# About the demo, first line, when reading is on.
sources-demo-books-what = Works their makers share under Creative Commons, so you can try { -app-name } before adding a { -source }. It isn’t a { -source } to keep.

# About the demo, in place of sources-demo-private when reading is on.
sources-demo-books-private = The demo never signs in anywhere and never writes to a { -tracker }.

# Group on the demo's series page.
demo-chapters = Chapters

# Group on the demo's series page: the other demo work.
demo-also = Also in the demo

# Group in About the demo.
demo-credits = Credits

# A chapter row's line on the demo's series page.
# @arg pages: int
demo-pages = { $pages ->
    [one] One page
   *[other] { $pages } pages
 }

# The story's row on the demo's series page. The licence is "Public domain" as the engine says it.
# @arg author: text
# @arg licence: text
demo-story-row = Story · { $author } · { $licence }

# Primary action on the demo's series page.
# @arg number: text
demo-read-chapter = Read chapter { $number }
