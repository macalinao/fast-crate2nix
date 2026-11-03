# Checks that [profile.release] opt-level/codegen-units reach rustc, and
# that `useCargoProfiles = false` restores buildRustCrate's defaults.
{ pkgs ? import ../../nix/nixpkgs.nix { config = { }; }
, lib ? pkgs.lib
, generatedCargoNix ? ./Cargo.nix
}:
let
  withProfiles = pkgs.callPackage generatedCargoNix { };
  withoutProfiles = pkgs.callPackage generatedCargoNix { useCargoProfiles = false; };
  root = withProfiles.rootCrate.build;
  dep = lib.head root.dependencies;
  has = drv: flag: lib.hasInfix flag drv.buildPhase;
in
assert has root "-C opt-level=1";
assert has root "-C codegen-units=4";
assert has dep "-C opt-level=s";
assert has dep "-C codegen-units=2";
assert !(has withoutProfiles.rootCrate.build "-C opt-level=1");
assert !(has withoutProfiles.rootCrate.build "-C codegen-units=4");
root
