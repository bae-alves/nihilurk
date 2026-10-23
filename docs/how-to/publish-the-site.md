How to publish the documentation site
=====================================

    Audience       The maintainer, and anyone who changes how the site
                   looks or what it lists.
    Prerequisites  mdBook 0.5.4 (`cargo install mdbook --locked
                   --version 0.5.4`) to build it on your machine, and
                   Lua. CI needs neither from you.
    Result         `MANUAL.md` and `docs/` on GitHub Pages as a site a
                   search engine or an AI crawler can read, with a
                   sitemap, `robots.txt` and `llms.txt`.

The site is not a second copy of the docs. `site/build.lua` stages the pages from where they live, and mdBook turns them into HTML. Nothing it makes is committed.


Quick commands
--------------

    lua site/build.lua                     stage the pages in target/site-book/
    mdbook build target/site-book          build the site into target/site-book/book
    lua site/finish.lua                    give each built page its own tags
    mdbook serve target/site-book          read it at localhost:3000
    lua site/build_test.lua                the script's test

Pushing to `master` runs `.github/workflows/pages.yml` when `README.md`, `MANUAL.md`, `docs/`, `site/` or the workflow changes.


What the script does
--------------------

It copies `README.md`, `MANUAL.md` and every page under `docs/` into `target/site-book/src/`, keeping their paths. Then:

    links         a backticked path that names a page, such as
                  `../reference/cli-and-env.md`, becomes a link. A path
                  that names nothing stays as written. Code blocks are
                  left alone.
    SUMMARY.md    the contents list: the landing page, the manual, then
                  tutorials, how-to guides, reference, explanation.
                  Each entry takes its title from the page's first heading.
    sitemap.xml   one line per page, with the day of its last commit.
    robots.txt    allows everything and points at the sitemap.
    llms.txt      `site/llms.txt`, for AI answer engines.
    og-image.png  `site/og-image.png`, the picture link previews show
                  (1200x630).
    pages.tsv     each page's html path, url and description, for the
                  next step. It is not published.

It refuses a page with no heading, and names it.

The site URL is `site-url` in `site/book.toml`, and nowhere else. The script writes it into the sitemap, `robots.txt`, `llms.txt` and the structured data in `site/theme/head.hbs`. A custom domain is a one-line change there.

`site/finish.lua` runs after `mdbook build`. For each page in `pages.tsv` it writes the page's own description into the description, `og:description` and `twitter:description` tags, and adds a canonical link and `og:url`. A page's description is its first prose paragraph, cut at 160 characters; the landing page keeps the book's. Print and 404 pages are left alone.


The facts a crawler reads
-------------------------

`site/llms.txt` states what nihilurk is, the install line, the languages and the license. Each of those is a fact that `README.md` or `engine/Cargo.toml` also holds. When you change one, change both: nothing checks it.

The description in `site/book.toml` is the landing page's, and the fallback in the head template. `site/finish.lua` replaces it on every other page.

The workflow checks out full history (`fetch-depth: 0`) so the sitemap's `lastmod` is right. A shallow clone would give every page the same day.


Before the first deploy
-----------------------

Do this once. In the repository's Settings, under Pages, set Source to "GitHub Actions". Then run the workflow from the Actions tab, or push a change under `docs/`. Check `https://bae-alves.github.io/nihilurk/sitemap.xml`.

Then tell the search engines. Submit that sitemap in Google Search Console and Bing Webmaster Tools. Both need your account, so no script does it.


When it goes wrong
------------------

    The script names a page    That page has no heading. Add one.
    A build warning about      Prose holds a bare `<word>`, which HTML
    an unclosed HTML tag       reads as a tag and hides. Put it in backticks.
    The workflow says the      Source is not set to "GitHub Actions" yet.
    site is not enabled
    mdBook moved on            The version and its checksum are two
                               lines in `pages.yml`. Change both, build
                               it here first, and run `sha256sum` on the
                               tarball.


See also
--------

    ../../site/build.lua                  the script
    ../../site/build_test.lua             its test
    ../../.github/workflows/pages.yml     what a push to master starts
    update-the-docs.md                    which pages a change touches
    ../explanation/documentation-style.md the house style the pages keep
