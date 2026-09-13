#!/usr/bin/env bash
#
# docs_style.sh -- roog's documentation linter.
#
# Checks every page under docs/ against the house style, which is written down
# in docs/explanation/documentation-style.md. Each check below is one section
# of that page; if the two ever disagree, the page is the specification and
# this script is the bug.
#
# Eight checks, in the order a page is read:
#
#   1. header block        the title underline, Audience, Prerequisites
#   2. the third key       Status for reference/, This is for explanation/
#   3. heading depth       setext, then ###, and nothing below that
#   4. code blocks         indented -- bar a reference/ declaration or a
#                          ```mermaid diagram, which have nowhere else to live
#   5. forward pointers    a See also block -- or, in a tutorial, a
#                          Where to go next block -- with every entry glossed
#   6. dash consistency    -- or em dash, but not both in one page
#   7. restated dice       `2d6` in prose is a number that will go stale
#   8. broken links        a page-to-page link that lands nowhere, and a
#                          `models/src/...` path that names a file that is gone
#
# Line width is deliberately *not* checked. These pages are read rendered as
# often as they are `cat`ed, and a rendered paragraph reflows to the reader's
# window; a column count is a rule with nobody behind it.
#
# Why not prettier, or any other markdown formatter
# -------------------------------------------------
# Because it would fight the style rather than enforce it. roog's pages are
# hand-wrapped plain text with setext headings, four-space code blocks and a
# column-aligned See-also block, all of which exist so that `cat docs/…` reads
# correctly in a terminal. Every markdown formatter worth the name reflows
# tables, converts setext headings to ATX and rewraps or unwraps paragraphs on
# its own terms -- and the first thing it would do to this repository is undo
# the reason the pages look like this. It would also put a Node toolchain
# between a contributor and a spelling fix, in a project whose README says
# "terminal only, one binary, no assets".
#
# So this reports and never rewrites. Where a check fires, the fix is a
# judgement call about where a sentence should break, and that is a person's
# job.
#
# It lints docs/ and nothing else. `gdd.md` is flowing markdown by design,
# `MANUAL.md` is a player-facing document in a different voice, and the root
# `README.md` is a landing page -- none of the three answers to the page shape
# below, and all three say so in docs/explanation/documentation-style.md.
#
# Usage:
#   ./docs_style.sh                 lint every page under docs/
#   ./docs_style.sh docs/how-to     lint one directory or one file
#   ./docs_style.sh --quiet         print only what failed
#   ./docs_style.sh --help
#
# Exit status is 0 when every check passes, 1 when any fails -- so it drops
# into a pre-commit hook or a CI step as-is:
#
#   ./docs_style.sh || exit 1

set -uo pipefail

cd "$(dirname "$0")" || exit 1

# The presentation vocabulary is the pipelines' own. Two scripts that print
# differently read as two projects.
. compat/lib.sh

# ---------------------------------------------------------------------------
# Options
# ---------------------------------------------------------------------------

QUIET=0
TARGETS=()

while [ $# -gt 0 ]; do
  case "$1" in
    --quiet)  QUIET=1 ;;
    --help|-h)
      sed -n '3,48p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    -*) bad "unknown flag: $1"; note "try ./docs_style.sh --help"; exit 1 ;;
    *)  TARGETS+=("$1") ;;
  esac
  shift
done

if [ ${#TARGETS[@]} -eq 0 ]; then
  TARGETS=(docs)
fi

PAGES=$(find "${TARGETS[@]}" -name '*.md' 2>/dev/null | sort)
if [ -z "$PAGES" ]; then
  bad "no pages found under: ${TARGETS[*]}"
  exit 1
fi

PROBLEMS=0
hit() { PROBLEMS=$((PROBLEMS+1)); bad "$1"; }
pass() { [ "$QUIET" = 1 ] || ok "$1"; }

# `docs/README.md` is the index and opens with the table instead of a header
# block; `gdd.md` and `MANUAL.md` are not docs/ pages at all and answer to
# their own shape. Everything else carries the full header.
exempt_from_header() {
  case "$1" in
    docs/README.md|README.md|MANUAL.md|gdd.md) return 0 ;;
    *) return 1 ;;
  esac
}

