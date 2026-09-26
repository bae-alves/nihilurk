Contributing
============

Bug reports and code changes go through GitHub: open an issue, or send a pull
request.


Translations
------------

nihilurk ships in four languages -- English, Portuguese, Spanish, Haitian
Creole -- one binary per language, picked at build time (see
`strings/src/lib.rs`). English is the reference translation. The other three
are beta: they compile and run correctly today because `strings/src/pt.rs`,
`es.rs` and `ht.rs` each re-export English wholesale, function by function,
until someone translates that function for real. A beta binary shows a notice
saying so on startup.

Found an English sentence where a translation should be? Open an issue, or
send a PR: pick a function in `strings/src/en.rs`, translate it, and add the
translated version to the matching language file (overriding the `pub use
super::en::*` re-export for that one function -- see the comment at the top
of `strings/src/pt.rs` for the exact shape). Once every function in a
language file has a real translation, delete that language's `Some(...)` arm
in `beta_notice()` (`strings/src/lib.rs`) in the same PR -- a language stops
being beta the day it earns it, not before.

One known rough edge, flagged in a code comment where it lives, that a
translation can't paper over and nobody has designed around yet:

- `models/src/identify.rs`: articles and pluralization ("a dagger", "7
  arrows") are English grammar rules, hardcoded. Portuguese and Spanish need
  gender agreement; none of the three need "-s" for a plural in the general
  case. This needs real design, not just new strings.

That's out of scope for a translation PR. If you want to take it on, say so
in an issue first -- it's a small data-model change, not a string edit.

(Message-log coloring used to have the same problem -- it matched English
substrings like "curse" in the already-composed sentence. It's fixed now:
every log line carries its own `LogCategory`, set by whoever writes the
message, so a translation can't break it. See `models/src/components.rs`'s
`LogCategory`/`LogEntry` and `hud::log_paint`.)
