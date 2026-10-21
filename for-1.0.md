# For 1.0

## Text and translation

- **Placeholder English.** 41 strings in `strings/src/en.rs` carry `// TODO: placeholder English; needs a human's pass.` Read each one, reword it or keep it, delete the tag. This is the art pass, so take it slowly.
- **Portuguese and Spanish stubs.** 36 empty `// TODO: translate.` stubs each in `pt.rs` and `es.rs`, plus the strings tagged as machine translation.
- **Haitian Creole.** `ht.rs` re-exports English wholesale. Needs a native speaker.
- **Spanish and Haitian Creole review.** No native speaker has read either one (`CONTRIBUTING.md`, "Translations").
- **Beta notice.** Delete a language's arm in `beta_notice()` (`strings/src/lib.rs`) when every function in its file is translated.
- **Articles and plurals.** `models/src/identify.rs` hardcodes English "a dagger", "7 arrows". pt and es need gender agreement. This needs design, not strings.

## Design

- **Other `scs.md` weak and ugly points.** Assumptions checked late; no conflict detection between systems; menus, targeting and looking still change state at the key; the planner and the applier describe one decision twice; spawn and despawn authority is spread out; prose other than the two counts is unchecked.
- **Relationship indexes.** Declined until the world holds thousands of entities.
- **Ideas from other games.** `.claude/concepts-from-other-games.md` (local, untracked). Each must pass the three-question test at its top.

## Release tooling

- **AUR job.** Waits on the account embargo. When it lifts, add one `aur` job to `release.yml` (`needs: publish`, `if: vars.AUR_ENABLED == 'true'`): `archlinux` container, non-root user, tag tarball sha into a copy of `aur/PKGBUILD`, `makepkg --printsrcinfo`, push to `ssh://aur@aur.archlinux.org/nihilurk.git` with a deploy-key secret. The repo PKGBUILD keeps `sha256sums=('SKIP')`.

## Content and docs

- **New content.** Monsters, spells, items, bodies. Check `.claude/tutorial-inventions.md` before naming anything, since the tutorials use those names.
- **New doc pages.** Fix wrong pages before 1.0. Add new ones after.
