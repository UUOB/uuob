#!/usr/bin/env bash
# Screenshots of the Mac app, for checking how it looks (mac.yml): each page and
# sheet, light and dark, and a small window. It presses keys and clicks buttons by
# name through System Events, so the app itself needs no screenshot code.
#   mac/shots.sh <UUOB.app> <data-dir> <out-dir>
# A sheet that a version doesn't have (its button isn't found) is simply left out.
set -uo pipefail
app=$1/Contents/MacOS/UUOB data=$2 out=$3
mkdir -p "$out"
pid=

se() { osascript -e "tell application \"System Events\" to $1" >/dev/null 2>&1 || true; }
esc() { se 'key code 53'; sleep 1.2; }
cmd() { se "keystroke \"$1\" using {command down}"; sleep 1.5; }
shot() { screencapture -x "$out/$mode-$1.png"; }

# The runner's local network prompt (not UUOB's): Don't Allow, in whichever process shows it.
deny() {
  osascript >/dev/null 2>&1 <<'EOF' || true
tell application "System Events"
  repeat with p in (every process whose name is not "UUOB")
    try
      repeat with w in (windows of p)
        repeat with b in (buttons of w)
          set n to name of b
          if n starts with "Don" and n ends with "Allow" then click b
        end repeat
      end repeat
    end try
  end repeat
end tell
EOF
}

# Clicks UUOB's first button whose title, description or tooltip is (or, with ~, contains) the text.
click() {
  local r
  r=$(osascript - "$1" <<'EOF' 2>>"$out/clicks.txt"
on run argv
  set t to item 1 of argv
  set part to t starts with "~"
  if part then set t to text 2 thru -1 of t
  tell application "System Events" to tell process "UUOB"
    set frontmost to true
    repeat with e in (entire contents of front window)
      try
        if role of e is "AXButton" then
          set ws to {}
          try
            set end of ws to name of e
          end try
          try
            set end of ws to description of e
          end try
          try
            set end of ws to value of attribute "AXHelp" of e
          end try
          repeat with w in ws
            if w is not missing value then
              if (part and (w as text) contains t) or ((w as text) is t) then
                click e
                return "ok"
              end if
            end if
          end repeat
        end if
      end try
    end repeat
  end tell
  return "missing"
end run
EOF
  )
  sleep 1.8
  echo "$1 -> $r" >> "$out/clicks.txt"
  [ "$r" = ok ] || { echo "  no button: $1"; return 1; }
}

open_on() {
  UUOB_DATA_DIR="$data" UUOB_SCREEN=$1 "$app" & pid=$!
  sleep 8; deny; sleep 1
  # The same size every time (a version may remember the small window from before).
  [ "$mode" = small ] || { se 'tell process "UUOB" to set position of front window to {12, 31}'; se 'tell process "UUOB" to set size of front window to {1000, 676}'; }
}
quit() { kill "$pid" 2>/dev/null; wait "$pid" 2>/dev/null; sleep 1; }

each_mode() {
  # Network: the share offer the demo data brings, then the page and its sheets.
  open_on network
  shot offer; esc
  shot network
  [ $mode = light ] && osascript >"$out/ax.txt" 2>&1 <<'EOF'
tell application "System Events" to tell process "UUOB"
  set out to "windows: " & (count of windows) & linefeed
  set es to entire contents of front window
  set out to out & "elements: " & (count of es) & linefeed
  repeat with e in es
    try
      set out to out & (role of e) & " | " & (name of e as text) & " | " & (description of e as text) & linefeed
    end try
  end repeat
  return out
end tell
EOF
  click "Add a device" && { shot add-device; esc; }
  click "Join a network" && { shot join; esc; }
  click "Rename" && { shot rename; esc; }
  cmd k; shot palette; esc
  se 'keystroke "?"'; sleep 1.5; shot shortcuts; esc
  se 'keystroke "c"'; sleep 1.5; shot key-c; esc
  quit

  # Access: the page, Add and each of its choices, and an app's and a person's details.
  open_on access; esc
  shot access
  cmd n; shot add; esc
  for c in "An app, with a code:app-code" "A person: show my code:person-code" "A person: enter their code:meet" "An AI:ai"; do
    cmd n; click "${c%:*}" && { sleep 1; shot "${c##*:}"; esc; }
    esc
  done
  click "~Calendar" && shot app-detail
  quit
  open_on access; esc
  click "~Petra" && shot person-detail
  quit

  # Settings: the page, your name, leaving.
  open_on settings; esc
  shot settings
  click "Change name" && { shot name; esc; }
  click "~Leave" && { shot leave; esc; }
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
  se 'tell process "UUOB" to set size of front window to {380, 480}'; sleep 2
  shot $s
  if [ $s = access ]; then cmd n; shot add; esc; cmd k; shot palette; esc; fi
  if [ $s = network ]; then click "Rename" && { shot rename; esc; }; fi
  quit
done
ls "$out"
