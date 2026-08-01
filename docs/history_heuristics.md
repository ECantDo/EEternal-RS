# More about implementing History Heuristics

Here I would like to document a bit more about what I tried with the History Heuristics that would have otherwise been
glossed over when looking at the general
[readme version history](../readme.md#feature-history). This way I can explore more of the how and why I did things a
particular way.

As this is also my first deeper dive document, I want to mention (that I won't be mentioning elsewhere), all the
previous additions have been very robotic and hasn't needed much testing to confirm if the feature works. There isn't
much flexibility with previous features to mess around with, however, that's not to say I will re-visit some of them and
go into a deeper dive.

For example, NegaMax, Iterative Deepening, and Transposition Tables are all mostly in their final form from the get-go,
either that, or I have copied things I know work for the feature from my
[previous engine](https://github.com/ECantDo/EEternal-Chess).

I might also decide that I want to explain more about the engine for a newcomer to engine development, and I want to
give the 3 nuggets of wisdom that I can. I find that there are plenty of good sources online, but most of them need to
delve into existing engines, and figure out how they work. Either that, or it's a how-to guide on YouTube (which are
amazing), but most of the time suck when asking: "Why do I need this?"

I will be the first to admit, I also hate reading big long documents, but this is where I am at for this project, maybe
there will be something cleaner to watch in the future...

---

The first experiment I wanted to try with History Heuristics was clearing the history table at the start of every search
instead of preserving it across searches, which is the more traditional approach.

I first wanted to try this since I am lazy and I didn't want to re-work the UCI code going into search, so here we are.

Initially I made 2 versions, one with some caching, and one without. What's this caching you ask? In a previous version,
I added attack caching, basically, when I make a move, I compute and cache all the attacks from both players. That way,
move gen is *theoretically* faster (it wasn't), and checking if a square is attacked is just a lookup rather than
computing it each time. The reason why it wasn't faster at the time was due to me not utilizing checking if a square was
attacked, enough; meaning the overhead time of computing all attacks was slower than just computing it for each square I
needed, when I needed it (like 1 to 3 per move).

This changed with the addition of this History Heuristic, since on each access to get and update the table, I am putting
more bins in for if the squares we are moving from/to are attacked. So having the cached attack bitboards gains some 1
million nps (in search) over computing each time. Although, weirdly, in my implementations, the non-cached version was
missing a search of some 4 million nodes, from the start position, and I have no idea where they went. For reference,
both searches should have reported roughly the same number of nodes, ~13 million.

```rust
pub struct QuietHistory {
    // [side to move][is from threatened <false/true>][ is to threatened <false/true>][from][to]
    entries: Box<[[[MoveHistory<i16>; 2]; 2]; 2]>,
}
```

Moving forward, I am going to be calling the cached version `a` and the non-cached version `b` with the major version
being `0.0.10`.

Despite having searched 4 million fewer nodes, from the same position, version `b` performed much worse than version
`a`, clearly there is a bug somewhere, though I was never able to identify it. Since both implementations should produce
identical search trees, the differing node counts indicate a bug rather than an algorithmic improvement; so I am not too
fussed about the node count being different, or finding the bug.

```terminaloutput
Score of EEternalRS-0_0_10b vs EEternalRS-0_0_10a: 71 - 178 - 187  [0.377] 436
Elo difference: -87.0 +/- 24.8, LOS: 0.0 %, DrawRatio: 42.9 %
SPRT: llr -2.95 (-100.2%), lbound -2.94, ubound 2.94 - H0 was accepted
```

And looking at version `a` compared to the previous major version, it has done quite nicely.

```terminaloutput
Score of EEternalRS-0_0_10a vs EEternalRS-0_0_9: 257 - 120 - 185  [0.622] 562
Elo difference: 86.4 +/- 23.9, LOS: 100.0 %, DrawRatio: 32.9 %
SPRT: llr 2.96 (100.4%), lbound -2.94, ubound 2.94 - H1 was accepted
```

It should also be noted at this time, there has not been any tuning for the constants as a part of the function. So the
true potential Elo improvement is unknown, but can still give some good insights while using values from thin air, but
seem good.

Next, let's introduce version `c` (0.0.10c), this version has been modified from version `a` so that the History
Heuristic is saved between searches. Everything else is the same. Some initial testing suggests that sharing (at the
very least, the Quiet move) histories across a full game does not pose significant improvement.

```terminaloutput
Score of EEternalRS-0_0_10c vs EEternalRS-0_0_10a: 619 - 584 - 797  [0.509] 2000
Elo difference: 6.1 +/- 11.8, LOS: 84.4 %, DrawRatio: 39.9 %
SPRT: llr 0.493 (16.7%), lbound -2.94, ubound 2.94
```

However, it is still important to note, that the values being used in this test are still *very* suboptimal, so there is
the possibility of cumulative bias. By my current testing, it is showing either no improvement of the engine, or an elo
difference of 1 to 4 points (unknown if better or worse).

The above test does suggest that there is a slight improvement, but take note of the margin of error (+/- 11.8 Elo), vs
the predicted Elo delta (difference; +6.1). The Elo delta is roughly half of the margin of error, so in this case, I
am going to be sticking with version `a`. I will, however, be looking at this again when I put in the effort of tuning
my arbitrary values and get a better signal-to-noise ratio.

... yes I could run more games, but I only have so much CPU time to give, so we are going to leave it for now.