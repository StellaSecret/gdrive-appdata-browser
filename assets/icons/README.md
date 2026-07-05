# Icon assets

- **`peoplemodeler.svg`** — provided directly as a ready-to-use SVG.
- **`asthmatrack.png`** — the source design was an HTML+Canvas generator
  (`scripts/asthmatrack-icon-generator.html`) meant to be opened in a
  browser and downloaded by hand. Since no headless browser or
  `node-canvas` was available in the environment used to build this
  feature, `scripts/render_asthmatrack_icon.py` re-implements the same
  drawing calls in Pillow/numpy and was used to generate this PNG
  directly, without a browser. Re-run it if you want to tweak the design:

  ```bash
  pip install pillow numpy
  python3 scripts/render_asthmatrack_icon.py
  # writes asthmatrack-512.png in the working directory - copy it here
  ```

Both are referenced in `src/main.rs` via Dioxus's `asset!()` macro and
bundled automatically by `dx bundle`.
