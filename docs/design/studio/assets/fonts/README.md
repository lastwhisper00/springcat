# Interface font

Noto Sans SC variable font, bundled with the design prototype so Chinese and Latin text use the same face without depending on OS fallback. The complete font is retained for arbitrary task titles and note input, including characters outside the sample copy.

- Asset: `NotoSansSC-VF.ttf` (17,773,244 bytes).
- Verified variable weight axis: 100–900, default 100.
- Source: the existing Noto Sans SC installation at `C:/Windows/Fonts/NotoSansSC-VF.ttf`.
- Upstream family: https://github.com/google/fonts/tree/main/ofl/notosanssc
- License: SIL Open Font License 1.1, included as `OFL.txt` from the upstream family directory.
- Loaded by `fonts.css` under the CSS alias `SpringCat Sans`, using `font-display: swap` and actual 400/500 weights for the main list.

The local prototype retains the full font for coverage; production delivery can use WOFF2 conversion and script-based partitioning. Standalone SVG/Figma import uses the installed Noto Sans SC family; the browser design gallery loads this bundled asset for its inline SVGs.
