# Blip — Brand & Logo Guidelines

## 1. Concept: The Living Island
Blip's visual identity is built directly on its core product experience: a dynamic island notch docked at the top of the desktop screen, brought to life through friendly, animated mascot eyes that listen, react, and assist.

- **The Capsule**: Represents the physical software form factor — sleek, unobtrusive, rounded.
- **The Twin Eyes**: Expressive pill-shaped capsules that give the assistant warmth, responsiveness, and personality.
- **The Cyan Glow**: Signifies active attention, voice listening, and intelligent response.

---

## 2. Official Color Palette

| Name | Role | HEX | RGB | CMYK |
|---|---|---|---|---|
| **Electric Cyan** | Active Mascot Eye, Pulse, Accent | `#4cc9f0` | `76, 201, 240` | `68, 16, 0, 6` |
| **Cyan Glow (High)** | Eye Highlight / Gradient Top | `#70d6ff` | `112, 214, 255` | `56, 16, 0, 0` |
| **Deep Cyan** | Eye Gradient Base | `#00b4d8` | `0, 180, 216` | `100, 17, 0, 15` |
| **Obsidian Dark** | Notch Body / Tile Base | `#0d0f15` | `13, 15, 21` | `38, 29, 0, 92` |
| **Graphite Shell** | Island Subtle Border | `#1e222e` | `30, 34, 46` | `35, 26, 0, 82` |
| **Pure White** | Reversed Mark, Text | `#ffffff` | `255, 255, 255` | `0, 0, 0, 0` |

---

## 3. Logo Files & Variants

### Vector Masters (`assets/branding/`)
- `blip-symbol.svg` — Pure black monochrome master (100% vector, clean `fill-rule="evenodd"` cutouts).
- `blip-symbol-small.svg` — Small-size cut (optimized optical proportions for 16–32 px).
- `blip-symbol-color.svg` — Full-color vector mark with signature cyan glow.
- `blip-lockup-horizontal.svg` — Full horizontal lockup (Living Island symbol + custom geometric 'blip' wordmark).
- `app-icon.svg` — The production squircle Windows/Desktop app icon with ambient radial glow.

### Production App Icons (`src-tauri/icons/` & `assets/branding/variants/`)
- `icon.ico` — Multi-resolution Windows application icon (16, 24, 32, 48, 64, 128, 256 px).
- `icon.png` — High-resolution 512×512 PNG master.
- `32x32.png`, `64x64.png`, `128x128.png`, `128x128@2x.png` — Runtime assets for Tauri / Windows Explorer.

---

## 4. Usage Rules
- **Clear Space**: Maintain minimum breathing room equal to half the height of the island capsule around the mark.
- **Minimum Size**: Do not render the standard symbol below 16 px. For 16–32 px (favicons, tray), use `blip-symbol-small.svg`.
- **Misuse**:
  - Do NOT rotate or tilt the capsule.
  - Do NOT stretch or alter the eye capsule proportions.
  - Do NOT replace the eyes with standard circular dots.
