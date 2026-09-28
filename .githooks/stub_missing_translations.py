#!/usr/bin/env python3
"""Auto-stubs any `strings::en` function a locale hasn't caught up with yet.

Called from `.githooks/pre-commit`, never by hand. `en.rs` is the only
module with real content (see `strings/src/lib.rs`'s doc comment); `pt.rs`
and `es.rs` are expected to lag behind it, function by function, as they get
translated. Before this script existed, that lag was the committer's
problem: a new `en.rs` function with no `pt`/`es` line failed the build and
the commit with it, whether or not the committer reads either language.

Instead: for every `pub fn` in `en.rs` that a locale file neither defines
itself nor re-exports from `en` (see the `pub use super::en::{...}` block at
the top of `pt.rs`/`es.rs` -- that one is a deliberate, permanent
English-only list, not a translation debt, and covering a name there counts
as covering it here too), this appends a stub with the same signature that
returns an empty value, marked `// TODO: translate.`. A blank string reads
as obviously wrong in play -- which is the point: it is a louder flag than
quietly showing English text would be, so it does not get mistaken for a
finished translation. Re-run any time; a name this has already stubbed (or a
human has since translated for real) is a `pub fn` in the file already, so
it is never stubbed twice.

`ht.rs` is not touched -- it re-exports `en` wholesale on purpose (see its
own doc comment) and never gains per-function overrides.
"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
STRINGS_SRC = ROOT / "strings" / "src"

SIG_RE = re.compile(r"pub fn (\w+)\(([^)]*)\)\s*->\s*([^\{]+?)\s*\{", re.S)
TOP_FN_RE = re.compile(r"^pub fn (\w+)", re.M)
USE_ONE_RE = re.compile(r"pub use super::en::(\w+);")
USE_BLOCK_RE = re.compile(r"pub use super::en::\{([^}]*)\};", re.S)


def en_signatures(en_text):
    """name -> (params, return_type), in file order."""
    sigs = {}
    for m in SIG_RE.finditer(en_text):
        name, params, ret = m.group(1), m.group(2).strip(), m.group(3).strip()
        sigs[name] = (params, ret)
    return sigs


def covered_names(locale_text):
    names = set(TOP_FN_RE.findall(locale_text))
    names |= set(USE_ONE_RE.findall(locale_text))
    for block in USE_BLOCK_RE.findall(locale_text):
        names |= set(re.findall(r"\w+", block))
    return names


def empty_value_for(ret_type):
    if ret_type in ("&'static str", "&str"):
        return '""'
    if ret_type == "String":
        return "String::new()"
    m = re.fullmatch(r"\[&'static str;\s*(\d+)\]", ret_type)
    if m:
        n = int(m.group(1))
        return "[" + ", ".join(['""'] * n) + "]"
    # Unknown shape: fail loudly rather than emit something that won't
    # compile -- see the pre-commit check right after this script runs.
    raise SystemExit(
        f"stub_missing_translations.py: don't know an empty value for "
        f"return type {ret_type!r} -- teach `empty_value_for` about it."
    )


def stub_for(name, params, ret_type):
    body = empty_value_for(ret_type)
    return (
        "\n// TODO: translate.\n"
        "#[allow(unused_variables)]\n"
        f"pub fn {name}({params}) -> {ret_type} {{\n"
        f"    {body}\n"
        "}\n"
    )


def sync_locale(locale, en_sigs):
    path = STRINGS_SRC / f"{locale}.rs"
    text = path.read_text()
    covered = covered_names(text)
    missing = [name for name in en_sigs if name not in covered]
    if not missing:
        return False
    addition = "\n// --- Auto-stubbed by .githooks/pre-commit: not yet translated. ---\n"
    for name in missing:
        params, ret = en_sigs[name]
        addition += stub_for(name, params, ret)
    path.write_text(text.rstrip("\n") + "\n" + addition)
    print(
        f"stub_missing_translations.py: stubbed {len(missing)} function(s) "
        f"in strings/src/{locale}.rs: {', '.join(missing)}"
    )
    return True


def main():
    en_text = (STRINGS_SRC / "en.rs").read_text()
    en_sigs = en_signatures(en_text)
    changed = []
    for locale in ("pt", "es"):
        if sync_locale(locale, en_sigs):
            changed.append(locale)
    for locale in changed:
        print(f"strings/src/{locale}.rs")
    return 0


if __name__ == "__main__":
    sys.exit(main())
