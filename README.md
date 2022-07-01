# Raw to Image

Converts raw image files produced by cameras into regular image files.
Also works recursively on whole file trees instead of just a single folder or image.

Currently only supports CR2, but the raw formats listed [here](https://crates.io/crates/rawloader/) should be easy to add.
Just open an issue so I can add them to the whitelist.
Pretty much the same goes for output formats as long as they are [supported by image-rs](https://docs.rs/image/latest/image/codecs/index.html).

### Supported raw formats:
* CR2

### Supported image formats:
* jpeg
* png
* tiff


# Building the binary

To build the latest release checkout the `latest` tag and run cargo like so:
```sh
git checkout latest && cargo build --release
```
The binary will then be located in `target/release/`.
