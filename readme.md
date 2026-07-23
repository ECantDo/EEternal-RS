# EEternal Chess

---

A _mostly_ UCI-compatible chess engine, rewritten from
my [C++ implementation](https://github.com/ECantDo/EEternal-Chess); the engine can make reasonable moves, it does have a
fixed search depth, and a terrible evaluation, but it is making decisions.

The C++ implementation is roughly around 2900 ELO on a single thread, based on a private Swiss tournament, provided by
Qiles Corey, from the
[Stockfish Discord](https://discord.gg/GWDRS3kU6R) (roughly 360 engines played, some of the top engines were included).
My current goal is to get the engine back to where I left the C++ engine, then get to 3000+ ELO.

---

## Feature History

| Version Number | Feature(s) Added                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
|----------------|------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| 0.0.1          | Full [move generation](https://www.chessprogramming.org/Move_Generation) capability; The engine can generate all legal moves, and play a random move from one of those moves.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| 0.0.2          | [NegaMax](https://www.chessprogramming.org/Negamax) with [Alpha-Beta](https://www.chessprogramming.org/Alpha-Beta) pruning, with a fixed search depth of 5. This bot played surprisingly well. <br> Evaluation is based purly on the material value of the position.                                                                                                                                                                                                                                                                                                                                                                                                               |
| 0.0.3          | [Iterative Deepening](https://www.chessprogramming.org/Iterative_Deepening) with basic [Time Management](https://www.chessprogramming.org/Time_Management). Not much else to add here...                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| 0.0.4          | [Transposition Table](https://www.chessprogramming.org/Transposition_Table); The engine can store previous positions. <br> [Move Ordering](https://www.chessprogramming.org/Move_Ordering) + SEE; Since the engine had TTs, and that the move ordering sucked, adding a way to order the best move from the TT seemed like a good idea. <br> I decided to use SEE ([Static Exchange Evaluation](https://www.chessprogramming.org/Static_Exchange_Evaluation)) to help order moves. <br> While I was at it, I also added [Quiescence Search](https://www.chessprogramming.org/Quiescence_Search), since I am also wanting to add my NNUE, and Q Search was having issues by itself. |
| 0.0.5          | It finally happend, I re-implemented my [NNUE](https://www.chessprogramming.org/NNUE) (Efficiently Updateable Neural Networks). Likely my biggest gain since version 0.0.2.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| 0.0.6          | [Late Move Reductions](https://www.chessprogramming.org/Late_Move_Reductions) and [Check Extensions](https://www.chessprogramming.org/Check_Extensions). Both of wich change the search depth to expore more/less of the search tree based on how likely it is to be better or worse than what has already been explored.                                                                                                                                                                                                                                                                                                                                                          |
| 0.0.7          | Major bug fix; was using the fullmove counter, as the half-move clock; so games lasting more than 50 full moves (or even just searching into 50 full moves) evaluated the board as a draw. Resulting in throwing A LOT of games.                                                                                                                                                                                                                                                                                                                                                                                                                                                   |

\* _[See below](#approx-elo) for the ELO improvement_

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

The ELO has been calculated based on engines that played in
[Sebastian Lague's Chess Challenge](https://github.com/SebLague/Chess-Challenge),
[Ratings List](https://github.com/SebLague/Tiny-Chess-Bot-Challenge-Results/blob/main/RatingsList.txt)
of the Swiss and Knockout games played. The ELOs are based on the
[Boychesser](https://github.com/analog-hors/Boychesser) engine (ELO: 2772 +/- 11, at the time)

Games are played with a time control of `10+0.1s`, and with an opening book of 8 moves, each engine gets the chance to
play as both white and black, from the same position.

| Version Number                                                          | Approx ELO      | VS                                                                                       | VS Elo (Assumed) | WLD                            |
|-------------------------------------------------------------------------|-----------------|------------------------------------------------------------------------------------------|------------------|--------------------------------|
| [0.0.7]()                                                               | **2631 +/- 15** | `EEternalRS_V0.0.6`                                                                      | 2468             | (481, 43, 476)                 |
| [0.0.6](https://github.com/ECantDo/EEternal-RS/releases/tag/v0.0.6)     | **2468 +/- 11** | `EEternalRS_V0.0.5`                                                                      | 2393             | (255, 42, 702)                 |
| [0.0.5](https://github.com/ECantDo/EEternal-RS/releases/tag/V0.0.5)     | **2393 +/- 20** | `Game Tech Explained Bot` (by: Game Tech Explained), `TinyHugeBot` (by: Popax21 & atpx8) | 1713, 2513       | (986, 6, 8), (222, 565, 212)   |
| [0.0.4](https://github.com/ECantDo/EEternal-RS/releases/tag/V0.0.4)     | **1399 +/- 17** | `DLComp2` (by: DawnLamp8), `applemethod-orz` (by: RedBlackTree)                          | 1387, 1085       | (526, 459, 15), (789, 94, 117) |
| [0.0.3](https://github.com/ECantDo/EEternal-RS/releases/tag/V0.0.3)     | **1371 +/- 16** | `applemethod-orz` (by: RedBlackTree)                                                     | 1085             | Not Saved                      |
| [0.0.2](https://github.com/ECantDo/EEternal-RS/releases/tag/V0.0.2)     | **1098 +/- 12** | `Turochamp` (Faithful) (by: P Rivero)                                                    | 1026             | Not Saved                      |
| [0.0.1](https://github.com/ECantDo/EEternal-RS/releases/tag/V0.0.1-RNG) | **1**           | N/A                                                                                      | N/A              | N/A                            |

## Elo Calculation

_As of V0.0.4_

The ELO is calculated based on assuming that the *guessed* ELO ratings are the *true* ratings. That leads into the elo
ratings are still approximate, but can still be wildly off. The more games that are played, the better approximation
that we can get, relative to the assumed true ELO ratings of the played bots.

```py
import math

input_engine_elo: list[int] = [...]

wld: list[tuple[int, int, int]] = [...]


def calc_error(win_rate: tuple[int, int, int], p: float):
	N = sum(win_rate)
	p_w = win_rate[0] / N
	p_d = win_rate[2] / N
	p_l = win_rate[1] / N

	variance = p_w * ((1 - p) ** 2) + p_d * ((0.5 - p) ** 2) + p_l * ((0 - p) ** 2)
	std_err = (variance / N) ** 0.5

	p_low = p - 1.95996 * std_err
	p_high = p + 1.95996 * std_err

	elo_low = 400 * math.log10(p_low / (1 - p_low))
	elo_high = 400 * math.log10(p_high / (1 - p_high))

	width = elo_high - elo_low
	pm = width / 2

	return pm


def main():
	score_percentage: list[float] = [(x[0] + 0.5 * x[2]) / sum(x) for x in wld]

	delta_elo: list[float] = [400 * math.log10(p / (1 - p)) for p in score_percentage]

	estimated_elo = [sum(x) for x in zip(input_engine_elo, delta_elo)]

	elo_error = [calc_error(wld[idx], p) for idx, p in enumerate(score_percentage)]

	# Calc elo POOLED

	weights: list[float] = [1 / (err ** 2) for err in elo_error]

	pooled_elo = sum(val[0] * val[1] for val in zip(weights, estimated_elo)) / sum(weights)
	pooled_error = 1 / (sum(weights) ** 0.5)

	print(f"{pooled_elo} +/- {pooled_error}")

if __name__ == "__main__":
	main()
```

### Comments about other Engines

#### DLComp2

When running the games, I noticed a lot of games ending with illegal moves, basically all
`a1a1` (moving the piece on a1 to a1). Based on the results of these games, I bet it's ELO rating would be much higher.
It made 276 illegal moves, resulting in a loss, out of 1000 games. Assuming that issue doesn't exist, and giving
`DLComp2` those additional games as wins (I don't know that the actual result would have been); `DLComp2` would be rated
about 158 (+/- 24) ELO points higher than it currently is. But, I cannot assume and will just use the scores I have.