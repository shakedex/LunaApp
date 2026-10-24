#!/usr/bin/env bash
# Pulls three graded frames from each sample clip: 17, 50 and 83 % in.
# usage: frames.sh <clips dir> <out dir>
# The clips are Shaked's GH5 V-Log L takes of Luna (P1066462..P1066473);
# P1066472 is left out (the bathroom shot). The grade is a quick lift for the
# report, not a Rec.709 transform.
set -euo pipefail
src=$1
out=$2
mkdir -p "$out"
grade="scale=640:-2:flags=lanczos,format=rgb24,colorlevels=rimin=0.06:gimin=0.06:bimin=0.06:rimax=0.82:gimax=0.82:bimax=0.82,curves=all='0/0 0.25/0.18 0.6/0.66 1/1',eq=saturation=1.3:gamma=1.05"
for clip in P1066462 P1066463 P1066464 P1066465 P1066466 P1066467 P1066468 P1066469 P1066470 P1066471 P1066473; do
  file="$src/$clip.MOV"
  duration=$(ffprobe -v error -show_entries format=duration -of csv=p=0 "$file")
  i=0
  for at in 0.17 0.5 0.83; do
    t=$(awk "BEGIN { print $duration * $at }")
    ffmpeg -v error -y -ss "$t" -i "$file" -frames:v 1 -vf "$grade" -q:v 3 "$out/${clip}_$i.jpg"
    i=$((i + 1))
  done
done
echo "frames in $out"
