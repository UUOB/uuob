#!/usr/bin/env bash
# Screenshots of the Mac app, for checking how it looks (mac.yml): each page and
# sheet, light and dark, and a small window. It presses keys and clicks buttons by
# name through System Events, so the app itself needs no screenshot code.
#   mac/shots.sh <UUOB.app> <data-dir> <out-dir>
# A sheet that a version doesn't have (its button isn't found) is simply left out.
set -uo pipefail
data=$2 out=$3
mkdir -p "$out"
# The kit's helpers (sdk/kit/mac/shots-lib.sh): keys, clicks that fall back to
# finding the words on screen where GitHub's runner gives buttons no names, real
# pointer moves for hover hints. `click` and `win` act on the app's pid.
RUNNER_TEMP=${RUNNER_TEMP:-$(mktemp -d)}
source "$(cd "$(dirname "$0")/.." && pwd)/sdk/kit/mac/shots-lib.sh"
full="$(cd "$1" && pwd)/Contents/MacOS/UUOB"
app_pid=

open_on() {
  # Through Launch Services, as a person starts it: started bare, its window shows
  # nothing inside it to accessibility.
  open -n -a "$(dirname "$(dirname "$(dirname "$full")")")" --env UUOB_DATA_DIR="$data" --env UUOB_SCREEN="$1"
  sleep 8
  app_pid=$(pgrep -n -f "$full")
  echo "$mode $1: pid ${app_pid:-none}" >> "$out/clicks.txt"
  deny
  # The runner's local network prompt, where it gives its buttons no names either.
  local at; at=$(seen "~t allow") && [ -n "$at" ] && pointer $at left
  win "set frontmost to true"; sleep 1
  # The same size every time (a version may remember the small window from before).
  [ "$mode" = small ] || { win "set position of front window to {12, 31}"; win "set size of front window to {1000, 676}"; }
}
quit() { kill "$app_pid" 2>/dev/null; sleep 1.5; }

# A hover hint: the pointer resting on an icon button (at a fixed place in the
# 1000x676 window at 12,31), then a shot once the hint has had time to show.
hint() { pointer "$1" "$2"; sleep 2.5; shot "hint-$3"; pointer 130 450; }   # parked on the sidebar's empty part

each_mode() {
  # Network: the share offer the demo data brings, then the page and its sheets.
  open_on network
  shot offer; esc
  shot network
  hint 927 700 rename
  hint 222 677 sidebar
  click "Rename" || pointer 927 700 left
  shot rename; esc
  # Add and Join sit under the device list.
  pointer 620 500 scroll -30; pointer 130 450
  click "Add a device" && { shot add-device; esc; }
  click "Join a network" && { shot join; esc; }
  cmd k; shot palette; esc
  se 'keystroke "?"'; sleep 1.5; shot shortcuts; esc
  se 'keystroke "c"'; sleep 1.5; shot key-c; esc
  quit

  # Access: the page, Add and each of its choices, and an app's and a person's details.
  open_on access; esc
  shot access
  hint 946 105 add
  cmd n; shot add; esc
  for c in "An app, with a code:app-code" "A person: show my code:person-code" "A person: enter their code:meet"; do
    cmd n; click "${c%:*}" && { sleep 1; shot "${c##*:}"; esc; }
    esc
  done
  # "An AI" reads as "An Al" to the text finder: its row in the Add card.
  cmd n; pointer 354 464 left; sleep 1; shot ai; esc; esc
  click "Calendar" && shot app-detail
  quit
  open_on access; esc
  click "Petra" && shot person-detail
  quit

  # Settings: the page, your name, leaving.
  open_on settings; esc
  shot settings
  click "Change name" || pointer 927 226 left
  shot name; esc
  pointer 620 500 scroll -30; pointer 130 450
  pointer 900 644 left; sleep 1; shot leave; esc   # its Leave… button, at the page's end
  quit
}

for mode in light dark; do
  [ $mode = dark ] && se 'tell appearance preferences to set dark mode to true'
  each_mode
done

# A small window (dark): the rail, narrow pages, and sheets wider than the window.
mode=small
for s in network access settings; do
  open_on $s; esc
  win 'set size of front window to {380, 480}'; sleep 2
  shot $s
  if [ $s = access ]; then cmd n; shot add; esc; cmd k; shot palette; esc; fi
  if [ $s = network ]; then click "Rename" && { shot rename; esc; }; fi
  quit
done

# Closing the window leaves UUOB in the menu bar only: out of the Dock and ⌘Tab
# (ApplicationType "UIElement"), and back to "Foreground" when it opens again.
open_on network; esc
{ echo "open: $(lsappinfo info -only ApplicationType -app UUOB)"
  cmd w; sleep 2
  echo "closed: $(lsappinfo info -only ApplicationType -app UUOB)"
  open "$1"; sleep 3   # already running: macOS asks it to reopen its window
  echo "reopened: $(lsappinfo info -only ApplicationType -app UUOB)"; } >"$out/dock.txt" 2>&1
shot reopened
quit
cat "$out/dock.txt"
ls "$out"
