# cargo-vendor-checksum
It is a tool for updating checksum files in vendor directories. After
modifying files, e.g. when patching sources for building packages.

## Examples
If you already in the package catalog. Run the following command to update
checksum for file `nix/src/net/mod.rs`:
```
cargo-vendor-checksum --files-in-vendor-dir nix/src/net/mod.rs
```

Vendor directory could be specified via `vendor` flag:
```
cargo-vendor-checksum --vendor $PWD/hyperfine/vendor -f nix/src/net/mod.rs
```

Checksum for alt files could be updated via command:
```
cargo-vendor-checksum --all
```
This command could take a lot of time to complete.