# An ADR is a record with a lifecycle, and `Status` is what records it --
# proposed, accepted, superseded. It keeps that key wherever it is filed.
is_adr() { case "$1" in */adr-*) return 0 ;; *) return 1 ;; esac; }

# Prose only: no indented block, no table row. What is left is the sentences,
# which is what checks 6 and 7 are about.
prose_lines() {
  grep -nvP '^\s{2,}|\|' "$1"
}

# ---------------------------------------------------------------------------
# 1. The header block
# ---------------------------------------------------------------------------

stage "Header block"

MISSING=""
for f in $PAGES; do
  exempt_from_header "$f" && continue
  grep -q '^    Audience  ' "$f"      || MISSING="$MISSING$f: no Audience"$'\n'
  # An ADR keeps the header ADRs have everywhere -- Status, Audience,
  # Supersedes, Related -- and has no Prerequisites line to keep.
  is_adr "$f" || grep -q '^    Prerequisites  ' "$f" \
    || MISSING="$MISSING$f: no Prerequisites"$'\n'
  # The title is setext: a line, then a run of `=`.
  head -2 "$f" | tail -1 | grep -qP '^=+$'  || MISSING="$MISSING$f: no title underline"$'\n'
done

if [ -n "$MISSING" ]; then
  while IFS= read -r m; do [ -n "$m" ] && hit "$m"; done <<< "$MISSING"
  note "every page but docs/README.md opens with Audience and Prerequisites"
else
  pass "every page names its audience and its prerequisites"
fi

# ---------------------------------------------------------------------------
# 2. The third key
# ---------------------------------------------------------------------------

stage "The promise each page makes"

