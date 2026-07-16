# EEternal Chess

---

A soon-to-be UCI-compatible chess engine, rewritten from my [C++
implementation](https://github.com/ECantDo/EEternal-Chess).

The C++ implementation is roughly around 2900 ELO on a single thread, based
on a private Swiss tournament, provided by Qiles Corey, from the 
[Stockfish Discord](https://discord.gg/GWDRS3kU6R) (roughly 360 engines played,
some of the top engines were included). My current goal is to get
the engine back to where I left the C++ engine, then get to 3000+ ELO.

---

### Features

- Can print `Hello World`.
- Can compile

---

### Why the rewrite?

The C++ version's multithreading was bolted on early without much thought
and became painful to extend. This version aims to get the architecture
right, from the start. ... Ok there was thought behind adding it, but this was
my first proper chess engine, so I also didn't know what I was doing.