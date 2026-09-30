# guccify

A tiny, dependency-free static site generator built specifically for
`zacsimile.github.io`. Like `mksite.c` (from https://maurycyz.com/), it 
deliberately favors a small amount of hardcoded site knowledge over a 
configuration framework.

It also builds `blog.html` and `rss.xml` from pages marked `category: blog`.

## Build the site

From this directory:

```sh
cargo run
```

The default output is the parent directory (the GitHub Pages repository root).
Only generated HTML files and `rss.xml` are touched; `css/`, `images/`, and `files/` stay where
they are. To preview without changing files:

```sh
cargo run -- --check
```

You can also select alternate directories:

```sh
cargo run -- --content content --output /tmp/guccify-preview
```

## Add or edit a page

Pages in `content/` have a short metadata header followed by `---` and an HTML
fragment. Supported fields are:

- `title` and `output` (required)
- `nav`: `about`, `research`, `blog`, or blank
- `mathjax`: `true` to load MathJax 3
- `category`: set to `blog` to add a page to the blog index and RSS feed
- `date`: creation/publication date, required for blog pages, in `YYYY-MM-DD` format
- `date_modified`: optional last modification date for blog pages, in `YYYY-MM-DD` format; must not precede `date`
- `summary`: required for blog pages and used as the RSS description
- `layout`: `bare` for the 404-style page
- `footer`: optional custom footer HTML

The output path, shared navigation, social links, stylesheets, and normal footer
are assembled by the generator.

For example, an updated blog post can use:

```text
category: blog
date: 2024-10-30
date_modified: 2026-09-29
```

At the bottom of the post, the generator displays “Created October 30, 2024” followed by “Last modified
September 29, 2026”. Omit `date_modified` (or leave it blank) to show only the
creation date. Both dates must be valid calendar dates. Set modification dates
explicitly when editing a post; rebuilding does not change them. Blog ordering
and RSS publication dates continue to use `date`.
