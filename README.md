# vulkan-tutorial

Code for the [Vulkan Tutorial](https://vulkan-tutorial.com) exercises, translated into Rust using the the [ash crate](https://github.com/MaikKlein/ash).

# Status

Commits correspond to the completion of each exercise (although you notice a few commits are missing for smaller exercises). Exercises up to 4.3 have been completed.

# Usage

A flake.nix is provided for Linux systems. If you are running Linux and you have [nix with flake support](https://github.com/mschwaig/howto-install-nix-with-flake-support) installed then you can simple run

```
nix develop
```

and then

```
cargo run
```
