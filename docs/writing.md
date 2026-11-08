# Writing Kaeri copy

This guide covers every word people read: app strings in `i18n/`, READMEs, the website and the
docs. It condenses the maintainer's humanizer guide and his answers about how it applies here.

## The hard rule: legal first

One rule is not a matter of taste.

- Kaeri is neutral about sources. It never supplies, promotes or curates them, and the copy
  never sounds excited about content or about where it comes from.
- The legal line is "Kaeri doesn't host any shows or books. You add your own sources." It
  stays word for word. In the apps it shows only in About; the long form is the website's legal
  page, and that page keeps its substance.
- The legal words in `i18n/lint.toml` stay banned. Say "sources", never "providers".

Everything below is guidance. Where older style notes in the repo disagree with it, this guide
wins.

## Write like a person

Clean writing can still sound like nobody wrote it. Give it a voice.

- Vary the rhythm. A short sentence lands. A longer one can take its time and carry the detail.
- Be specific. "Watch history stays in a SQLite file on the device" beats "your data stays
  private".
- Have opinions about Kaeri's own choices: a native app per platform, no account, history that
  stays local, plugins in a sandbox. Say why, plainly. Never have opinions about sources or
  shows.
- No first person, not even in the README. The voice comes from rhythm, detail and those
  opinions.
- App strings stay short. Voice there means plain, exact, human wording in labels, empty states
  and errors, not jokes.

## Patterns to cut

Inflated significance. "Serves as a testament to", "plays a key role", "marks a shift",
"evolving landscape". Say what the thing does.

Promotional words. Stunning, vibrant, rich, seamless, powerful, groundbreaking, boasts.

The AI word list. Additionally, crucial, delve, enhance, foster, highlight and showcase as
verbs, intricate, key as an adjective, pivotal, tapestry, testament, underscore, valuable.

Copula dodging. "Serves as", "stands as", "features", "offers" where "is" or "has" works.

Trailing -ing clauses that add a fake analysis: "..., ensuring a smooth experience".

Negative parallels. "It's not just X, it's Y." "Not only... but also...".

Groups of three by reflex. Two items are fine. So are four.

Synonym cycling. If it's a source in one sentence, it's a source in the next.

False ranges. "From X to Y" only when X and Y sit on a real scale.

Vague authority. "Experts say", "many users find". Name the thing or cut it.

Filler and hedging. "In order to" is "to". "Has the ability to" is "can". "It is important to
note that" goes. "Could potentially" is "may".

Chatbot leftovers. "Here is", "let me know", "great question", "as of this writing".

Upbeat endings. "The future looks bright." End on the last useful fact.

## Style

- No em dashes. Use a full stop, a comma, a colon or brackets.
- Sentence-case headings. No emoji in headings or lists.
- Bold only where a reader would miss something without it. No bold label followed by a
  sentence as a list item; write the sentence.
- Describe how things work now. No asides about past bugs or old behaviour.
- Straight quotes in source text. Translated catalogues follow `docs/translations.md`.

## Before and after

Before: "Kaeri serves as a powerful, seamless hub for your media, offering a rich experience
across every platform."

After: "Kaeri is a native app on each platform it runs on: SwiftUI on Apple devices, Compose on
Android, WinUI on Windows, GTK on Linux. The engine underneath is the same Rust code everywhere."

Read the result aloud. If it sounds like a press release, start again.
