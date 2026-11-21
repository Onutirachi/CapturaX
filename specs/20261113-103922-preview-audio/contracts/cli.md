# Contract: Command Line (additions)

Adds to the [device preview's command line](../../20261106-170507-device-preview/contracts/cli.md);
nothing changes there except what is listed. Not shown in the interface.

| Option | Effect |
|---|---|
| `--list-devices` | After the video devices, prints the audio inputs and outputs (name, id, link, format, `default` mark), then for each video device its link and the input `pair` chooses (or `none`), then exits 0. Used to check pairing (quickstart "Pairing") |
| `--no-audio` | Starts without the audio monitor and sessions, for comparing the video's measurements with and without sound (SC-007) |

`--test-video` plays no sound (it has no audio input).
