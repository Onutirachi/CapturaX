#!/usr/bin/env bash
# Records the preview audio's own delay inside the PC with PipeWire (research R10 of the
# preview-audio spec). Run it while the app plays a preview with sound:
#
#   pipewire-measure.sh start                   null sink capturax-measure; the card on its left
#                                               channel, the app's playback on its right
#   pipewire-measure.sh record <file> <seconds> records the sink's monitor into <file> (WAV)
#   pipewire-measure.sh stop                    removes the sink, and with it its links
#
# The app's own links are left alone: it keeps playing on the real output, so its
# `[audio-source]` stats line gives that output's latency in the same run.
# Needs pw-cli, pw-dump, pw-link, pw-record and jq.

set -euo pipefail

SINK=capturax-measure
INPUT_STREAM='Preview audio input'
OUTPUT_STREAM='Preview audio'

die() {
    echo "pipewire-measure: $*" >&2
    exit 1
}

usage() {
    sed -n '2,12p' "$0" | sed 's/^# \{0,1\}//'
    exit "${1:-0}"
}

for tool in pw-cli pw-dump pw-link pw-record jq; do
    command -v "$tool" >/dev/null || die "$tool not found"
done

# The newest node whose property has this value (and with this media.class, when given);
# empty when none. The app's streams share node.name "capturax"; media.name tells them apart.
node_id() { # dump property value [class]
    jq -r --arg key "$2" --arg value "$3" --arg class "${4:-}" '
        [.[] | select(.type == "PipeWire:Interface:Node"
            and .info.props[$key] == $value
            and ($class == "" or .info.props["media.class"] == $class)) | .id]
        | max // empty' <<<"$1"
}

# A node's port in this direction for this channel: the channel, else MONO, else the first.
# Monitor ports are skipped.
port_id() { # dump node direction channel
    jq -r --arg node "$2" --arg dir "$3" --arg channel "$4" '
        [.[] | select(.type == "PipeWire:Interface:Port"
            and .info.direction == $dir
            and (.info.props["node.id"] | tostring) == $node
            and (.info.props["port.monitor"] | not))] as $ports
        | ([$ports[] | select(.info.props["audio.channel"] == $channel)]
            + [$ports[] | select(.info.props["audio.channel"] == "MONO")]
            + ($ports | sort_by(.id)))
        | (first | .id) // empty' <<<"$1"
}

# The node linked into this node's input; empty when none.
feeding_node() { # dump node
    jq -r --arg node "$2" '
        [.[] | select(.type == "PipeWire:Interface:Link"
            and (.info["input-node-id"] | tostring) == $node) | .info["output-node-id"]]
        | first // empty' <<<"$1"
}

node_label() { # dump node
    jq -r --arg node "$2" '
        .[] | select(.type == "PipeWire:Interface:Node" and (.id | tostring) == $node)
        | .info.props | "\(.["node.description"] // .["media.name"] // .["node.name"]) (\(.["node.name"]))"' <<<"$1"
}

link_exists() { # dump output-port input-port
    jq -e --arg out "$2" --arg in "$3" '
        any(.[]; .type == "PipeWire:Interface:Link"
            and (.info["output-port-id"] | tostring) == $out
            and (.info["input-port-id"] | tostring) == $in)' <<<"$1" >/dev/null
}

sink_links() { # dump sink
    jq -r --arg node "$2" '
        [.[] | select(.type == "PipeWire:Interface:Link"
            and (.info["input-node-id"] | tostring) == $node)] | length' <<<"$1"
}

link() { # dump output-port input-port what
    if link_exists "$1" "$2" "$3"; then
        echo "$4: already linked"
    else
        pw-link "$2" "$3" || die "could not link $4"
        echo "$4: linked"
    fi
}

start() {
    local dump input output source sink
    dump=$(pw-dump)
    input=$(node_id "$dump" media.name "$INPUT_STREAM" Stream/Input/Audio)
    output=$(node_id "$dump" media.name "$OUTPUT_STREAM" Stream/Output/Audio)
    [[ -n $input && -n $output ]] ||
        die "the app's audio streams are not there; start the preview with sound first"
    source=$(feeding_node "$dump" "$input")
    [[ -n $source ]] || die "the app's input stream is not linked to a device"

    sink=$(node_id "$dump" node.name "$SINK")
    if [[ -z $sink ]]; then
        pw-cli create-node adapter "{ factory.name=support.null-audio-sink
            node.name=$SINK node.description=\"CapturaX measure\" media.class=Audio/Sink
            audio.position=[FL FR] audio.rate=48000 monitor.channel-volumes=false
            object.linger=true }" >/dev/null || die "could not create the null sink"
        for _ in $(seq 20); do
            dump=$(pw-dump)
            sink=$(node_id "$dump" node.name "$SINK")
            [[ -n $sink && -n $(port_id "$dump" "$sink" input FR) ]] && break
            sleep 0.1
        done
        [[ -n $sink ]] || die "the null sink did not appear"
        echo "sink $SINK: created"
    else
        echo "sink $SINK: already there"
    fi

    local source_port output_port sink_left sink_right
    source_port=$(port_id "$dump" "$source" output FL)
    output_port=$(port_id "$dump" "$output" output FL)
    sink_left=$(port_id "$dump" "$sink" input FL)
    sink_right=$(port_id "$dump" "$sink" input FR)
    [[ -n $source_port && -n $output_port && -n $sink_left && -n $sink_right ]] ||
        die "a port is missing (card $source_port, app $output_port, sink $sink_left/$sink_right)"

    echo "left:  $(node_label "$dump" "$source")"
    link "$dump" "$source_port" "$sink_left" "card -> sink left"
    echo "right: $(node_label "$dump" "$output")"
    link "$dump" "$output_port" "$sink_right" "app -> sink right"
    echo "Now: $0 record <file> <seconds>"
}

record() { # file seconds
    [[ $# -eq 2 && $2 =~ ^[0-9]+$ && $2 -gt 0 ]] || usage 2
    local dump sink
    dump=$(pw-dump)
    sink=$(node_id "$dump" node.name "$SINK")
    [[ -n $sink ]] || die "no sink $SINK; run start first"
    (($(sink_links "$dump" "$sink") >= 2)) ||
        die "the sink lost its links (did the app restart?); run start again"
    mkdir -p "$(dirname "$1")"
    echo "recording $2 s into $1 (left: card, right: app)"
    local status=0
    timeout -s INT "$2" pw-record --target "$SINK" -P '{ stream.capture.sink=true }' \
        --rate 48000 --channels 2 --channel-map FL,FR "$1" || status=$?
    # timeout ends the recording with SIGINT and reports 124: that is the normal end.
    ((status == 0 || status == 124)) || die "pw-record failed ($status)"
    echo "done: $1"
}

stop() {
    local sink
    sink=$(node_id "$(pw-dump)" node.name "$SINK")
    if [[ -z $sink ]]; then
        echo "no sink $SINK"
        return
    fi
    pw-cli destroy "$sink" >/dev/null || die "could not remove the sink"
    echo "sink $SINK: removed"
}

case "${1:-}" in
    start) start ;;
    record) shift; record "$@" ;;
    stop) stop ;;
    -h | --help | help) usage ;;
    *) usage 2 ;;
esac
