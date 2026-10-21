RUST = ["cargo", "rustc", "rustdoc", "rustfmt", "cargo-fmt", "cargo-clippy", "clippy-driver"]

dev(
    packages = [pkg("rustup", links = {name: "rustup" for name in RUST})],
    # The dbus crate links libdbus through pkg-config on Linux.
    libs = [lib("dbus-1")] if platform.is_linux else [],
    # mkShell's stdenv brings a C compiler for build scripts; Xcode's does on macOS.
    bins = [bin("pkg-config"), bin("gcc")] if platform.is_linux else [],
    env = {"RUSTUP_TOOLCHAIN": "1.99.1"},
)
