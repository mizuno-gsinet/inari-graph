# Graphest Graphing Engine

<p align="center">
  <img src="images/cover.gif"><br>
  The graph of sin(<i>x</i> ± sin <i>y</i>) (sin <i>x</i> ± <i>y</i>) = cos(sin((sin <i>x</i> ± cos <i>y</i>) (sin <i>y</i> ± cos <i>x</i>))) over [4, 6.5] × [2, 4.5].
</p>

## Binaries

### `graph`

```bash
cargo r --bin graph --release --features "arb" -- "y = sin(x)"
```

By default:

- The plot will be saved to `graph.png` in the current directory.

- The following colors are used:
  - ![Black](images/black.png) There is at least one solution in the pixel. (Solution is any point that satisfies the relation.)
  - ![Blue](images/blue.png) There may or may not be solutions in the pixel.
  - ![White](images/white.png) There are no solutions in the pixel.

Use `-h` option to view usage.

## Conditional Features

- `arb` - Use [Arb](https://arblib.org) to boost plotting performance and enable additional functions. With this feature enabled, it can take quote a long time (~45 minutes) to build the binaries. You might want to comment out the statements that contains `.arg("check")` in [build.rs](build.rs) to bypass unit tests.
