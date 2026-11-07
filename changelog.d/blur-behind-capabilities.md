---
section: fixed
---
- **`WindowCapabilities::blur_behind` is false where `ext-background-effect-v1` can't blur** (zgx-desktop): a compositor with the protocol's manager but without its `blur` capability (blur turned off) applies none, and the repository's winit asks through that manager alone where there is one; zgx now binds the manager on its own connection and reads its `capabilities`, where it took the manager's presence for blur.
