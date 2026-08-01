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

\* _[See below](#approx-elo) for the ELO improvement_

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

---

## Why the rewrite?

The C++ version's multithreading was bolted on early without much thought and became painful to extend. This version
aims to get the architecture right, from the start. ... Ok there was thought behind adding it, but this was my first
proper chess engine, so I also didn't know what I was doing.

---

## Approx ELO

| Version Number                                                          | Approx ELO      | VS                                                                                                                                      | VS Elo (Assumed) | WLD                                             |
|-------------------------------------------------------------------------|-----------------|-----------------------------------------------------------------------------------------------------------------------------------------|------------------|-------------------------------------------------|
| 0.1.0                                                                   | **2714 +/- 14** | `EEternal Chess V3.2`, `Reckless 0.3.0`                                                                                                 | 2721, 2616       | (544, 269, 187), (547, 234, 219)                |
| [0.0.9](https://github.com/ECantDo/EEternal-RS/releases/tag/V0.0.9)     | **2696 +/- 14** | `drip leab` (by: JeffLegendPower, AtadOfAChad, Gonumen, KevinStriker), `Reckless 0.3.0`                                                 | 2602, 2616       | (614, 207, 179), (452, 356, 192)                |
| [0.0.8](https://github.com/ECantDo/EEternal-RS/releases/tag/V0.0.8)     | **2653 +/- 15** | `drip leab` (by: JeffLegendPower, AtadOfAChad, Gonumen, KevinStriker), `TinyHugeBot` (by: Popax21 & atpx8), `NNBot` (by: Jamie Whiting) | 2602, 2513, 2246 | (418, 261, 120), (519, 150, 131), (646, 67, 87) |
| [0.0.7](https://github.com/ECantDo/EEternal-RS/releases/tag/V0.0.7)     | **2607 +/- 10** | `EEternalRS_V0.0.6`, `drip leab` (by: JeffLegendPower, AtadOfAChad, Gonumen, KevinStriker)                                              | 2468, 2602       | (481, 43, 476), (317, 318, 365)                 |
| [0.0.6](https://github.com/ECantDo/EEternal-RS/releases/tag/v0.0.6)     | **2468 +/- 11** | `EEternalRS_V0.0.5`                                                                                                                     | 2393             | (255, 42, 702)                                  |
| [0.0.5](https://github.com/ECantDo/EEternal-RS/releases/tag/V0.0.5)     | **2393 +/- 20** | `Game Tech Explained Bot` (by: Game Tech Explained), `TinyHugeBot` (by: Popax21 & atpx8)                                                | 1713, 2513       | (986, 6, 8), (222, 565, 212)                    |
| [0.0.4](https://github.com/ECantDo/EEternal-RS/releases/tag/V0.0.4)     | **1399 +/- 17** | `DLComp2` (by: DawnLamp8), `applemethod-orz` (by: RedBlackTree)                                                                         | 1387, 1085       | (526, 459, 15), (789, 94, 117)                  |
| [0.0.3](https://github.com/ECantDo/EEternal-RS/releases/tag/V0.0.3)     | **1371 +/- 16** | `applemethod-orz` (by: RedBlackTree)                                                                                                    | 1085             | Not Saved                                       |
| [0.0.2](https://github.com/ECantDo/EEternal-RS/releases/tag/V0.0.2)     | **1098 +/- 12** | `Turochamp` (Faithful) (by: P Rivero)                                                                                                   | 1026             | Not Saved                                       |
| [0.0.1](https://github.com/ECantDo/EEternal-RS/releases/tag/V0.0.1-RNG) | **1**           | N/A                                                                                                                                     | N/A              | N/A                                             |

[Read more about the Elo Calculation here](docs/elo_calculation.md).
