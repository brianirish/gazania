project('zinnia',
  version: '0.1.0',
  meson_version: '>= 1.0.0',
  license: 'MIT',
)

gnome = import('gnome')
cargo = find_program('cargo', required: true)
find_program('blueprint-compiler', required: true)

app_id = 'io.github.brianirish.Zinnia'
cargo_target_dir = meson.project_build_root() / 'cargo-target'
cargo_profile = get_option('buildtype') == 'debug' ? 'debug' : 'release'

custom_target('cargo-build',
  build_by_default: true,
  build_always_stale: true,
  output: ['zinnia', 'zinnia-app'],
  console: true,
  install: true,
  install_dir: get_option('bindir'),
  command: [
    meson.project_source_root() / 'build-aux/cargo.sh',
    meson.project_source_root(),
    cargo_target_dir,
    cargo_profile,
    '@OUTDIR@',
  ],
)

subdir('data')
