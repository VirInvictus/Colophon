# Meson build wrapper for colophon.
#
# A thin orchestration layer around `cargo build --release`. The Rust
# workspace at `colophon/` and `colophon-core/` is the source of truth;
# meson only handles the GNOME-shaped install layout (binary into $bindir,
# gschema into $datadir/glib-2.0/schemas, desktop / metainfo / icon into
# their canonical subdirs) so the project ships cleanly through Flatpak and
# any distro that expects a meson buildsystem.
#
# Use:
#   meson setup build --prefix=/usr
#   meson compile -C build
#   meson install -C build
#
# The Flatpak manifest at `org.virinvictus.Colophon.json` invokes meson the
# same way against `/app`.

project(
  'colophon',
  'rust',
  version: '0.10.0',
  meson_version: '>= 0.59.0',
  license: 'MIT',
)

application_id = 'org.virinvictus.Colophon'
prefix = get_option('prefix')
bindir = prefix / get_option('bindir')
datadir = prefix / get_option('datadir')

cargo = find_program('cargo')
glib_compile_schemas = find_program('glib-compile-schemas')

# Cargo build target. Build always runs (cargo handles its own incremental
# logic) so meson stays out of Rust's dependency tracking.
cargo_options = ['--manifest-path', meson.project_source_root() / 'Cargo.toml']
cargo_options += ['--release']
cargo_options += ['--package', 'colophon']

cargo_build = custom_target(
  'cargo-build',
  build_by_default: true,
  build_always_stale: true,
  output: 'colophon',
  console: true,
  install: true,
  install_dir: bindir,
  command: [
    'env', 'CARGO_TARGET_DIR=' + meson.project_build_root() / 'cargo-target',
    cargo, 'build', cargo_options,
    '&&', 'cp', meson.project_build_root() / 'cargo-target' / 'release' / 'colophon', '@OUTPUT@',
  ],
)

# GSettings schema. install_data drops the .xml into the canonical schema
# directory; the post-install script compiles the schema cache so end users
# don't need a manual step.
install_data(
  'data/' + application_id + '.gschema.xml',
  install_dir: datadir / 'glib-2.0' / 'schemas',
)

meson.add_install_script(
  glib_compile_schemas,
  datadir / 'glib-2.0' / 'schemas',
)

# Desktop entry + AppStream metainfo. Desktop file into $datadir/applications,
# metainfo into $datadir/metainfo (the AppStream-spec location, replacing the
# older $datadir/appdata path the file is named for historically).
install_data(
  'data/' + application_id + '.desktop',
  install_dir: datadir / 'applications',
)

install_data(
  'data/' + application_id + '.appdata.xml',
  install_dir: datadir / 'metainfo',
  rename: application_id + '.metainfo.xml',
)

# Application icon. The scalable SVG covers every desktop-environment size;
# the symbolic SVG is for the monochrome contexts GTK draws (sidebars, menus).
# hicolor is the freedesktop fallback theme every real icon theme inherits
# from, so the icon is available regardless of the user's active theme.
install_data(
  'data/icons/hicolor/scalable/apps/' + application_id + '.svg',
  install_dir: datadir / 'icons' / 'hicolor' / 'scalable' / 'apps',
)
install_data(
  'data/icons/hicolor/symbolic/apps/' + application_id + '-symbolic.svg',
  install_dir: datadir / 'icons' / 'hicolor' / 'symbolic' / 'apps',
)

summary({
  'prefix': prefix,
  'bindir': bindir,
  'datadir': datadir,
  'application id': application_id,
}, section: 'Configuration')
