# sdkm App Icons

Final transparent RGBA assets (outer white matte/halo removed). Only files used by the
build/runtime are kept.

## Files

- `02-app-icon-simplified.png` — 1024 × 1024 source (simplified mascot)
- `02-app-icon-simplified-128.png` — 128 × 128 sidebar icon (`assets.rs` embeds it as `app-icon.png`)
- `02-app-icon-simplified-256.png` — 256 × 256 window icon on non-Windows platforms
- `sdkm.ico` — multi-size Windows exe icon (16/24/32/48/64/128/256), embedded by `crates/sdkm-cli/build.rs` and `crates/sdkm-gui/build.rs`

## Regenerate derived files

From the 1024 × 1024 source:

```python
from PIL import Image

img = Image.open("02-app-icon-simplified.png").convert("RGBA")
img.resize((128, 128), Image.LANCZOS).save("02-app-icon-simplified-128.png", optimize=True)
img.resize((256, 256), Image.LANCZOS).save("02-app-icon-simplified-256.png", optimize=True)
img.save("sdkm.ico", sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
```

All PNGs are RGBA with transparent pixels outside the icon artwork.