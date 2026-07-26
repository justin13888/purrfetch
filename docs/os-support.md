# Operating-system support

This document covers runtime detection and the logos compiled into `purr`. It
does **not** indicate that a distribution-specific installation channel is
published or release-tested. See the [installation matrix](../README.md#installation)
for supported user installation paths and the
[packaging policy](../packaging/README.md) for maintainer status.

purr targets the operating systems and distributions that are **mainstream or
still actively maintained**. This is a deliberate subset of [neofetch's
operating-system list][ns]: neofetch carried explicit detection/art for 150+
systems, many long discontinued. Carrying obscure or abandoned distros adds
maintenance and binary weight for vanishing real-world use, so purr prunes them.

Reference: neofetch [`ccd5d9f`][nf] · [OS-support wiki][ns] · assessed 2026-06-14.

[nf]: https://github.com/dylanaraps/neofetch/blob/ccd5d9f52609bbdcd5d8fa78c4fdb0f12954125f/neofetch
[ns]: https://github.com/dylanaraps/neofetch/wiki/Operating-System-Support

## Shipped logos (50)

purr ships a `${c1}`..`${c6}` logo (neofetch format) for each of:

**Arch family** — arch, archcraft, arcolinux, artix, blackarch, endeavouros,
garuda, manjaro, parrot

**Debian / Ubuntu family** — debian, devuan, deepin, elementary, kali, kubuntu,
lubuntu, mint, mx, popos, raspbian, tails, ubuntu, ubuntu-budgie, ubuntu-mate,
xubuntu, zorin

**Fedora / RHEL family** — alma, centos, fedora, mageia, oracle, redhat, rocky*

**openSUSE** — opensuse-leap, opensuse-tumbleweed

**Independent** — alpine, gentoo, guix, nixos, peppermint, qubes, slackware,
solus, steamos, void

**BSD** — freebsd, netbsd, openbsd

**Other platforms** — macos, windows, plus a generic `linux` fallback

> *`rocky` is covered by the Fedora/RHEL palette; detection falls back to the
> generic logo if no dedicated art is matched.

Detection is substring-based against the running distribution name, longest
match first (so e.g. `ubuntu-mate` wins over `ubuntu`). Unknown distros fall
back to the platform-generic logo (`linux` / `macos` / `windows`).

Adding a logo is a drop-in: place a `name.txt` file in `ascii/distros/` using
neofetch's `${c1}`..`${c6}` markers, optionally with a leading
`# set_colors N N ...` palette line. No code change required.

## Pruned from neofetch (not shipped)

These were dropped from neofetch's list. They can still be requested via
`--ascii_distro <name>` (falling back to the generic logo) and a dedicated logo
can be contributed at any time.

**Discontinued / abandoned distros** — Antergos, Apricity OS, Chakra, ChaletOS,
Chapeau, DracOS, Kogaion, Bitrig, Blag, Gnewsense, GrombyangOS, Lunar Linux,
MagpieOS, Nurunner, OBRevenge, SwagArch, SalentOS, DesaOS, Trisquel, Redstar OS
(propaganda artifact), Container Linux / CoreOS (archived).

**Legacy / niche Unix** — AIX, IRIX, MINIX, Solaris-era (Oracle Solaris,
OpenIndiana, Joyent SmartOS), FreeMiNT, GNU Hurd, Haiku. *(Supportable in
principle but unverified; out of scope for purr's Linux/macOS/Windows focus.)*

**Mobile / embedded** — Android, iOS, postmarketOS, SailfishOS, LineageOS,
OpenWRT/LEDE. *(libmacchina can read some of these; purr does not ship logos or
claim support.)*

**Long tail** — the remaining ~80 rare distros in neofetch's list (AOSC OS,
AryaLinux, Calculate, Exherbo, Frugalware, Funtoo, GalliumOS, GoboLinux,
Hyperbola, KaOS, Korora, NuTyX, Parabola, Pardus, PCLinuxOS, Porteus, Puppy,
Rosa, Sabayon, Scientific, Siduction, SliTaz, Source Mage, Sparky, … ) are not
shipped. Most are inactive or have very small user bases.

See [`neofetch-parity.md`](neofetch-parity.md) for the full feature-parity
status, including detection breadth (package managers, etc.).
