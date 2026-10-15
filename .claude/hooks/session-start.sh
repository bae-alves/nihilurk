#!/usr/bin/env bash
#
# SessionStart: gives a Claude Code cloud session the Lua that CONTRIBUTING.md
# asks for. `rustfmt.lua` runs after every edit, and the play-nihilurk skill
# draws its screens with `screen.lua`; a fresh cloud container has neither
# Lua nor a package index. A local machine is left alone: there, Lua is yours
# to install.

set -euo pipefail

[[ "${CLAUDE_CODE_REMOTE:-}" == true ]] || exit 0
command -v lua > /dev/null && exit 0

sudo=()
(( EUID == 0 )) || sudo=(sudo)
"${sudo[@]}" apt-get update -qq
"${sudo[@]}" env DEBIAN_FRONTEND=noninteractive apt-get install -y -qq lua5.4 > /dev/null
lua -v
