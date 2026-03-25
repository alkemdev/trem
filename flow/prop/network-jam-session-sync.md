# Network jam session sync — speculative

## What

Research note: clock sync between two trem instances over LAN (OSC timecode, custom protocol), or shared Rung clip merge — not realtime audio streaming.

## Why

Collaborative composition is a common ask; rational timing could be a selling point for sync semantics.

## Notes

- Audio streaming is out of scope initially; focus on shared sequence state + metronome lock.
- Conflict resolution for simultaneous edits is hard; pair with CRDT research or "leader follower" only.
