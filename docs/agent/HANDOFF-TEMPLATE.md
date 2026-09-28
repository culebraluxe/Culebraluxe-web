# Handoff — the shape an agent writes when it stops

A session that ends with "I am full" must leave **this file, pushed**, not a chat log and not a summary of intent. The
next agent reads it once and can act. Copy this file to `docs/agent/HANDOFF-<topic>-<date>.md`, fill every section, delete
the italics. Short beats complete: a row per fact, a line per action.

**The rule behind the shape.** State what is TRUE (with the file or the command that proves it), what is HELD (with whose
call it is), what LANDED (with the commit), what is OPEN (in order), what is NOT VERIFIED (honestly), and what the next
agent must not touch. Anything that is none of those is prose, and prose is what the last bad handoff was made of.

## 1. STATUS — what is true right now

| # | Fact | How to check it |
| --- | --- | --- |
| S1 | *one load-bearing fact* | *`path:line`, or the command that shows it* |

*One row per fact. A fact with no path, line or command behind it goes in §5 instead.*

## 2. HOLDS — do not act on these

| # | Held | Who holds it | What an agent must do |
| --- | --- | --- | --- |
| H1 | *the thing* | *the owner / another agent, by name* | *the one sentence to obey* |

*Every hold names its owner. "Marketing is held for redesign" is a hold; "marketing looks unfinished" is gossip.*

## 3. WHERE TO LOOK — task → the one place

| Your task | Read | The files you touch |
| --- | --- | --- |
| *what the next agent might be asked to do* | *the doc section* | *the exact files* |

## 4. DONE — what landed, with the receipts

| Commit | What it changed | The gate that ran |
| --- | --- | --- |
| *sha on `origin/main`* | *one line* | *the exact command, and its result* |

*"Done" means pushed and its gate ran. A gate that was NOT run is a §5 line, never a §4 row.*

## 5. NOT VERIFIED — the honest gaps

- *What you did not run, could not reach, measured once, or only read from a doc.*

## 6. OPEN — the next actions, in order

1. *The action, the file to open first, and how the next agent knows it is finished.*
2. *Repeat. If an action is really a decision for the owner, it belongs in §7, not here.*

## 7. ASK THE OWNER

- *Each ask as a question answerable in one word, plus what happens on each answer.*
