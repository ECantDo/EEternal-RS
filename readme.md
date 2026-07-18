# EEternal Chess

---

A _mostly_ UCI-compatible chess engine, rewritten from my [C++
implementation](https://github.com/ECantDo/EEternal-Chess); the engine can make reasonable moves, it does have a
fixed search depth, and a terrible evaluation, but it is making decisions.

The C++ implementation is roughly around 2900 ELO on a single thread, based
on a private Swiss tournament, provided by Qiles Corey, from the
[Stockfish Discord](https://discord.gg/GWDRS3kU6R) (roughly 360 engines played,
some of the top engines were included). My current goal is to get
the engine back to where I left the C++ engine, then get to 3000+ ELO.

---

## Features

- Can no longer print "Hello, world!" :(
- Can tell you its name and author
- Can make moves
- No engine logs (Not really a good thing, but you might like it like that)

### Evaluation

This engine keeps a running total of the piece values (`white - black`),
and uses this as the sole evaluation function.

### Search

This engine always searches to a fixed depth of 5 ply. This is because I just
wanted to get something working, before making it work good. I also think
that it's funny. It's search speed is good enough, for depth 5 (at least on my
computer) to almost never take more than a second to evaluate the position.

The search uses NegaMax with alpha beta pruning.

### UCI

The following UCI commands are implemented:

| Command    | Description                                    |
|------------|------------------------------------------------|
| uci        | Tell the engine to use the UCI protocol        |
| isready    | Confirms the engine is ready                   |
| ucinewgame | Starts a new game                              |
| position   | Sets up the position                           |
| go         | Just `go`, no depth, time, or nodes            |
| go perft   | Runs (non-bulk) perft for the current position |
| quit       | Stops the engine                               |
| d          | Display the current board state                |

---

## Why the rewrite?

The C++ version's multithreading was bolted on early without much thought
and became painful to extend. This version aims to get the architecture
right, from the start. ... Ok there was thought behind adding it, but this was
my first proper chess engine, so I also didn't know what I was doing.

---

## Approx ELO

The ELO has been calculated based on games
using [Sebastian Lague's Chess Challenge](https://github.com/SebLague/Chess-Challenge),
[Ratings List](https://github.com/SebLague/Tiny-Chess-Bot-Challenge-Results/blob/main/RatingsList.txt)
of the Swiss and Knockout games played.

I have downloaded some bots in the surrounding area where my engine is roughly located,
and played games against those bots.

| Version Number | Approx ELO  | Version Description                             | VS                                  | VS Elo      |
|----------------|-------------|-------------------------------------------------|-------------------------------------|-------------|
| 0.0.2          | 1098 +/- 12 | Fixed search depth of 5. Basic search algorithm | Turochamp (Faithful) (by: P Rivero) | 1026 +/- 81 |  
| 0.0.1          | 1           | Makes random moves very quickly                 | N/A                                 | N/A         |
