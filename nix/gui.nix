# Системные библиотеки оболочек Tauri и окружение их запуска.
#
# Tauri на Linux линкуется с системным WebKitGTK, а не тащит свой движок. Без
# этих пакетов `cargo tauri build` падает на этапе линковки, не найдя
# `webkit2gtk-4.1` через pkg-config (001 T111).
{ pkgs }:
{
  # Библиотеки, которые ищутся и при сборке (pkg-config), и при запуске.
  libs = [
    pkgs.webkitgtk_4_1
    pkgs.libsoup_3
    pkgs.gtk3
    pkgs.glib
    pkgs.cairo
    pkgs.pango
    pkgs.atk
    pkgs.gdk-pixbuf
    pkgs.librsvg
    pkgs.openssl
    # Tauri/GTK обращаются к шине сообщений (портал выбора файлов,
    # уведомления). Без libdbus собранная оболочка не находит libdbus-1.so.3
    # во время работы — сборка при этом проходит, и отсутствие проявилось бы
    # только при запуске.
    pkgs.dbus
  ];

  # Каталоги со схемами GSettings, пригодные для XDG_DATA_DIRS.
  #
  # В XDG_DATA_DIRS кладётся каталог, СОДЕРЖАЩИЙ `glib-2.0/schemas`, а в nixpkgs
  # схемы лежат на уровень глубже — в
  # `share/gsettings-schemas/<имя-пакета-с-версией>`. Отсюда `.name`: просто
  # `${pkgs.gtk3}/share` не сработает.
  #
  # Нужно только dev-shell'у: пакет из `desktop.nix` получает то же самое от
  # `wrapGAppsHook3`.
  gsettingsSchemaDirs = pkgs.lib.concatStringsSep ":" [
    "${pkgs.gtk3}/share/gsettings-schemas/${pkgs.gtk3.name}"
    "${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}"
  ];
}
