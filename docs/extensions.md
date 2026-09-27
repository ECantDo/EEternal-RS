# Extensions

What I call extensions relate to two separate things, Check *Extensions* and Late Move Reductions. Both "extend" the
search in some way. Here, we will be exploring both types.

```rust
for move_entry in ordered_moves {
    let mv = move_entry.mv();
    let is_quiet = mv.is_quiet();
    
    search_data.make_move(mv);
    
    let mut check_extensions: i32 = 0;
    let mut lmr_reductions: i32 = 0;
    if search_data.board.in_check() {
        check_extensions += 1; // Check extensions
    } else if depth > = 2 && moves_searched >= 2 {
        // search the first 3 moves in full, then LMR
        lmr_reductions = LMR_TABLE[depth as usize][moves_searched];
    }
    // ...
    let new_depth = depth + check_extensions - 1 - lmr_reductions;
    let score = - search::<PV>(search_data, - beta, - alpha, new_depth, ply + 1);
    //...
}
```

---

## Check Extensions

Check Extensions let the engine search another move deeper, when the current board is in check. The standard for check
extensions is to extend once. However, I wanted to try giving the engine an extra move when it is the PV line being
searched.

```rust
if search_data.board.in_check() {
    if Node::PV && ply < 128 {
        check_extensions += 1;
    }
    check_extensions += 1; // Check extensions
}
```

```terminaloutput
Score of EEternalRS-0_1_1a vs EEternalRS-0_1_0: 9 - 116 - 44  [0.183] 169
Elo difference: -259.4 +/- 51.6, LOS: 0.0 %, DrawRatio: 26.0 %
SPRT: llr -2.96 (-100.4%), lbound -2.94, ubound 2.94 - H0 was accepted
```

Needless to say, don't do this. This is a very bad idea. Firstly, you need an extra check since the depth bounds grow
really quickly, to make sure to not go over your PV limit (`ply < 128`). And secondly, it just seems to be a terrible
idea.

The next thing I wanted to try has to do with LMR, so keep reading.

---

## Late Move Reductions (LMR)

Late Move Reductions, assume that after a certain number of moves, we are not going to be finding the best move. I.e.
the best move in a given position is likely to be either the first, or second move searched. Therefore, we can reduce
the number of moves we look through to just check that every other move is worse.

Intuition would tell you to not do a search reduction when in check; However, this seems to be just fine to do. Likely
due to the fact that your best checks are likely still going to be among the first moves searched.

```rust
if depth >= 2 && moves_searched >= 2 { // Importantly no `else` here vs the above
    lmr_reductions = LMR_TABLE[depth as usize][moves_searched];
}
```

```terminaloutput
Score of EEternalRS-0_1_1b vs EEternalRS-0_1_0: 446 - 318 - 549  [0.549] 1313
Elo difference: 34.0 +/- 14.3, LOS: 100.0 %, DrawRatio: 41.8 %
SPRT: llr 2.96 (100.4%), lbound -2.94, ubound 2.94 - H1 was accepted
```

If you want free Elo, just drop your `else` check, when making your `is_in_check()` call. Furthermore, please do check
if your LMR reduction has failed high, you still need to recheck the full search.

I should also include, I got these results from this LMR table:

```rust
pub static LMR_TABLE: LazyLock<[[i32; MAX_PLY]; MAX_PLY]> = LazyLock::new(|| {
    std::array::from_fn(|depth| {
        std::array::from_fn(|moves| lmr_reduction(depth as i32, moves as i32))
    })
});
fn lmr_reduction(depth: i32, moves_searched: i32) -> i32 {
    (0.75 + (depth.max(1) as f64).ln() * (moves_searched.max(1) as f64).ln() / 2.5).max(0.0) as i32
}
```

Since it is important to know where I am getting my values from, if you have a different LMR table, your results
will almost certainly be different.

I do want to revisit checking for a check later just to double-check as my engine improves, but for now, this was
surprising result.

**As an added test**, since previously, my LMR knew for a fact that the check extensions could not happen, I was not
adding them back in on the re-search. So double-checking that adding back in our extensions is a good idea on a recheck:

```rust
if lmr_reductions > 0 && score > alpha {
    score = - search::<NonPV>(
        search_data,
        - alpha - 1,
        - alpha,
        depth + check_extensions - 1,
        ply + 1,
    );
}
```

During my testing, this change showed a roughly equal quality of play, however some 2000 games in, it did start to show
some promise:

```terminaloutput
Score of EEternalRS-0_1_1c vs EEternalRS-0_1_1b: 1240 - 1162 - 1598  [0.510] 4000
Elo difference: 6.8 +/- 8.3, LOS: 94.4 %, DrawRatio: 40.0 %
SPRT: llr 1.18 (40.1%), lbound -2.94, ubound 2.94
```

I believe the issue is that in 99% of positions, you're not in check, so this extra step has been skipped most of the
time. This does lead me to believe that adding this in, has improved the quality of play by another 3 (ish) Elo points.
So, at this point I am happy to just say it works, without needing to run something like 10,000 games, since these cases
are rare, and when they do show up, they tend to show improvement. Plus it also just seems reasonable that the reason
why I left this feature out in the first place, was due to knowing that if I had the value it, it would always have been
`0`.

I mainly just don't want to be running this for some 15000 games to verify a rarer case that has been left out for
values of always `0`.
