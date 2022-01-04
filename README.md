## Rust wrapper for libdecor

> [libdecor - A client-side decorations library for Wayland client](https://gitlab.gnome.org/jadahl/libdecor)
>
> libdecor is a library that can help Wayland clients draw window
> decorations for them. It aims to provide multiple backends that implements the
> decoration drawing.

Documentation for the main branch can be found [here](https://cmeissl.github.io/libdecor-rs)

## Dependencies

Required:

- `libdecor`
- `libwayland-client`

Install via dnf

```sh
sudo dnf install libdecor libwayland-client
```

## Building

```sh
cargo build
```

## Examples

### Demo

The rust [demo.rs](libdecor/examples/demo.rs) is a translation of the
original `C` [demo.c](https://gitlab.gnome.org/jadahl/libdecor/-/blob/master/demo/demo.c).

```sh
cargo run --release --example demo
```
