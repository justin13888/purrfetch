# Third-party notices

purr (purrfetch) is licensed under the MIT License — see [LICENSE](LICENSE).

This product includes software and assets derived from third-party projects,
reproduced here in accordance with their licenses.

## libmacchina (Macchina)

purr links against [libmacchina](https://github.com/Macchina-CLI/libmacchina),
the system-readout library behind [macchina](https://github.com/Macchina-CLI/macchina),
which powers most of purr's probes (OS, kernel, uptime, packages, CPU, memory,
battery, and more). Rather than forking, purr contributes probe performance
improvements upstream to libmacchina so both projects benefit.

libmacchina is distributed under the MIT License; see its
[LICENSE](https://github.com/Macchina-CLI/libmacchina/blob/main/LICENSE.md).

## neofetch

purr aims for feature-parity with [neofetch](https://github.com/dylanaraps/neofetch)
and incorporates material derived from it, including:

- **ASCII distro logos** (`ascii/distros/*.txt`) — ported from neofetch's logos,
  retaining their `${c1}`..`${c6}` color markers.
- **Behavioral logic ported for parity** — e.g. default `set_colors` palettes
  (`build.rs`), CPU model-string cleanup, OEM-placeholder cleanup, the
  `gnome-shell`→`Mutter` window-manager rename, and the `${c1}`..`${c6}`
  renderer format.

neofetch is distributed under the MIT License, reproduced below:

```
MIT License

Copyright (c) 2015-2021 Dylan Araps

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```
