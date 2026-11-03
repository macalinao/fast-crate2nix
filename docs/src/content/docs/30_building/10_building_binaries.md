---
title: Building binaries
---

## Single binary crates

If your `Cargo.nix` was generated for a single binary crate (i.e. workspace)
then the derivation that builds your binary can be accessed via the
`rootCrate.build` attribute. Use this command to build it and make the result
available in the result directory:

```bash
your_crate_name="super_duper"
nix build -f Cargo.nix rootCrate.build
./result/bin/${your_crate_name}
```

Within a nix file (e.g. your manually written `default.nix`), you can access the
derivation like this:

```nix
let cargo_nix = callPackage ./Cargo.nix {};
in cargo_nix.rootCrate.build
```

## Cargo workspaces with multiple crates

If your `Cargo.nix` was generated for a workspace (i.e. not a single binary)
then the derivation that builds your binary CANNOT be accessed via the
`rootCrate` attribute. There is no single root crate.

Instead, you can conveniently access the derivations of all your workspace
members through the `workspaceMembers` attribute. Use this command to build one
of the workspace members and make the result available in the result directory:

```bash
your_crate_name="super_duper"
nix build -f Cargo.nix workspaceMembers.${your_crate_name}.build
./result/bin/${your_crate_name}
```

Within a nix file (e.g. your manually written `default.nix`), you can access the
derivation like this:

```nix
let cargo_nix = callPackage ./Cargo.nix {};
in cargo_nix.workspaceMembers."${your_crate_name}".build
```

## Cargo profiles

`buildRustCrate` compiles every crate at `-C opt-level=3` with a fixed number
of codegen units. The generated `Cargo.nix` instead applies the `opt-level` and
`codegen-units` set in your workspace's `[profile.release]` (or
`[profile.dev]` when called with `release = false;`), including
`[profile.release.package."*"]` (all non-workspace crates) and
`[profile.release.package.<name>]` overrides, like cargo does. Only values you
set explicitly are used. For example, to give your own crates parallel codegen
while dependencies stay at one codegen unit:

```toml
[profile.release]
codegen-units = 16

[profile.release.package."*"]
codegen-units = 1
```

Call the `Cargo.nix` with `useCargoProfiles = false;` to ignore the profiles.
Other profile settings (`lto`, `debug`, `build-override`, ...) are not applied.
