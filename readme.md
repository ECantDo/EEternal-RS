<div align="center">
	<img width="256" height="256" alt="EEternalRS Icon" src="https://github.com/user-attachments/assets/73db6acf-4bdc-4e05-82a0-6961222d5345" />
	<h1>EEternal Chess</h1>
</div>

---

A UCI-compatible chess engine, rewritten from my
[C++ implementation](https://github.com/ECantDo/EEternal-Chess); the engine makes some strong moves based on a NNUE
evaluation.

The C++ implementation is roughly around 2900 ELO on a single thread, based on a private Swiss tournament, provided by
Qiles Corey, from the
[Stockfish Discord](https://discord.gg/GWDRS3kU6R) (roughly 360 engines played, some of the top engines were included).
My current goal is to get the engine back to where I left the C++ engine, then get to 3000+ ELO.

---

## Approx Elo

Time control: `10+0.1` (10 seconds, plus 0.1 seconds per move), 16MB hash.

| Version | Approx Elo | Games | Opponents | Opponent Elo | Results |
|---------|------------|-------|-----------|--------------|---------|
| [0.2.0](https://github.com/ECantDo/EEternal-RS/releases/tag/V0.2.0) | **3396 +/- 12** | 2344 | 26 | 3094 to 3525 | [games](docs/elo_results.md#020) |
| [0.1.16](https://github.com/ECantDo/EEternal-RS/releases/tag/V0.1.16) | **3382 +/- 12** | 2408 | 27 | 3087 to 3525 | [games](docs/elo_results.md#0116) |
| [0.1.15](https://github.com/ECantDo/EEternal-RS/releases/tag/V0.1.15) | **3399 +/- 13** | 2280 | 25 | 3102 to 3525 | [games](docs/elo_results.md#0115) |
| [0.1.14](https://github.com/ECantDo/EEternal-RS/releases/tag/V0.1.14) | **3378 +/- 13** | 2280 | 25 | 3102 to 3525 | [games](docs/elo_results.md#0114) |
| [0.1.13](https://github.com/ECantDo/EEternal-RS/releases/tag/V0.1.13) | **3361 +/- 12** | 2408 | 27 | 3087 to 3525 | [games](docs/elo_results.md#0113) |
| [0.1.12](https://github.com/ECantDo/EEternal-RS/releases/tag/V0.1.12) | **3343 +/- 12** | 2408 | 27 | 3087 to 3525 | [games](docs/elo_results.md#0112) |
| [0.1.11](https://github.com/ECantDo/EEternal-RS/releases/tag/V0.1.11) | **3351 +/- 12** | 2408 | 27 | 3087 to 3525 | [games](docs/elo_results.md#0111) |
| [0.1.10](https://github.com/ECantDo/EEternal-RS/releases/tag/V0.1.10) | **3337 +/- 12** | 2472 | 28 | 3049 to 3525 | [games](docs/elo_results.md#0110) |
| [0.1.9](https://github.com/ECantDo/EEternal-RS/releases/tag/V0.1.9) | **3317 +/- 12** | 2536 | 29 | 3020 to 3525 | [games](docs/elo_results.md#019) |
| [0.1.8](https://github.com/ECantDo/EEternal-RS/releases/tag/V0.1.8) | **3266 +/- 13** | 2216 | 24 | 3015 to 3438 | [games](docs/elo_results.md#018) |
| [0.1.7](https://github.com/ECantDo/EEternal-RS/releases/tag/V0.1.7) | **3202 +/- 13** | 2326 | 26 | 2902 to 3391 | [games](docs/elo_results.md#017) |
| [0.1.6](https://github.com/ECantDo/EEternal-RS/releases/tag/V0.1.6) | **3173 +/- 13** | 2280 | 25 | 2880 to 3367 | [games](docs/elo_results.md#016) |
| [0.1.5](https://github.com/ECantDo/EEternal-RS/releases/tag/V0.1.5) | **3159 +/- 13** | 2216 | 24 | 2880 to 3362 | [games](docs/elo_results.md#015) |
| [0.1.4](https://github.com/ECantDo/EEternal-RS/releases/tag/V0.1.4) | **3071 +/- 14** | 2149 | 23 | 2739 to 3270 | [games](docs/elo_results.md#014) |
| [0.1.3](https://github.com/ECantDo/EEternal-RS/releases/tag/V0.1.3) | **3069 +/- 14** | 2152 | 23 | 2739 to 3244 | [games](docs/elo_results.md#013) |
| [0.1.2](https://github.com/ECantDo/EEternal-RS/releases/tag/V0.1.2) | **2949 +/- 13** | 2216 | 24 | 2655 to 3219 | [games](docs/elo_results.md#012) |
| [0.1.1](https://github.com/ECantDo/EEternal-RS/releases/tag/V0.1.1) | **2888 +/- 11** | 3280 | 20 | 2553 to 3087 | [games](docs/elo_results.md#011) |
| [**0.1.0**](https://github.com/ECantDo/EEternal-RS/releases/tag/V0.1.0) | **2786 +/- 12** | 2960 | 16 | 2553 to 2934 | [games](docs/elo_results.md#010) |
| [0.0.9](https://github.com/ECantDo/EEternal-RS/releases/tag/V0.0.9) | **2696 +/- 14** | 2000 | 2 | 2602 to 2616 | [games](docs/elo_results.md#009) |
| [0.0.8](https://github.com/ECantDo/EEternal-RS/releases/tag/V0.0.8) | **2653 +/- 15** | 2399 | 3 | 2246 to 2602 | [games](docs/elo_results.md#008) |
| [0.0.7](https://github.com/ECantDo/EEternal-RS/releases/tag/V0.0.7) | **2607 +/- 10** | 2000 | 2 | 2468 to 2602 | [games](docs/elo_results.md#007) |
| [0.0.6](https://github.com/ECantDo/EEternal-RS/releases/tag/v0.0.6) | **2468 +/- 11** | 999 | 1 | 2393 | [games](docs/elo_results.md#006) |
| [0.0.5](https://github.com/ECantDo/EEternal-RS/releases/tag/V0.0.5) | **2393 +/- 20** | 1999 | 2 | 1713 to 2513 | [games](docs/elo_results.md#005) |
| [0.0.4](https://github.com/ECantDo/EEternal-RS/releases/tag/V0.0.4) | **1399 +/- 17** | 2000 | 2 | 1085 to 1387 | [games](docs/elo_results.md#004) |
| [0.0.3](https://github.com/ECantDo/EEternal-RS/releases/tag/V0.0.3) | **1371 +/- 16** | - | 1 | 1085 | [games](docs/elo_results.md#003) |
| [0.0.2](https://github.com/ECantDo/EEternal-RS/releases/tag/V0.0.2) | **1098 +/- 12** | - | 1 | 1026 | [games](docs/elo_results.md#002) |
| [0.0.1](https://github.com/ECantDo/EEternal-RS/releases/tag/V0.0.1-RNG) | **1** | - | - | - | - |

[Read more about the Elo Calculation here](docs/elo_calculation.md#computing-the-engine-elo)

[Read more about the Engines used here](docs/elo_calculation.md#engines-used)

---

## Approx CCRL Blitz Elo

Time control: `60+0.1` (60 seconds, plus 0.1 seconds per move), 128MB hash.

| Tested Version | Approx Elo\*            | VS                                                                                 | WLD                                                                   | Opponent CCRL Elo            |
|----------------|-------------------------|------------------------------------------------------------------------------------|-----------------------------------------------------------------------|------------------------------|
| 0.2.0          | **3388 +/- 19** \[#144] | `Wahoo 4.0.0`, `Pounce 3.0.0`, `StockNemo 5.7.0.0`, `Mantissa 3.7.2`, `Stash 37.0` | (158, 5, 37), (113, 20, 67), (97, 26, 77), (76, 51, 73), (57, 80, 63) | 3094, 3219, 3270, 3311, 3419 |
| 0.1.13         | **3344 +/- 21** \[#155] | `Wahoo 4.0.0`, `Pounce 3.0.0`, `StockNemo 5.7.0.0`, `Mantissa 3.7.2`, `Stash 37.0` | (115, 13, 32), (78, 24, 58), (63, 32, 65), (59, 44, 57), (42, 75, 43) | 3094, 3219, 3270, 3311, 3419 |
| 0.1.1          | **2943 +/- 18** \[#257] | `Reckless 0.4.0`, `Stash 24.0`, `Boychesser`, `Raphael 2.1`                        | (103, 93, 104), (126, 87, 87), (187, 54, 59), (202, 29, 69)           | 2934, 2880, 2772, 2739       |

[Read more about CCRL Testing](docs/elo_calculation.md#ccrl-testing)

\*[Placement](docs/elo_calculation.md#placement)

---

## Feature History

| Version Number | Feature(s) Added                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
|----------------|------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| 0.0.1          | Full [move generation](https://www.chessprogramming.org/Move_Generation) capability; The engine can generate all legal moves, and play a random move from one of those moves.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| 0.0.2          | [NegaMax](https://www.chessprogramming.org/Negamax) with [Alpha-Beta](https://www.chessprogramming.org/Alpha-Beta) pruning, with a fixed search depth of 5. This bot played surprisingly well. <br> Evaluation is based purely on the material value of the position.                                                                                                                                                                                                                                                                                                                                                                                                              |
| 0.0.3          | [Iterative Deepening](https://www.chessprogramming.org/Iterative_Deepening) with basic [Time Management](https://www.chessprogramming.org/Time_Management). Not much else to add here...                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| 0.0.4          | [Transposition Table](https://www.chessprogramming.org/Transposition_Table); The engine can store previous positions. <br> [Move Ordering](https://www.chessprogramming.org/Move_Ordering) + SEE; Since the engine had TTs, and that the move ordering sucked, adding a way to order the best move from the TT seemed like a good idea. <br> I decided to use SEE ([Static Exchange Evaluation](https://www.chessprogramming.org/Static_Exchange_Evaluation)) to help order moves. <br> While I was at it, I also added [Quiescence Search](https://www.chessprogramming.org/Quiescence_Search), since I am also wanting to add my NNUE, and Q Search was having issues by itself. |
| 0.0.5          | It finally happened, I re-implemented my [NNUE](https://www.chessprogramming.org/NNUE) (Efficiently Updateable Neural Networks). Likely my biggest gain since version 0.0.2.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| 0.0.6          | [Late Move Reductions](https://www.chessprogramming.org/Late_Move_Reductions) and [Check Extensions](https://www.chessprogramming.org/Check_Extensions). Both of which change the search depth to explore more/less of the search tree based on how likely it is to be better or worse than what has already been explored.                                                                                                                                                                                                                                                                                                                                                        |
| 0.0.7          | Major bug fix; was using the fullmove counter, as the half-move clock; so games lasting more than 50 full moves (or even just searching into 50 full moves) evaluated the board as a draw. Resulting in throwing A LOT of games.                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| 0.0.8          | Included [Pricipal Variation Search](https://www.chessprogramming.org/Principal_Variation_Search) (PVS). During search, on non-PV nodes, the search does a [Null Window](https://www.chessprogramming.org/Null_Window) to prove that the move is worse than the current PV.                                                                                                                                                                                                                                                                                                                                                                                                        |
| 0.0.9          | Added [Killer Moves](https://www.chessprogramming.org/Killer_Move). Killer moves are quiet (non-capturing) moves that previously caused a beta cutoff (`alpha >= beta`) at the same ply. Since they're often strong regardless of what led up to them, they get tried early when searching sibling branches. For example, if a move finds mate and the opponent doesn't block it, that same move is likely to still be strong against a lot of other responses.                                                                                                                                                                                                                    |
| 0.1.0          | Added a [History Heuristic](https://www.chessprogramming.org/History_Heuristic) for quiet moves [Read More](docs/history_heuristics.md).<br>I am happy to make this my 0.1 release, as it is winning against EEternal V3.2 (C++), in some short tests; and being my goal to match that engine, we have made it.                                                                                                                                                                                                                                                                                                                                                                    |
| 0.1.1          | Many little tweaks happening here and there, some with [Extensions](docs/extensions.md), [Noisy History](docs/history_heuristics.md#noisy-history)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| 0.1.2          | Added [Null Move Pruning](https://www.chessprogramming.org/Null_Move_Pruning) (NMP). On nodes expected to fail high, the engine "passes" its turn and does a reduced search; if it still beats beta, the whole node gets pruned. SPRT vs 0.1.1: **+77.9 +/- 22.6** Elo.                                                                                                                                                                                                                                                                                                                                                                                                            |
| 0.1.3          | Added [Reverse Futility Pruning](https://www.chessprogramming.org/Reverse_Futility_Pruning) (RFP). At shallow non-PV nodes where the static eval beats beta by a depth-scaled margin (90 cp/ply, depth <= 8), the node is pruned without searching, returning a value halfway between the eval and beta. SPRT vs 0.1.2: **+138.9 +/- 31.8** Elo.                                                                                                                                                                                                                                                                                                                                   |
| 0.1.4          | Added [Late Move Pruning](https://www.chessprogramming.org/Futility_Pruning#MoveCountBasedPruning) (LMP). At non-root nodes not in check, once `3 + depth^2` moves have been searched, the remaining quiet (non-promotion) moves are skipped without being searched. SPRT vs 0.1.3: **+9.5 +/- 6.7** Elo.                                                                                                                                                                                                                                                                                                                                                                          |
| 0.1.5          | Added [Aspiration Windows](https://www.chessprogramming.org/Aspiration_Windows). From depth 4, each iteration first searches a narrow window (±25 cp) around the previous score, and widens it (×1.5) on a fail high/low. Also fixed a bug where fail-low PV nodes could produce illegal PV lines. SPRT vs 0.1.4: **+87.6 +/- 21.8** Elo.                                                                                                                                                                                                                                                                                                                                          |
| 0.1.6          | Added [Internal Iterative Reductions](https://www.chessprogramming.org/Internal_Iterative_Reductions) (IIR). At PV and cut nodes (depth >= 4) with no TT move, the search depth is reduced by 1, since the move ordering there is poor anyway. SPRT vs 0.1.5: **+10.0 +/- 7.0** Elo.                                                                                                                                                                                                                                                                                                                                                                                               |
| 0.1.7          | Fixed an off-by-one bug in [Static Exchange Evaluation](https://www.chessprogramming.org/Static_Exchange_Evaluation) (SEE): the final, speculative recapture was always counted, so taking any undefended piece with a more valuable piece scored as losing (e.g. QxP on a hanging pawn gave -1000). This fixed capture ordering and qsearch SEE pruning. SPRT vs 0.1.6: **+65.5 +/- 19.3** Elo.                                                                                                                                                                                                                                                                                   |
| 0.1.8          | Added SEE pruning: near the leaves (depth <= 8), moves that lose too much material by [SEE](https://www.chessprogramming.org/Static_Exchange_Evaluation) are skipped before being made (quiets below -64 * depth, captures below -20 * depth^2). SEE now also scores quiet moves. Also fixed hard time-limit stops so an aborted search no longer triggers re-searches. SPRT vs 0.1.7: **+76.0 +/- 19.8** Elo.                                                                                                                                                                                                                                                                     |
| 0.1.9          | Captures that lose material (SEE < 0) are now ordered after the killer moves instead of with the good captures. SPRT vs 0.1.8: **+44.4 +/- 15.9** Elo. Also fixed the build so `target-cpu=native` is actually applied (AVX2 for the NNUE, ~16% faster). SPRT native vs plain build: **+27.7 +/- 12.5** Elo.                                                                                                                                                                                                                                                                                                                                                                       |
| 0.1.10         | The search data (quiet and noisy history) is now kept between moves instead of being rebuilt on every `go`; it is only reset on `ucinewgame`. Killers and other per-search state are still reset each search. SPRT vs 0.1.9: **+23.7 +/- 11.6** Elo.                                                                                                                                                                                                                                                                                                                                                                                                                               |
| 0.1.11         | On a fail low, the [transposition table](https://www.chessprogramming.org/Transposition_Table) no longer stores the "best" move (it is only the least-bad of the moves that failed low); the move already stored for that position is kept instead. SPRT vs 0.1.10: **+17.6 +/- 9.9** Elo.                                                                                                                                                                                                                                                                                                                                                                                         |
| 0.1.12         | Fixed [delta pruning](https://www.chessprogramming.org/Delta_Pruning) in QSearch treating en passant as capturing nothing, so en passant captures were pruned too easily. SPRT vs 0.1.11: **+7.9 +/- 5.9** Elo.                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| 0.1.13         | Staged move generation (a move picker, ported from the C++ engine): TT move, good captures, killers, bad captures, then quiets. Quiet moves are only generated when nothing earlier caused a cutoff, and are dropped once late move pruning starts (~16% faster). SPRT vs 0.1.12: **+37.3 +/- 14.5** Elo.                                                                                                                                                                                                                                                                                                                                                                          |
| 0.1.14         | [Late move pruning](https://www.chessprogramming.org/Futility_Pruning#MoveCountBasedPruning) uses an *improving* flag (static eval compared with 2 plies ago, falling back to 4): when not improving, the quiet-move limit `3 + depth²` is halved. SPRT vs 0.1.13: **+34.1 +/- 14.2** Elo.|
| 0.1.15         | [Late move reductions](https://www.chessprogramming.org/Late_Move_Reductions) reduce one more ply when not improving (from the 4th move on). SPRT vs 0.1.14: **+22.0 +/- 11.2** Elo.                                                                                                      |
| 0.1.16         | [Reverse futility pruning](https://www.chessprogramming.org/Reverse_Futility_Pruning) margin uses `depth - improving`, so it cuts one depth-step sooner when the static eval is improving. SPRT vs 0.1.15: **+16.0 +/- 9.3** Elo.                                                         |
| 0.2.0          | Pawn [correction history](https://www.chessprogramming.org/Static_Evaluation_Correction_History): a table indexed by side to move and a pawn-structure hash learns how far the static eval tends to be off in that pawn structure, and adds `entry / 64` to the static eval (in search and QSearch stand pat). After each search it is updated towards `best_score - static_eval`, scaled by depth, skipped when in check, when the TT move is a capture, or when the bound can't tell which way the error goes. SPRT vs 0.1.16: **+5.5 +/- 4.4** Elo. |
| 0.2.1          | Search parameters tuned with [SPSA](https://www.chessprogramming.org/SPSA): the search's numbers are now compile-time parameters (`build/params.csv`), and 30 of them were tuned together over 160,000 fixed-node self-play games. The largest changes: reverse futility margin 90 to 45, late move reductions from depth 4 instead of 2, correction history divisor 64 to 101, history bonus multiplier 4 to 6, and new SEE piece values. SPRT vs 0.2.0: **+35.2 +/- 14.1** Elo. |

**Why share *everything*?**<br>
I am wanting to save a full history of changes that I have made, to track the Elo progression of my engine. Furthermore,
I would like to be able to look back on this project and point at some of the most important features to add when
starting out with chess programming.

### UCI

The following UCI commands are implemented:

| Command      | Description                                          |
|--------------|------------------------------------------------------|
| uci          | Tell the engine to use the UCI protocol              |
| isready      | Confirms the engine is ready                         |
| ucinewgame   | Starts a new game                                    |
| position     | Sets up the position                                 |
| go           | Supports `depth`, `movetime`, `wtime/btime`, `nodes` |
| go perft     | Runs (non-bulk) perft for the current position       |
| go bulkperft | Runs bulk perft for the current position             |
| quit         | Stops the engine                                     |
| stop         | Stops the current search                             |
| d            | Display the current board state                      |
| setoption    | Set an option for the engine                         |

#### Options

| Option Name | Default Value | Type | Range     |
|-------------|---------------|------|-----------|
| Hash        | 16 (mb)       | spin | 1 to 4096 |

---

## Why the rewrite?

The C++ version's multithreading was bolted on early without much thought and became painful to extend. This version
aims to get the architecture right, from the start. ... Ok there was thought behind adding it, but this was my first
proper chess engine, so I also didn't know what I was doing.

---

## Use of AI

My use of AI in this repo mainly boils down to helping update the readme after testing completes, and running my
tests (SPRTs, gauntlets) for me. Commits where the AI did all the work are marked with a `Co-Authored-By` line.

Outside the repository, I am mainly utilizing it to help find my old code in my C++ engine, for the features and how I
did them before.
