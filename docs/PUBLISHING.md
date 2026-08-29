# Publishing KAGRA to PyPI

The package name on PyPI is **`kagra`** (`pip install kagra`). The import stays `import kagra`.

## One-time setup

1. Create the project on [pypi.org](https://pypi.org) (or let the first trusted-publish create it).
2. PyPI → Project → Publishing → **Trusted publishers**:
   - Owner: `EMMA019`
   - Repository: `KAGRA`
   - Workflow: `publish.yml`
   - Environment: `pypi`
3. In GitHub: Settings → Environments → create **`pypi`** (optional reviewers on the first release).

No API token is stored in the repo. The workflow uses OIDC (`id-token: write`).

## Cut a release

1. Bump `version` in `pyproject.toml` and `old/kagra-core/Cargo.toml` together.
2. Add a `CHANGELOG.md` section.
3. Tag and push:

```bash
git tag v0.1.0
git push origin v0.1.0
```

`Publish` builds sdist + wheels (Linux manylinux_2_28 × 3.10–3.12, plus Windows × 3.10–3.12) and uploads them. macOS wheels are omitted until they can be verified on a Mac. The sdist must contain `LICENSE` at the archive root (`license = { file = "LICENSE" }` in `pyproject.toml`); PyPI 400s otherwise.

If a tag already exists and nothing reached PyPI (the `v0.1.0` Linux jobs failed that way), delete the tag and recreate it after this workflow is on `master`:

```bash
git push origin :refs/tags/v0.1.0
git tag -d v0.1.0
git tag -a v0.1.0 origin/master -m "v0.1.0 — first PyPI release"
git push origin v0.1.0
```

Dry-run a wheel build without uploading: Actions → Publish → Run workflow → `dry_run` checked. (`workflow_dispatch` never publishes; only `v*` tags do.)

## After the first upload

```bash
pip install kagra
python -m kagra
```

From-source installs (contributors) still use `maturin develop`.
