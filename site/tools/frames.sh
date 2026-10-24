#!/usr/bin/env bash
# Three graded frames from each sample clip, at 17, 50 and 83 % in.
# usage: frames.sh <clips dir> <out dir>
#
# The clips are Shaked's GH5 takes of Luna the cat (P1066462..P1066473, V-Log L).
# P1066472 is left out. The grade is a quick lift to Rec.709-ish for the
# thumbnails, not a proper transform.
set -euo pipefail
src=$1
out=$2
mkdir -p "$out"
grade="scale=960:-2:flags=lanczos,format=rgb24,colorlevels=rimin=0.07:gimin=0.07:bimin=0.07:rimax=0.78:gimax=0.78:bimax=0.78,curves=all='0/0 0.2/0.17 0.55/0.68 1/1',eq=saturation=1.35:contrast=1.03"
for clip in P1066462 P1066463 P1066464 P1066465 P1066466 P1066467 P1066468 P1066469 P1066470 P1066471 P1066473; do
  file="$src/$clip.MOV"
  duration=$(ffprobe -v error -show_entries format=duration -of csv=p=0 "$file")
  i=0
  for at in 0.17 0.5 0.83; do
    t=$(awk "BEGIN { print $duration * $at }")
    ffmpeg -v error -y -ss "$t" -i "$file" -frames:v 1 -vf "$grade" -q:v 2 "$out/${clip}_$i.jpg"
    i=$((i + 1))
  done
done
echo "frames in $out"