WRONG=""
want_third() {
  is_adr "$1" && { echo "Status"; return; }
  case "$1" in
    docs/reference/*)   echo "Status" ;;
    docs/explanation/*) echo "This is" ;;
    *)                  echo "" ;;
  esac
}
for f in $PAGES; do
  exempt_from_header "$f" && continue
  third=$(grep -oP '^    (Status|This is)(?=  )' "$f" | head -1 | xargs)
  want=$(want_third "$f")
  [ "$third" = "$want" ] || \
    WRONG="$WRONG$f: third key is '${third:-none}', wants '${want:-none}'"$'\n'
done

if [ -n "$WRONG" ]; then
  while IFS= read -r w; do [ -n "$w" ] && hit "$w"; done <<< "$WRONG"
  note "reference/ and ADRs promise Status; explanation/ promises 'This is';"
  note "how-to/ and tutorial/ promise nothing beyond their title"
else
  pass "every page's third key matches its kind"
fi

# One reference page is allowed to disagree with the rest only if it says so.
NO_FORMULA=""
for f in $(echo "$PAGES" | grep '^docs/reference/'); do
  tr '\n' ' ' < "$f" | tr -s ' ' \
    | grep -q 'the source is right and this page is a bug' \
    || NO_FORMULA="$NO_FORMULA $f"
done
if [ -n "$NO_FORMULA" ]; then
  for f in $NO_FORMULA; do warn "no 'the source is right' formula: $f"; done
  note "that sentence is what makes a stale reference page a defect"
fi

# ---------------------------------------------------------------------------
# 3. Heading depth
# ---------------------------------------------------------------------------

stage "Heading depth"

DEEP=$(grep -rn '^####' $PAGES 2>/dev/null)
ATX=$(grep -rn '^##\? ' $PAGES 2>/dev/null)

if [ -n "$DEEP" ]; then
  echo "$DEEP" | while read -r l; do bad "heading past ###: $l"; done
  PROBLEMS=$((PROBLEMS+1))
fi
if [ -n "$ATX" ]; then
  echo "$ATX" | while read -r l; do bad "ATX heading where setext is the style: $l"; done
  PROBLEMS=$((PROBLEMS+1))
fi
[ -z "$DEEP$ATX" ] && pass "setext for the top two levels, ### for the third"

# ---------------------------------------------------------------------------
# 4. Fenced code
# ---------------------------------------------------------------------------

stage "Code blocks"

# Four-space indentation for examples, with two exceptions the corpus keeps:
# `reference/` fences a bare *declaration* -- a signature, a struct shape -- as
# a heading for the paragraph under it, and any page may fence a ```mermaid
# diagram, which has nowhere else to live. Indentation is what survives `cat`,
# `less` and a paste into a commit message; a fence around an example is noise.
STRAY_FENCE=""
for f in $PAGES; do
  case "$f" in docs/reference/*) continue ;; esac
  # Openers only: a block's closing ``` is not a fence of its own, and
  # counting it made every mermaid diagram look like a stray code block.
  n=$(awk '/^```/ { if (open) { open=0; next } open=1; if ($0 !~ /^```mermaid/) n++ } END { print n+0 }' "$f")
  [ "$n" -gt 0 ] && STRAY_FENCE="$STRAY_FENCE$f: $n non-mermaid fence(s)"$'\n'
done

if [ -n "$STRAY_FENCE" ]; then
  while IFS= read -r l; do [ -n "$l" ] && warn "$l"; done <<< "$STRAY_FENCE"
  note "indent four spaces; fences are for reference/ declarations and mermaid"
else
  pass "code is indented; fences hold declarations and diagrams"
fi

# ---------------------------------------------------------------------------
# 5. See also
# ---------------------------------------------------------------------------

stage "See also"

NO_SEE=""
UNGLOSSED=""
for f in $PAGES; do
  exempt_from_header "$f" && continue
  # A tutorial points *forward* rather than sideways, so it closes with
  # "Where to go next" instead. Same shape, different promise.
  grep -qP '^(See also|Where to go next)$' "$f" || { NO_SEE="$NO_SEE $f"; continue; }
  # Two-space indent, a target, then a gloss. A bare filename tells the reader
  # nothing about whether to follow it.
  while IFS= read -r entry; do
    printf '%s' "$entry" | grep -qP '^  \S+\s{2,}\S' || UNGLOSSED="$UNGLOSSED $f"
  done < <(sed -n '/^\(See also\|Where to go next\)$/,$p' "$f" | grep -P '^  \S')
done

if [ -n "$NO_SEE" ]; then
  for f in $NO_SEE; do hit "nothing to read next: $f"; done
  note "it is the last section on every page, and the gloss is the point"
else
  pass "every page ends by pointing somewhere else"
fi
[ -n "$UNGLOSSED" ] && for f in $(printf '%s\n' $UNGLOSSED | sort -u); do
  warn "See also entry with no gloss: $f"
done

# ---------------------------------------------------------------------------
# 6. Dashes
# ---------------------------------------------------------------------------

stage "Dashes"

MIXED=""
for f in $PAGES; do
  # Prose only, and with backticked spans blanked first: `-- content` is a
  # command-line flag, not punctuation.
  flat=$(prose_lines "$f" | sed 's/`[^`]*`//g')
  a=$(printf '%s\n' "$flat" | grep -c ' -- ')
  b=$(printf '%s\n' "$flat" | grep -c '—')
  [ "$a" -gt 0 ] && [ "$b" -gt 0 ] && MIXED="$MIXED$f: $a of '--', $b of em dash"$'\n'
done

if [ -n "$MIXED" ]; then
  while IFS= read -r m; do [ -n "$m" ] && warn "$m"; done <<< "$MIXED"
  note "either is fine; pick the one the page already has and stay with it"
else
  pass "no page mixes its dashes"
fi

# ---------------------------------------------------------------------------
# 7. Restated tuning numbers
# ---------------------------------------------------------------------------

stage "Restated tuning numbers"

# The single most reliable source of stale documentation in this repository:
# a dice expression copied out of constants.rs into prose, where nothing will
# ever update it. Name the constant instead.
#
# `gdd.md` is exempt: it is a design document, not a description of the code,
# and it is allowed to say what the game is *meant* to roll.
DICE=""
for f in $PAGES; do
  # Prose only. A field table explaining that `power` is "the attack die:
  # rolls 1d[power]" is describing the column, not pinning a value, and it
  # lives in an indented block that `prose_lines` already skips.
  found=$(prose_lines "$f" | sed 's/`[^`]*`//g' | grep -nP '(?<![\[\w])\d+d\d+')
  [ -n "$found" ] && DICE="$DICE$f: $(printf '%s' "$found" | head -3 | tr '\n' ';')"$'\n'
done

if [ -n "$DICE" ]; then
  while IFS= read -r l; do [ -n "$l" ] && warn "dice literal in prose -- $l"; done <<< "$DICE"
  note "name the constant instead; a copied number is one nobody will update"
else
  pass "no dice expression is restated where a constant would do"
fi

# ---------------------------------------------------------------------------
# 8. Links that point at nothing
# ---------------------------------------------------------------------------

stage "Cross-references"

# Source paths the docs *qualify* -- `models/src/map/levels.rs` rather than a
# bare `levels.rs`. A bare filename is prose and is left alone; a qualified one
# is a claim about where something lives, and the split above is exactly the
# kind of change that falsifies a pile of them at once.
STALE_SRC=""
for f in $PAGES; do
  for path in $(grep -oP '`(models|engine|perf|compat|particle-core|docs)/[^`[:space:]]+`' "$f" \
                  | tr -d '`' | grep -v '[*]' | grep -v '::' | grep -v '/$'); do
    [ -e "$path" ] || STALE_SRC="$STALE_SRC$f -> $path"$'\n'
  done
done
STALE_SRC=$(printf '%s' "$STALE_SRC" | sort -u)
if [ -n "$STALE_SRC" ]; then
  while IFS= read -r l; do [ -n "$l" ] && hit "no such file: $l"; done <<< "$STALE_SRC"
  note "a qualified path is a claim about where something lives"
else
  pass "every source path the docs name exists"
fi

BROKEN=""
for f in $PAGES; do
  dir=$(dirname "$f")
  # A backticked token ending in `.md`, or a See-also target. It counts as a
  # link if it resolves from the page it is written on -- or from the
  # workspace root, which is how a page refers to `docs/…` from outside.
  for target in $(grep -oP '`[^` ]+\.md`' "$f" | tr -d '`') \
                $(sed -n '/^\(See also\|Where to go next\)$/,$p' "$f" \
                    | grep -oP '^  \K\S+\.md'); do
    [ -f "$dir/$target" ] && continue
    [ -f "$target" ] && continue
    BROKEN="$BROKEN$f -> $target"$'\n'
  done
done

BROKEN=$(printf '%s' "$BROKEN" | sort -u)
if [ -n "$BROKEN" ]; then
  while IFS= read -r b; do [ -n "$b" ] && hit "link to a file that is not there: $b"; done <<< "$BROKEN"
  note "paths are relative to the page they are written on"
else
  pass "every page-to-page link resolves"
fi

# ---------------------------------------------------------------------------
# Verdict
# ---------------------------------------------------------------------------

heading "Verdict"

PAGE_COUNT=$(printf '%s\n' $PAGES | wc -l | xargs)

if [ "$PROBLEMS" -eq 0 ]; then
  ok "$PAGE_COUNT pages, house style kept"
  note "the style itself: docs/explanation/documentation-style.md"
  exit 0
fi

bad "$PROBLEMS check(s) failed across $PAGE_COUNT pages"
note "the style itself: docs/explanation/documentation-style.md"
exit 1
