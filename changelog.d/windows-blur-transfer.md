---
section: added
---
- **`WindowCapabilities::blur_pass` and `blur_lift`** (zgx-desktop): how much of what is behind the window its system blur lets through, and what it adds. Windows' acrylic accent lets 83% through and adds 8 (its luminosity pass, which no accent state or tint undoes), so a scrim over plain glass read darker there than over macOS's material, by 8 under an 80% scrim over white; macOS and Linux report a pure blur. An app can solve a thinner scrim that comes out the same over anything.
