#!/bin/sh
# xclick.sh "t x y; t x y; t key K; t text S" : at second t after start, X-level input on :99 (window at 112,98)
export DISPLAY=:99
start=$(date +%s.%N)
focus() { w=$(xdotool search --name "Diablo II" 2>/dev/null | head -1); [ -n "$w" ] && xdotool windowfocus "$w" 2>/dev/null; }
echo "$1" | tr ';' '\n' | while read t a b c; do
  [ -z "$t" ] && continue
  while [ "$(echo "$(date +%s.%N) - $start < $t" | bc)" = 1 ]; do sleep 0.1; done
  focus
  case "$a" in
    key) xdotool key "$b";;
    text) xdotool type --delay 80 "$b";;
    *) xdotool mousemove $((a+112)) $((b+98)); sleep 0.1; xdotool mousedown 1; sleep 0.08; xdotool mouseup 1;;
  esac
  echo "x $t $a $b"
done
