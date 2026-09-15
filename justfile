# Colophon's documented incantations, in runnable form.
# The README and CLAUDE.md describe these; this file executes them.

# default: list the recipes
default:
    @just --list

# the full local suite (119+ tests; the perf harness is #[ignore]d)
test:
    cargo test --workspace

# the CI gate set: fmt, clippy -D warnings, then the suite
check:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    just test

# reclaim build disk (house rule: no target/ outlives a session)
clean:
    cargo clean

# regenerate the vendored Flatpak crate sources after ANY dependency
# change (uv is the pinned runner; the file is linguist-generated)
regen-sources:
    uv run scripts/flatpak-cargo-generator.py Cargo.lock -o generated-sources.json

# verify the Flatpak manifest for real (needs flatpak-builder + the
# GNOME 50 runtime and the rust-stable 25.08 SDK extension)
flatpak-verify:
    flatpak-builder --force-clean --user flatpak-build org.virinvictus.Colophon.json
    flatpak-builder --user --run flatpak-build org.virinvictus.Colophon.json colophon --help
