# appDataFolder Browser (Web / 100% Rust)

A browser-based, 100%-Rust (compiled to WebAssembly via Dioxus) tool for
viewing, downloading, and deleting revisions of files in Google Drive's
hidden `appDataFolder` — built for recovering `people_modeler_backup.json`
from PeopleModeler.

**No server, no secrets in the repo.** Sign-in uses Google Identity Services'
browser-native OAuth flow: only a public **Client ID** is needed (never a
client secret), and the access token lives only in your browser tab's
memory for the session.

> ⚠️ Build note: this was written and verified by compiling natively
> (`cargo check`, `cargo test`, `cargo clippy`, `cargo fmt --check` — all
> passing) with Rust 1.91, since the sandbox used to write this didn't have
> the `wasm32-unknown-unknown` target available (no access to rustup's
> download servers). The 11 unit tests genuinely run and pass.
>
> Three real CI failures were fixed by actually running this on GitHub
> (things this sandbox couldn't test itself):
> 1. `Dioxus.toml`'s `out_dir` setting turned out to be unreliable, so
>    `docs/index.html` never existed. Fixed by passing the CLI flag
>    explicitly to `dx bundle` instead.
> 2. That first fix used the wrong flag name (`--outdir`, based on an old
>    GitHub issue title rather than the CLI's actual `--help`/error output)
>    — the real flag is **`--out-dir`**. The CLI's own error message named
>    the correct flag directly, which is what fixed it.
> 3. Switching to `cargo-binstall` for a faster `dioxus-cli` install
>    resulted in `dx: command not found` — the prebuilt-binary path didn't
>    actually put a working `dx` on `PATH`. Rather than trying to guess why
>    (another unverifiable assumption), the `build` job now verifies
>    `command -v dx` after the binstall attempt and falls back to
>    `cargo install dioxus-cli` (compiling from source) if it's missing —
>    slower in that case, but that path is proven to work.
>
> There's also a defensive fallback step in the `build` job that searches
> `target/dx/` for the real output if it's ever not where expected, so a
> future flag/version change fails with a clear message instead of a
> confusing "file not found."

## Continuous Integration & Deployment

This repo ships with a single linear pipeline: **`.github/workflows/pipeline.yml`**

```
unit tests & lint  →  build wasm bundle  →  e2e smoke tests  →  deploy to Pages
```

- **`unit-tests`**: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`
- **`build`**: installs the wasm32 target, installs `dioxus-cli` via
  `cargo-binstall` (prebuilt binary — compiling it from source takes 10+
  minutes due to its dependency tree), runs
  `dx bundle --platform web --release --out-dir docs` **once**, and uploads
  that same build both as a plain artifact (for the `e2e` job) and as a
  Pages artifact (for the `deploy` job) — no duplicate wasm builds.
- **`e2e`**: downloads the build, nests it under its production
  `/repo-name/` path locally (so the smoke tests hit the exact asset paths
  that will be live on Pages), and runs Playwright against it headlessly.
- **`deploy`**: only runs on pushes to `main` (PRs stop after `e2e`).
  Reuses the artifact already uploaded by `build` — no rebuild — and
  publishes via `actions/deploy-pages`.

To enable deployment: **Settings → Pages → Source → GitHub Actions**.

### About the e2e tests

The Playwright suite (`e2e/`) is a **smoke test**, not a full functional
test — it verifies the WASM app boots, the sign-in form renders, client-side
validation works, and the Client ID persists via `localStorage`. It
deliberately does **not** exercise real Google sign-in, since that requires
a live Google account and interactive consent — not something safe or
practical to automate in public CI. Testing actual Drive API behavior
(listing/downloading/deleting revisions) needs to be done manually against
your own account.

### Running tests locally

```bash
# Unit tests
cargo test

# Formatting & lint (same checks CI runs)
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings

# E2E (after building the app)
dx bundle --platform web --release --out-dir docs
cd e2e
npm install
npx playwright install --with-deps chromium
npm test
```

## Quick sign-in presets (optional)

If you personally maintain multiple projects (e.g. PeopleModeler,
AsthmaTrack), you can add one-click "quick sign-in" avatar buttons for each,
so you don't have to paste a Client ID in manually every time. This is
purely a personal convenience layered on top of the manual input — it
never replaces it, and it degrades gracefully to nothing if unconfigured.

**Important:** `appDataFolder` access is scoped per OAuth app/project —
there's no single "admin" Client ID that can see everyone's data. Each
preset is tied to one specific project's own Client ID and only ever
unlocks that project's own backup data. This is not a way to grant anyone
broader access; it's just a shortcut for switching between your own
several projects.

### How it works

- Each preset's Client ID is baked in at **build time** from its own
  individually-named GitHub Actions secret (e.g.
  `GOOGLE_WEB_CLIENT_ID_PEOPLEMODELER`). OAuth Client IDs aren't secret —
  only client *secrets* are — so this is about avoiding hardcoding values
  in source, not about hiding them; the value ends up plainly visible in
  the compiled WASM either way, which is expected.
- If a secret isn't set (e.g. local builds, forked PRs — GitHub withholds
  repo secrets from fork-triggered workflow runs), that preset's button
  simply doesn't appear. The manual Client ID field always works
  regardless.

### Adding a new preset project

1. **Settings → Secrets and variables → Actions → New repository secret**,
   named `GOOGLE_WEB_CLIENT_ID_<YOURPROJECT>`, value = that project's Web
   application OAuth Client ID.
2. In `src/main.rs`, add a line to `KNOWN_PROJECTS`:
   ```rust
   const KNOWN_PROJECTS: &[(&str, Option<&str>)] = &[
       ("PeopleModeler", option_env!("GOOGLE_WEB_CLIENT_ID_PEOPLEMODELER")),
       ("AsthmaTrack", option_env!("GOOGLE_WEB_CLIENT_ID_ASTHMATRACK")),
       ("YourProject", option_env!("GOOGLE_WEB_CLIENT_ID_YOURPROJECT")),
   ];
   ```
3. In `.github/workflows/pipeline.yml`, add the matching env var to the
   `build` job's "Build web bundle (release)" step:
   ```yaml
   GOOGLE_WEB_CLIENT_ID_YOURPROJECT: ${{ secrets.GOOGLE_WEB_CLIENT_ID_YOURPROJECT }}
   ```

### Security notes

Storing a Client ID as a GitHub secret is a hygiene choice (keeps it out of
tracked source, masked in CI logs), **not** a confidentiality boundary —
OAuth Client IDs are public identifiers by design and end up plainly
visible in the compiled WASM/page source regardless of how they're built.
That's expected and normal for this type of app; it's the same as any
client-side OAuth integration.

The actual access control that matters lives in Google Cloud Console, on
each OAuth Client ID itself:

- **Authorized JavaScript origins** — this is the real boundary. Google's
  servers check the request's `Origin` header server-side before issuing a
  token, so this list is what actually stops someone else from using your
  Client ID from a different site. Check it whenever you add a preset or
  change domains: it should contain only your real GitHub Pages URL(s) and
  `http://127.0.0.1:8080` for local dev — nothing stale or wildcarded.
- **OAuth consent screen status** (Testing vs. Published) and its **Test
  users** list — while in Testing, only allowlisted Google accounts can
  complete sign-in at all, independent of who can see the Client ID.
- **Scope** — this app only ever requests `drive.appdata`, which can't
  touch a user's general Drive files, only the requesting app's own hidden
  folder. Don't broaden this without a reason.
- There is **no client secret** anywhere in this architecture (the Web
  application + Google Identity Services token-client flow is a
  public-client model by design) — so there's nothing of that kind to leak
  in the first place.
- Access tokens are short-lived (~1 hour), held only in browser memory for
  the session, and never written to `localStorage` — only the Client ID
  (non-secret) is persisted there.

## Pre-commit hooks

This repo uses [pre-commit](https://pre-commit.com) to catch issues before
they're committed: secret detection, general hygiene checks, and the same
`cargo fmt`/`cargo clippy` checks CI runs.

Setup (one-time, per clone):

```bash
pip install pre-commit    # or: pipx install pre-commit / brew install pre-commit
pre-commit install
```

After that, hooks run automatically on `git commit`. To run them manually
against everything (e.g. right after cloning):

```bash
pre-commit run --all-files
```

Hooks configured in `.pre-commit-config.yaml`:
- `trailing-whitespace`, `end-of-file-fixer`, `check-merge-conflict`,
  `check-added-large-files`, `check-case-conflict`, `mixed-line-ending`
- **`detect-secrets`** — scans staged changes for anything that looks like
  an API key, token, or credential, checked against `.secrets.baseline`
  (the known-clean baseline committed in this repo)
- **`cargo fmt`** / **`cargo clippy`** — same checks as CI's `unit-tests` job

If `detect-secrets` ever flags a genuine new secret you need to allow (e.g.
a placeholder that looks real), regenerate the baseline after removing the
actual secret from the code:

```bash
detect-secrets scan > .secrets.baseline
```

## 1. One-time Google Cloud setup

1. https://console.cloud.google.com/ → create/select a project.
2. **APIs & Services → Library** → enable **Google Drive API**.
3. **APIs & Services → OAuth consent screen** → add your Google account
   under **Test users** (required for the `drive.appdata` scope on an
   unverified app).
4. **APIs & Services → Credentials → Create Credentials → OAuth client ID**.
   - Application type: **Web application** (not Desktop — this flow is
     pure client-side JS, so it authenticates by *origin*, not redirect URI).
   - Under **Authorized JavaScript origins**, add the URL you'll host this
     on, e.g. `https://yourusername.github.io`.
   - You do **not** need to set an authorized redirect URI for this flow.
5. Copy the **Client ID** (looks like `xxxxx.apps.googleusercontent.com`).
   You'll paste this into the running app itself — nothing goes in a file.

## 2. Local setup

You need:
- Rust + the `wasm32-unknown-unknown` target:
  ```bash
  rustup target add wasm32-unknown-unknown
  ```
- The Dioxus CLI, pinned to the version this repo is tested against
  (0.6.3). Recommended via `cargo-binstall` (fetches a prebuilt binary —
  compiling `dioxus-cli` from source takes 10+ minutes):
  ```bash
  cargo install cargo-binstall   # one-time
  cargo binstall dioxus-cli --version 0.6.3 --locked
  ```
  Or, if you'd rather compile it yourself:
  ```bash
  cargo install dioxus-cli --version 0.6.3 --locked
  ```

Then, from this project folder:

```bash
dx serve --platform web
```

This opens a local dev server (typically `http://127.0.0.1:8080`). Paste
your Client ID into the page and click **Sign in with Google**.

(For local testing, also add `http://127.0.0.1:8080` as an authorized
JavaScript origin on the OAuth client, alongside your GitHub Pages URL.)

## 3. Deploy to GitHub Pages (automatic, via Actions)

Deployment is handled by the `deploy` job in `.github/workflows/pipeline.yml`
— you don't need to build or commit anything manually. Just:

1. Push this repo to GitHub.
2. **Settings → Pages → Source → GitHub Actions**.
3. Push to `main` (or run the workflow manually via **Actions → Deploy to
   GitHub Pages → Run workflow**).

The workflow automatically sets `base_path` in `Dioxus.toml` to your repo
name before building, so asset paths resolve correctly at
`https://yourusername.github.io/your-repo-name/`.

If you'd rather build and deploy by hand instead of using the workflow:

```bash
# Edit Dioxus.toml first: uncomment base_path and set it to your repo name
dx bundle --platform web --release --out-dir docs
cp docs/index.html docs/404.html
# then commit docs/ and set Settings → Pages → Source → Deploy from a branch → main /docs
```

## How it works

- `index.html` loads Google's `accounts.google.com/gsi/client` script and
  defines one small JS bridge function (`gisRequestTokenPromise`) that
  wraps the token flow in a `Promise`.
- `src/gis.rs` calls that bridge via `wasm-bindgen` and awaits the result
  in async Rust.
- `src/drive.rs` makes the actual Drive API v3 calls (list files, list
  revisions, download, delete) directly from the browser using `gloo-net`,
  with the access token as a Bearer header.
- `src/browser.rs` has small helpers for triggering a file download (via a
  base64 `data:` URL, no Blob API needed) and a native `confirm()` dialog
  before deletes.
- `src/main.rs` is the Dioxus UI: a Client ID input + sign-in button, then
  a card per file with a table of revisions and Download/Delete buttons.

## Notes & caveats

- **Deletion is permanent** — no trash for revisions. Download anything you
  want to keep first.
- The access token from this flow is short-lived (~1 hour) and there's no
  refresh token in this model — if it expires, just click "Sign in with
  Google" again.
- Revisions are typically retained by Google for a limited window (often
  ~30 days), so old ones may simply no longer exist.
- The Client ID is not secret and is fine to have visible in the page/URL —
  it only identifies which app is asking, it doesn't grant any access by
  itself.
