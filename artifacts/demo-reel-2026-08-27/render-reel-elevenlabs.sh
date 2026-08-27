#!/bin/zsh
set -euo pipefail

reel_dir="${0:A:h}"
work_dir="$reel_dir/render-work-elevenlabs"
audio_dir="$reel_dir/voice/elevenlabs-eric"
output_file="$reel_dir/planogrammy-brand-blocking-performance-reel-elevenlabs-eric.mp4"
mkdir -p "$work_dir/segments"

voice_files=(01-title 02-fields 03-prompt1 04-proposal1 05-approve1 06-prompt2 07-proposal2 08-capacity 09-dense 10-final)
frame_files=(01-title 02-fields 03-prompt1 04-proposal1 05-approve1 06-prompt2 07-proposal2 08-capacity 09-dense-approved 10-final)

for index in {1..10}; do
  voice_file="${voice_files[$index]}"
  frame_file="${frame_files[$index]}"
  audio_file="$audio_dir/$voice_file.mp3"
  segment_file="$work_dir/segments/$(printf '%02d' "$index").mp4"
  audio_duration="$(ffprobe -v error -show_entries format=duration -of default=nw=1:nk=1 "$audio_file")"
  segment_duration="$(awk -v duration="$audio_duration" 'BEGIN { printf "%.3f", duration + 0.35 }')"
  fade_out="$(awk -v duration="$segment_duration" 'BEGIN { printf "%.3f", duration - 0.22 }')"

  ffmpeg -hide_banner -loglevel error -y \
    -loop 1 -framerate 30 -i "$reel_dir/frames/$frame_file.png" \
    -i "$audio_file" \
    -vf "scale=1280:720,fade=t=in:st=0:d=0.22,fade=t=out:st=$fade_out:d=0.22,format=yuv420p" \
    -af "apad=pad_dur=0.35" -t "$segment_duration" \
    -c:v libx264 -preset medium -crf 18 -r 30 -c:a aac -b:a 160k "$segment_file"
done

concat_file="$work_dir/segments.ffconcat"
: > "$concat_file"
for segment in "$work_dir"/segments/*.mp4; do
  printf "file '%s'\n" "$segment" >> "$concat_file"
done

ffmpeg -hide_banner -loglevel error -y -f concat -safe 0 -i "$concat_file" -c copy \
  -metadata title="Planogrammy — Brand Blocking and Performance Optimization" \
  -metadata artist="ElevenLabs — Eric - Smooth, Trustworthy" \
  -metadata comment="Local deterministic demo using synthetic representative performance data" \
  -movflags +faststart "$output_file"

ffmpeg -hide_banner -loglevel error -y -i "$output_file" \
  -vf "select='eq(n,60)+eq(n,480)+eq(n,900)+eq(n,1320)+eq(n,1740)+eq(n,2160)',scale=426:240,tile=3x2" \
  -frames:v 1 "$reel_dir/contact-sheet-elevenlabs-eric.jpg"
