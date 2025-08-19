# cargo-vendor-checksum
It is a tool to update checksums of modified files in vendor directory, e.g.
when patching sources for building packages. One can specify files via
`--files-in-vendor-dir` flag or run batch process via `--all` or `--packages`.

Flag `--ignore-missing` could be used to remove checksums for absent files.
For example after deleting `.a` files. Otherwise `cargo-vendor-checksum` return
an error `failed to get checksum for file`.

## Examples
##### If you already in the package catalog

Specify the files with checksums that need to be updated. For files
`nix/src/net/mod.rs` and `nix/Cargo.toml` in vendored package `nix` it must be:
```
cargo-vendor-checksum --files-in-vendor-dir nix/src/net/mod.rs nix/Cargo.toml
```

Update checksum of all vendored files via batch process (this command could
take a lot of time to complete):
```
cargo-vendor-checksum --all
```

Specify vendored packages for batch processing. E.g. to update checksums for
all files of vendored packages `nix` and `anyhow`:
```
cargo-vendor-checksum --packages anyhow nix
```

##### Outside of the package catalog

Set the path of vendor directory via flag `--vendor`:
```
cargo-vendor-checksum --vendor $PWD/hyperfine/vendor --all
```
