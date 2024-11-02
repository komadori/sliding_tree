Sliding Tree
============

This crate is a Rust library which provides a sliding tree structure that grows
from the leaves and recedes from the root. It is intended to be suitable for
implementing game tree search. It uses a queue of allocation buffers under the
hood to manage memory.

## Dependency

```toml
[dependencies]
sliding-tree = "0.2"
```

## Usage

An example demonstrating how to use the crate to implement Monte Carlo Tree
Search for a simple game is provided in `tests/mcts.rs`.

## Licence

This crate is licensed under the Apache License, Version 2.0 (see
LICENCE-APACHE or <http://www.apache.org/licenses/LICENSE-2.0>) or the MIT
licence (see LICENCE-MIT or <http://opensource.org/licenses/MIT>), at your
option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
