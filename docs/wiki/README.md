# AI Town — repo wiki

Developer-facing documentation for this repository. Player-facing text stays in the root `README.md`; setup commands stay in `docs/README.md`. When a page and the code disagree, the code wins — fix the page.

| Page | Read it when |
| --- | --- |
| [Developer notes](developer-notes.md) | First day: what this is, how to run and check it, house rules, traps |
| [Architecture](architecture.md) | You need the whole picture: processes, ports, one tick end to end |
| [Kernel](kernel.md) | Changing world rules, time, actions, saves |
| [Game modes](game-modes.md) | Adding or changing a mode (大逃杀, 狼人杀, 执剑人之夜, storyline) |
| [Agent runtime and memory](agent.md) | Changing what residents see, remember, or how they call the model |
| [Protocol and host](protocol-and-host.md) | Adding a message, a command, or anything on the wire |
| [Managers and storage](managers-and-storage.md) | Adding an `/api/towns` endpoint or touching files on disk |
| [Frontend](frontend.md) | Changing pages, prefabs, setup screens, looks, music |

Work in progress is planned and reported in [`Sprints/`](../../Sprints/README.md).
