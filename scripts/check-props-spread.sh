#!/usr/bin/env bash
# Guard the one bits-ui idiom this app cannot get wrong: an attribute written
# *after* a `{...props}` spread silently replaces whatever bits-ui put there.
#
# The expensive case is `style`. While a modal layer is open bits-ui sets
# `document.body { pointer-events: none }` (use-body-scroll-lock.svelte.js) and
# re-enables the panel with `pointer-events: auto`, delivered inside
# `props.style` — as a string, because svelte-toolbelt's `mergeProps`
# stringifies its style object. A literal `style="…"` after the spread is a
# later key in the same compiled object literal, so it drops that declaration;
# `pointer-events` inherits, and every control in the dialog goes click-dead.
# No error, no warning, `pnpm check` clean. That shipped in the bits-ui
# migration (#22) and took a bug report to find (#63) — hence a check rather
# than a comment.
#
# Event handlers are the same class of bug: the Content props carry
# FocusScope's `onkeydown`, which is the Tab focus trap.
#
# Rule: from a `{...props}` / `{...wrapperProps}` spread to the end of that open
# tag, the only permitted `style=` is `style={mergeStyle(…)}` (see
# src/lib/util.ts) and no `on<event>=` is permitted at all.
#
# Heuristic limits, both of which fail *open* (a missed warning, never a false
# alarm): a literal `>` inside an attribute value ends tag-scanning early, as
# does an arrow function written inline after the spread. Neither occurs today.
#
# Usage: scripts/check-props-spread.sh   — also run by `pnpm check`.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

bad="$(
  awk '
    FNR == 1 { inTag = 0 }
    {
      if (!inTag) {
        i = match($0, /\{\.\.\.(props|wrapperProps)\}/)
        if (i == 0) next
        inTag = 1
        rest = substr($0, i)
      } else {
        rest = $0
      }
      # Only scan as far as the end of the open tag.
      endTag = index(rest, ">")
      if (endTag > 0) { scan = substr(rest, 1, endTag); inTag = 0 } else { scan = rest }

      if (scan ~ /style=/ && scan !~ /style=\{mergeStyle\(/)
        printf "%s:%d: bare `style=` after a props spread — use style={mergeStyle(props, …)}\n", FILENAME, FNR
      if (scan ~ /[[:space:]]on[a-z]+=/)
        printf "%s:%d: event handler after a props spread — it will clobber the bits-ui handler\n", FILENAME, FNR
    }
  ' "$here"/src/lib/components/*.svelte
)"

if [[ -n "$bad" ]]; then
  printf '%s\n\n' "$bad" >&2
  echo "See mergeStyle() in src/lib/util.ts for the why." >&2
  exit 1
fi

echo "props-spread guard: ok ($(grep -l '{\.\.\.props}' "$here"/src/lib/components/*.svelte | wc -l) files with spreads)"
