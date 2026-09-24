{
  description = "Strudel WAV Exporter — Tauri app (Rust + Svelte)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" "clippy" "rustfmt" ];
        };
      in
      {
        devShells.default = pkgs.mkShell {
          nativeBuildInputs = [
            rustToolchain
            pkgs.cargo-watch
            pkgs.cargo-expand
            pkgs.cargo-edit
            pkgs.pkg-config

            # Tauri CLI
            pkgs.cargo-tauri

            # Node / frontend
            pkgs.nodejs_22
            pkgs.pnpm
          ];

          buildInputs = [
            # Tauri v2 — webview e sistema
            pkgs.webkitgtk_4_1
            pkgs.openssl
            pkgs.glib
            pkgs.libsoup_3
            pkgs.cairo
            pkgs.pango
            pkgs.atk
            pkgs.gdk-pixbuf
            pkgs.gtk3

            # Tauri extras
            pkgs.librsvg
            pkgs.xdotool

            # GStreamer — WebKit2GTK usa para Web Audio API e <audio>
            pkgs.gst_all_1.gstreamer
            pkgs.gst_all_1.gst-plugins-base  # appsink, appsrc, volume, etc.
            pkgs.gst_all_1.gst-plugins-good  # autoaudiosink (autodetect), pulseaudio
            pkgs.gst_all_1.gst-plugins-bad
            pkgs.gst_all_1.gst-libav
          ];

          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
          RUST_BACKTRACE = "1";
          PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";

          # Força X11 via XWayland — WebKit2GTK tem conflito de protocolo em Wayland puro
          GDK_BACKEND = "x11";
          WEBKIT_DISABLE_COMPOSITING_MODE = "1";

          # Necessário pro webkit2gtk encontrar os recursos
          XDG_DATA_DIRS = "${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}:${pkgs.gtk3}/share/gsettings-schemas/${pkgs.gtk3.name}:$XDG_DATA_DIRS";
          GIO_MODULE_DIR = "${pkgs.glib-networking}/lib/gio/modules/";

          # GStreamer plugin path para WebKit2GTK encontrar os elementos de áudio
          GST_PLUGIN_SYSTEM_PATH_1_0 = pkgs.lib.makeSearchPathOutput "lib" "lib/gstreamer-1.0" (with pkgs.gst_all_1; [
            gstreamer
            gst-plugins-base
            gst-plugins-good
            gst-plugins-bad
            gst-libav
          ]);

          shellHook = ''
            echo "Strudel WAV Exporter — dev shell"
            echo "rust  $(rustc --version)"
            echo "cargo $(cargo --version)"
            echo "node  $(node --version)"
            echo "pnpm  $(pnpm --version)"
          '';
        };
      }
    );
}
