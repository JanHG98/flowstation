# Phase 4 – StayAlive MCCH reliability fix

## Symptom

Network-originated audio calls were prepared and transmitted successfully, but a StayAlive
AIv2/common-SCCH terminal sometimes did not join the group call until it performed a
RoamingLocationUpdating procedure. Existing recordings appeared more reliable because they were
often started shortly after such a refresh, while TTS synthesis shifted the actual call start
later into the terminal's idle control-channel phase.

## Root cause

MM advertised `scch_information_and_distribution_on_18th_frame = 1` whenever the terminal
reported `clch_needed` or `common_scch`, even when the granted energy-saving mode was
`StayAlive`. The terminal could therefore leave the ordinary MCCH and rely on the frame-18 SCCH
path although it was not in Energy Economy. The running stack's frame-18 paging compatibility
path was not reliable enough for this terminal combination.

## Change

* A terminal granted `StayAlive` is no longer assigned frame-18 common-SCCH distribution.
* It remains on the ordinary MCCH continuously.
* Real Energy-Economy grants (`Eg1` through `Eg3`) keep the existing frame-18 SCCH assignment.
* TTS and recordings already share the same fully materialized WAV/AudioPlayer path; no audio
  conversion or TTS-specific dispatch behavior is changed.

## Expected log

For ISSI 5102 in StayAlive mode:

```text
MM: ISSI 5102 is StayAlive; keeping it on the ordinary MCCH instead of assigning frame-18 common-SCCH
DLocationUpdateAccept ... energy_saving_information: ... StayAlive ... scch_information_and_distribution_on_18th_frame: None
```

For a real Energy-Economy terminal, the existing `Some(1)` SCCH assignment remains.
