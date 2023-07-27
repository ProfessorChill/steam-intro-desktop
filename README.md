# Stream Intro

I needed a stream intro and having a waveform reader in OBS is oddly difficult
that said I decided to learn the libraries [iced](https://crates.io/crates/iced)
and [cpal](https://crates.io/crates/cpal) and create a waveform application.

## Issues

On Windows 11 it seems to sometimes not have a stream source config available
which just crashes the application. I may need to learn the win32 lib or find
some other lib that can record desktop audio.

On Linux it reads from the audio monitor instead of the desktop audio, to my
understanding I need to learn audio device drivers or something as cpal at least
to my research cannot read desktop audio.

All in all, I'm still learning how all this works and will develop this further
as I stream on Linux AND Windows.
