# Forge lanes

Neon `storyboard_story` is the control-plane packet. Git `docs/agent/packets/`
is the architect-readable projection so Grok can lead without Neon.

| Story column / packet heading | Lane |
|---|---|
| Context refs | Scout (DeepSeek) |
| Architect brief | Architect (Grok or human) |
| Goal / scope / acceptance | Smith (DeepSeek) implements, Assay checks |
| `agent_work_item` | Launch envelope after the lane gate |
| `storyboard_story_run` | Evidence |

Grok = judgment-lab (Architect, Inspector). DeepSeek = volume-lab
(Scout, Smith, Assay). Do not register Inspector on `deepseek-harness`.

See `docs/agent/ARCHITECT.md`.
