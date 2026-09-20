# CIL-00 — Council port story stack

Execute in order. Ready is explicit authorization. Do not merge stories.

| ID | Title | Depends | Domain |
|---|---|---|---|
| CIL-01 | Pinned submodule + sync script | — | vendor |
| CIL-02 | Neon schema + db/council.ts | CIL-01 | neon |
| CIL-03 | CouncilService kernel | CIL-02 | services |
| CIL-04 | composeCoreServices + entitlement | CIL-03 | services |
| CIL-05 | Model port (DeepSeek/OpenCode) | CIL-04 | harness |
| CIL-06 | Lead PRE uses verdict | CIL-04 | forge |
| CIL-07 | Architect duo + lint gate | CIL-06 | forge |
| CIL-08 | Assay checkpoint | CIL-06 | forge |
| CIL-09 | Deal / offer desk | CIL-05 | app |
| CIL-10 | Match (person × property) | CIL-05 | app |
| CIL-11 | Listing intake | CIL-05 | app |
| CIL-12 | Portal card | CIL-09 | app ui |
| CIL-13 | Outcome review after 10 | CIL-08, CIL-09 | assay |

CIL-06 can start after CIL-04 with NullModelPort if Lead only needs refuse +
manual recordVerdict in tests. Live duo needs CIL-05.

Do not add `council` to `lanes.ts`. Do not edit FORGE_SDLC XML for this stack.
