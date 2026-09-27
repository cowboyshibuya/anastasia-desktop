# Third-party notices

Anastasia Desktop bundles or derives from the components below. The desktop
source is GPL-3.0-only; see [LICENSE](LICENSE).

## Waku

<https://github.com/egoist/waku> — GPL-3.0-only, © EGOIST.

This repository retains its multiline composer and supporting interface code
from the former Waku-derived Anastasia prototype. The old daemon and provider
implementations are not part of this desktop client.

## Solar Icons

<https://www.figma.com/community/file/1166831539721848736> — CC BY 4.0, ©
480 Design.

Interface glyphs in `assets/icons/` are from the Solar icon set, used under
the Creative Commons Attribution 4.0 International license.

## Geist and Geist Mono

<https://github.com/vercel/geist-font> — SIL Open Font License 1.1, © The Geist Project Authors.

Bundled in `assets/fonts/`. Geist is the interface typeface; Geist Mono is used
for code and shortcut chips. Full license text: `assets/fonts/OFL-geist.txt`.

## JetBrains Mono

<https://github.com/JetBrains/JetBrainsMono> — SIL Open Font License 1.1, ©
The JetBrains Mono Project Authors.

Bundled in `assets/fonts/`, used by the terminal emulator. Full license text:
`assets/fonts/OFL.txt`.

## Symbols Nerd Font

<https://github.com/ryanoasis/nerd-fonts> — MIT, © Ryan L McIntyre.

Bundled in `assets/fonts/` for fallback symbols. Full
license text: `assets/fonts/LICENSE-nerd-fonts.txt`.

## GPUI

<https://github.com/zed-industries/zed> — Apache-2.0, © Zed Industries.

Anastasia builds against the `egoist/zed` fork of GPUI pinned in `Cargo.lock`.

## Rust dependencies

Every crate in `Cargo.lock` carries its own license. Generate the full
inventory with `cargo license` or `cargo about`.
