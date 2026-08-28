#!/usr/bin/env bash
# CodeGraph installer.
#
#   ./install.sh                      build and install the CLI, then the skill for every project
#   ./install.sh --cli                the CLI only
#   ./install.sh --skill              the skill only, for every project (~/.claude/skills)
#   ./install.sh --project <path>     the skill for one project (<path>/.claude/skills)
#   ./install.sh --uninstall          remove the CLI and the skill installed for every project
#
# Nothing here touches a project's source. The skill is a directory of
# Markdown; installing it for one project writes only under that project's
# `.claude/skills/codegraph/`.
set -euo pipefail

SOURCE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SKILL_SOURCE="$SOURCE_DIR/skills/codegraph"
GLOBAL_SKILLS="${CLAUDE_SKILLS_DIR:-$HOME/.claude/skills}"

say() { printf '%s\n' "$*"; }
die() { printf 'install: %s\n' "$*" >&2; exit 1; }

install_cli() {
  command -v cargo >/dev/null 2>&1 || die "cargo is not on PATH; install Rust from https://rustup.rs"
  say "Building the CLI (release). This takes a few minutes the first time."
  # --offline keeps a warm registry from stalling on a slow network; a cold
  # checkout needs the index, so fall back to the online path.
  cargo install --path "$SOURCE_DIR/crates/codegraph-cli" --force --offline 2>/dev/null ||
    cargo install --path "$SOURCE_DIR/crates/codegraph-cli" --force
  say "Installed: $(command -v codegraph || echo "$HOME/.cargo/bin/codegraph")"
  say "Check it with: codegraph summary . --no-semantic"
}

install_skill() {
  local target_root="$1" label="$2"
  [ -d "$SKILL_SOURCE" ] || die "skill source not found at $SKILL_SOURCE"
  mkdir -p "$target_root"
  rm -rf "$target_root/codegraph"
  cp -R "$SKILL_SOURCE" "$target_root/codegraph"
  say "Installed the skill for $label: $target_root/codegraph"
}

uninstall() {
  if command -v cargo >/dev/null 2>&1; then
    cargo uninstall codegraph-cli 2>/dev/null || say "The CLI was not installed through cargo."
  fi
  if [ -d "$GLOBAL_SKILLS/codegraph" ]; then
    rm -rf "$GLOBAL_SKILLS/codegraph"
    say "Removed $GLOBAL_SKILLS/codegraph"
  fi
  say "A skill installed for a single project stays where it is; remove"
  say "<project>/.claude/skills/codegraph by hand if you want it gone."
}

case "${1:-}" in
  "")
    install_cli
    install_skill "$GLOBAL_SKILLS" "every project"
    ;;
  --cli)
    install_cli
    ;;
  --skill)
    install_skill "$GLOBAL_SKILLS" "every project"
    ;;
  --project)
    [ $# -ge 2 ] || die "--project needs a path"
    project="${2%/}"
    [ -d "$project" ] || die "no such directory: $project"
    install_skill "$(cd "$project" && pwd)/.claude/skills" "$project"
    ;;
  --uninstall)
    uninstall
    ;;
  -h|--help)
    awk 'NR > 1 { if ($0 !~ /^#/) exit; sub(/^# ?/, ""); print }' "${BASH_SOURCE[0]}"
    ;;
  *)
    die "unknown option: $1 (try --help)"
    ;;
esac
