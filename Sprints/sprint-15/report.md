# Sprint 15 — report

Requested by the owner: prompt tuning in settings, because character and story generation were not good enough — a professional mode (prompt editing), a style mode (modular templates: background, role, style, example, no-gos, rubrics), and test-and-compare for any input; in the style of the game.

## Shipped

**「提示词」 in settings** — on the launcher and in the game. One dialog for the two generation prompts: 写故事 (the story editor) and 生成人物 (the resident library).

| Mode | What the player does | What is sent |
| --- | --- | --- |
| 默认 | nothing | the shipped prompt, byte for byte as before |
| 风格模式 | picks a preset, edits six modules: 身份, 背景, 文风, 例子, 不要, 自查 | the modules as labelled blocks between the task description and the output contract; the role replaces the first sentence. The contract is untouched, so replies still parse |
| 专业模式 | edits the whole template, placeholders listed beside it | that template; a warning if it lost the JSON contract or a placeholder |

- **Presets:** six for stories (讲究一点, 古龙式武侠, 本格推理, 温情日常, 权谋群像, 荒诞喜剧), five for characters (立得住的人, 江湖人物, 有秘密的人, 街坊邻居, 怪人). Each fills all the modules and is only a starting point.
- **试一试 · 比一比:** one input, two prompts (default, the one being edited, the saved one, or any preset), both run at once, results side by side with time and model; 「就用这一份」 adopts a side.
- **Where it applies:** the story editor and the in-game character generator use the saved tuning from the next call on. Settings are application-wide (`prompts.json` in the user's config folder).
- **Both managers:** `GET/POST /api/towns/prompts`, `POST /prompts/preview`, `POST /prompts/character`; `/story` accepts `prompt` for a trial.

## Tests

| Check | Result |
| --- | --- |
| `cargo clippy --workspace --all-targets -D warnings` | clean |
| `cargo test --workspace` | pass — 160 tests (+1) |
| `pnpm build`, `typecheck`, `format:check` | pass |
| `pnpm test` | 22 pass, 1 known root-only failure |

- **Default unchanged:** the story prompt (with and without rules) and the character prompt were dumped before and after the change and compared byte for byte — identical.
- **New unit test** (`tuning.rs`): default equals the shipped template; style inserts only the filled blocks, in order, before the contract, and an empty style equals the default; professional uses the given text, falls back when blank, and warns when the contract or placeholders are missing; over-long text is cut.
- **Manager tests** (Node and Rust): read defaults, save a style tuning, preview shows the block and the contract, a bare professional template warns, `/story` with `prompt` still returns a draft, `/prompts/character` answers.

## Real-model comparison (DeepSeek `deepseek-flash`)

**Story — the owner's wuxia outline, six characters, default vs. 古龙式武侠**, run from the dialog:

| | 默认 (67 s) | 古龙式武侠 (79 s) |
| --- | --- | --- |
| Title | 丁香一日局 | 茶凉丁香未开 |
| A character | 「病弱纤瘦却男装冷绝的紫丁香大护法，外号金大少。」 | 「金大少，男装薄肩，咳嗽时用白帕掩口，帕角绣一朵紫丁香。」 |
| A secret | 「她怀疑总盟主与金玉侯府有关，更怕答案会让自己下不了手。」 | 「剑柄缠着程小青旧年刀穗；卜鹰已猜到一半，她宁死不肯认。」 |
| A goal | 「查清总盟主缺席真相，并逼程小青先拔刀。」 | 「今日在渡口截住程小青，问清一桩旧案后再决定杀不杀。」 |
| A beat | 「清晨，暮春茶棚里竹筷钉死吕老先生，肥肉与猪脚乱飞……」 | 「茶凉了。青竹茶棚里坐满人，银雕的位子还空着。」 |

The default retells the outline's events as beats. The styled draft gives every character an object or habit, ties each secret to someone who has half-guessed it, makes goals things to do today, and writes beats around who is absent — which is what the preset's rubrics ask for. Both drafts validated.

**Character — "镖局里管账的中年人，年轻时走过镖", default vs. 江湖人物:** the default produced a tidy, generic bookkeeper; the preset produced a man who lost two fingers on a job he never mentions, raps the table three times, and still owes the old bookkeeper money.

One run of each; no claim beyond these samples.

## Not done / limits

- **No comparison of more than two prompts at once, and no saved history of trials.**
- **Only the two generation prompts are tunable.** The residents' own decision prompt, the narrator and the dream prompt are not.
- **A professional template that breaks the output format** makes generation fail with the usual "模型写的故事用不了" message; the dialog warns but does not stop it.
- **The default prompts were left as they are.** The 讲究一点 preset is the suggested upgrade for stories; making it the default is a product decision.
- **Presets live in the page**, not in user data: a player's own set of modules is one saved tuning per prompt, not a library of named templates.
- **Browser check** was done in desktop Chromium; the dialog has a narrow layout but was not exercised on a phone or in the app shells.
