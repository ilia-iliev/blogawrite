#!/usr/bin/env bash
# How long blogawrite takes to put a document on screen, measured from the outside on
# whatever session is running — X11 or Wayland, since nothing here looks at pixels. Qt's
# scene graph says when it drew a frame and what the frame cost; the script stamps each of
# those lines as it arrives and counts from the moment the process was started, so what
# comes out is what a writer waits through: exec, dynamic linking, Qt, QML, the document,
# the first frame.
#
# Each run opens a real window for a couple of seconds and takes the focus with it.
#
#     tools/profile-render.sh                       # the sample post, five runs
#     tools/profile-render.sh -n 10 test.md
#     QT_QUICK_BACKEND=rhi tools/profile-render.sh  # against the OpenGL path
#
# The app inherits this shell's environment, so a variable set out here is one more thing
# to measure with.

set -euo pipefail
# EPOCHREALTIME writes its decimal point the locale's way, and the arithmetic below reads
# it as a plain digit string.
export LC_ALL=C

readonly root="$(cd "$(dirname "$0")/.." && pwd)"
readonly binary="$root/target/release/blogawrite"
# Only the two categories worth having: the backend's name, and a line per frame. The
# rest of the scene graph's logging is loud enough to change what it is reporting on.
readonly logging='qt.scenegraph.general=true;qt.scenegraph.time.renderloop=true'
# Frames come in a burst while the window is filling up, and then in ones and twos as
# whatever is slower than the window catches up — the checker, an image off the disk. A
# gap this long, in milliseconds, ends the burst.
readonly burst_gap=250
# How long to watch each launch for, in seconds. Long enough for the checker's first pass,
# which lands about a second in.
readonly window=2.5

runs=5
build=yes
documents=()

usage() {
    cat <<USAGE
usage: tools/profile-render.sh [-n runs] [--no-build] [document...]

  -n runs      how many times to launch it (default $runs)
  --no-build   profile target/release/blogawrite as it stands
USAGE
}

while [ $# -gt 0 ]; do
    case "$1" in
        -n) runs="$2"; shift 2 ;;
        --no-build) build=no; shift ;;
        -h|--help) usage; exit 0 ;;
        -*) usage >&2; exit 2 ;;
        *) documents+=("$1"); shift ;;
    esac
done
[ ${#documents[@]} -gt 0 ] || documents=("$root/sample/post.md")

readonly work="$(mktemp -d)"
trap 'kill "${app:-}" 2>/dev/null || true; rm -rf "$work"' EXIT

# Milliseconds since `$1`, itself an EPOCHREALTIME reading, to three places.
since() {
    local microseconds=$(( ${EPOCHREALTIME/./} - ${1/./} ))
    printf '%d.%03d' $((microseconds / 1000)) $((microseconds % 1000))
}

# Every line of the app's stderr, prefixed with how long after the start it turned up.
stamp() {
    local started="$1"
    local line
    while IFS= read -r line; do
        printf '%s\t%s\n' "$(since "$started")" "$line"
    done
}

# Launch it on `$1`, watch it for `$window`, and leave a stamped log in `$2`.
launch() {
    local document="$1" log="$2"
    local started="$EPOCHREALTIME"
    : > "$log"

    QT_LOGGING_RULES="$logging" "$binary" "$document" \
        > /dev/null 2> >(stamp "$started" > "$log") &
    app=$!
    sleep "$window"
    kill "$app" 2>/dev/null || true
    wait "$app" 2>/dev/null || true
    app=
}

# One run's numbers, from its stamped log: when the first frame was drawn, when the burst
# that follows it ended, how many frames that took, how much of the time went on drawing
# them, and how many frames were still being drawn afterwards.
read_run() {
    awk -F'\t' -v gap="$burst_gap" '
        $2 !~ /Frame rendered/ { next }
        { match($2, /in [0-9]+ms/); cost = substr($2, RSTART + 3, RLENGTH - 5) }
        !first { first = $1; end = $1; drawing = cost; frames = 1; next }
        $1 - end > gap { late++; next }
        late { late++; next }
        { end = $1; drawing += cost; frames++ }
        END {
            if (!first) { print "none"; exit }
            printf "%.1f %.1f %d %d %d\n", first, end, frames, drawing, late
        }' "$1"
}

# The middle value of the numbers on stdin.
median() {
    sort -n | awk '{ value[NR] = $1 }
                   END {
                       if (!NR) exit
                       if (NR % 2) printf "%.1f\n", value[(NR + 1) / 2]
                       else printf "%.1f\n", (value[NR / 2] + value[NR / 2 + 1]) / 2
                   }'
}

# The blank-line-separated blocks of `$1`, which is what the window has to lay out.
blocks() { awk 'BEGIN { RS = "" } END { print NR }' "$1"; }

echo "blogawrite render profile"
echo "  $(git -C "$root" log -1 --format='%h %s')$([ -n "$(git -C "$root" status --porcelain)" ] && echo ' + uncommitted changes')"
echo "  ${XDG_SESSION_TYPE:-unknown} session, $(nproc) cores, $runs runs of $window s"

if [ "$build" = yes ]; then
    built="$EPOCHREALTIME"
    cargo build --release --quiet --manifest-path "$root/Cargo.toml"
    echo "  built in $(since "$built" | awk '{ printf "%.1fs", $1 / 1000 }')"
fi
[ -x "$binary" ] || { echo "no $binary — build it first" >&2; exit 1; }

for document in "${documents[@]}"; do
    echo
    echo "$(basename "$document") — $(blocks "$document") blocks, $(wc -c < "$document") bytes"
    printf '  %-6s %9s %9s %8s %9s %7s\n' run first settled frames drawing later

    firsts="$work/firsts"; settleds="$work/settleds"
    : > "$firsts"; : > "$settleds"
    for run in $(seq 1 "$runs"); do
        log="$work/$(basename "$document").$run"
        launch "$document" "$log"

        read -r first settled frames drawing late <<<"$(read_run "$log")"
        [ "$first" != none ] || { echo "  $run: nothing was drawn — see $log" >&2; continue; }
        echo "$first" >> "$firsts"
        echo "$settled" >> "$settleds"
        printf '  %-6s %7.1fms %7.1fms %8s %7sms %7s\n' \
            "$run" "$first" "$settled" "$frames" "$drawing" "$late"
    done
    printf '  %-6s %7sms %7sms\n' median "$(median < "$firsts")" "$(median < "$settleds")"
done

cat <<'LEGEND'

first     the window's first painted frame, timed from the exec — the wait a writer sees
settled   the last frame of the burst that follows it: the document, finished
frames    frames in that burst; drawing, the milliseconds of them Qt spent rendering
later     frames after the burst — the checker's first pass, and anything still repainting
LEGEND
