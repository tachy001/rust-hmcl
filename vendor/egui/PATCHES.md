# Rust HMCL egui patch

Upstream: egui 0.31.1, https://github.com/emilk/egui/tree/0.31.1/crates/egui.
Source copied from the crates.io 0.31.1 package already used by Cargo.lock.
The upstream MIT OR Apache-2.0 licensing is retained; license texts are copied
from the corresponding upstream tag.

Changes to `src/ui.rs`:

- Add `disable_interaction()` to block pointer and keyboard responses without
  changing the painter's colors. The input gate propagates to children.
- Track `is_visually_enabled()` separately, so custom widgets can preserve
  their enabled appearance during a transition. Explicit `disable()` and
  disabled/invisible children still preserve the normal disabled appearance.

The normalized Cargo manifest uses local license paths. Other upstream Rust
sources are unchanged. Upstream compiler warnings may become visible for a
path dependency; they were suppressed for the registry dependency.

When upgrading egui, reapply these changes and run the launcher's motion tests,
including keyboard focus, pointer clicks, exact 400 ms completion, nested
disabled controls, and retained snapshot textures. Do not replace input gating
with a tooltip layer: egui 0.31.1 allows interaction on that layer.
