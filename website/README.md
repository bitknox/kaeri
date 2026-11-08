# Kaeri website

The site at [kaeri.app](https://kaeri.app): the home, features, download and legal pages, and the
docs. It is built with Astro and Starlight, and it uses Bun with the committed `bun.lock`.

## Layout

```
.
├── public/                 static files, served as they are
├── scripts/                app shots, feature videos, icons and the pseudo-locale build
├── src/
│   ├── components/         page sections and the Starlight overrides
│   ├── content/
│   │   ├── docs/docs/      the docs, one Markdown or MDX file per page
│   │   ├── i18n/en.json    the site's own strings
│   │   └── legal/          the legal page
│   ├── data/               the feature matrix, screenshot and video lists, site and version data
│   ├── pages/              home, features, download, legal and 404
│   └── styles/
├── astro.config.mjs
└── package.json
```

Starlight turns each file under `src/content/docs/` into a route named after its path. Images
can go in `src/assets/` and be linked relatively from Markdown.

## Commands

Run these from `website/`:

| Command                          | What it does                                 |
| :------------------------------- | :------------------------------------------- |
| `bun install --frozen-lockfile`  | Installs the locked dependencies             |
| `bun run dev`                    | Starts the dev server at `localhost:4321`    |
| `bun run build`                  | Builds the site into `./dist/`               |
| `bun run preview`                | Serves the production build locally          |
| `bun run test`                   | Runs the unit tests under `src/`             |
| `bun run build:pseudo`           | Builds a pseudo-locale copy to check strings |

The copy follows [docs/writing.md](../docs/writing.md).
