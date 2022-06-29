# Raw to Image

Converts raw image files produced by cameras into jpeg files.
Currently only supports CR2, but the raw formats listed [here](https://crates.io/crates/rawloader/) should be easy to add.
Just open an issue so I can add them to the whitelist.

# Building the binary
You can (nightly) build release versions with the following command:
```sh
cargo build --release
```
