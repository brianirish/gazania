conf = configuration_data()
conf.set('VERSION', meson.project_version())

configure_file(
  input: app_id + '.desktop.in',
  output: app_id + '.desktop',
  configuration: conf,
  install: true,
  install_dir: get_option('datadir') / 'applications',
)

configure_file(
  input: app_id + '.metainfo.xml.in',
  output: app_id + '.metainfo.xml',
  configuration: conf,
  install: true,
  install_dir: get_option('datadir') / 'metainfo',
)

install_data(app_id + '.gschema.xml',
  install_dir: get_option('datadir') / 'glib-2.0' / 'schemas')

install_data('icons/hicolor/scalable/apps' / app_id + '.svg',
  install_dir: get_option('datadir') / 'icons' / 'hicolor' / 'scalable' / 'apps')

install_data('icons/hicolor/symbolic/apps' / app_id + '-symbolic.svg',
  install_dir: get_option('datadir') / 'icons' / 'hicolor' / 'symbolic' / 'apps')

gnome.post_install(
  glib_compile_schemas: true,
  gtk_update_icon_cache: true,
  update_desktop_database: true,
)
