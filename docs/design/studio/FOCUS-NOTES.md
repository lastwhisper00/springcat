# Concept 09 · Type

## Scope and authority

The user requested a more comfortable typeface and font sizes, and full-row translucent hover feedback instead of changing text color. This refinement adopts self-hosted Noto Sans SC, increases task titles and supporting text, and limits list hover/pressed feedback to the background. The source-logo ball, execution rules, task order, paging, colors, and 520px expanded width remain. Production Svelte/Tauri code is outside this change.

## Typography and row feedback

- Load the complete Noto Sans SC variable TTF from `assets/fonts/NotoSansSC-VF.ttf` through `fonts.css`, under the family `SpringCat Sans`. Use real 100–900 weights, no synthetic bold, and normal letter spacing. The font's SIL Open Font License is included with the assets.
- Task titles use 15px / 500 with 22px line height. Metadata and state labels use 13px / 400 with 20px line height. Keep 2px between lines and 10px vertical padding, giving a 64px row. Horizontal padding is 10px on both sides.
- Main header controls, list heading and footer controls use 13px. Detail titles use 20px / 500 with normal tracking. Every task status uses the same type treatment.
- Hover adds a translucent background to the entire task row: white at 6% in dark themes, deep neutral #141C26 at 4% in the light theme. Pressed uses 9% and 7%. Title, metadata, source icon and status colors do not change.
- Only the row background transitions, for 120ms. Reduced motion switches immediately. Keyboard focus retains its explicit focus ring.
- The design viewer loads `fonts.css`, so inline artboards share the self-hosted font. Standalone SVGs declare `SpringCat Sans`, then `Noto Sans SC`; they do not embed the large font file. Install Noto Sans SC when opening/importing standalone SVGs to preserve type metrics.
- The typography artboard includes identical default and hovered task rows to show that only the background changes. Other artboards remain in their default state.

## Agent identity and execution contract

- Expanded headers and summary capsules use a 28px neutral solid ball with a 16px source logo. A resting top capsule uses 22px / 13px. A resting side ball uses 28px / 16px inside the existing 36px window.
- Use a light logo on dark themes and a dark logo on the light theme. Keep the center neutral; only the outer ring carries the execution glow.
- The default mixed example has one running Cursor task, so the main ball displays Cursor's actual SVG logo.
- Count tasks and distinct sources across the entire collection, including other pages. “N 项进行中” is the task count. A corner “+N” is the count of other running sources. Multiple tasks from one source produce one logo and no extra-source badge.
- Preserve the current representative source while it still has running tasks. Switch only when it stops, to the next running source. Do not rotate logos automatically.
- Source names and the additional-source count are available through a tooltip and readable description. Static artboards illustrate the badge; the interactive prototype owns its tooltip behavior.
- With no execution, retain the previous source logo and turn off the outer ring. On initial load, the most recent task source may provide this identity. Keep “暂无进行中” beside it.
- Waiting, completion and failure do not light the execution ring. Footer green still means connected.
- The 6px resting notch edge can fit a 4px light indicator only. Hovering or expanding reveals the full logo; the logo is never compressed into that dot.
- No blinking, breathing, or automatic logo cycling is introduced. Reduced-motion mode keeps the static logo and ring state.

## Task list contract

1. Use the neutral heading “最近任务” and a quiet total count.
2. Every task has the same source-logo position, title size and weight, source/time line, row height, and background. The right edge shows a 13px state label and a 4px colored dot.
3. “待确认”, “进行中”, “已完成” and “失败” are peer statuses. Pending confirmation never creates a larger card, banner or separate action area.
4. Each pending task occupies one row. The default five-task example contains three waiting tasks interleaved with a running task and a completed task.
5. Sort by recent update when opening or refreshing. While reading, update a task in place and preserve its position, current page, and focused element.
6. Show at most five tasks per page, with explicit previous/next controls and total/range when more exist. Reducing page size on low-height native screens is an integration requirement, not behavior already implemented by this browser prototype. “全部任务” opens the full workspace.
7. Clicking a row opens that task’s existing detail. Request explanation and source actions belong in this detail. Reading or marking a task as read does not resolve a waiting status.

## Visual decisions

- The agent-logo ball and task count share a quiet header row with the three navigation choices. The floating header no longer repeats the brand name.
- The list heading uses quiet 13px / 500 text and all task titles use 15px / 500; source, time, count, and state use 13px / 400. Detail titles use 20px / 500.
- All task rows are 64px high; longer content and narrow layouts must preserve readable text and accessible detail access.
- Row status text uses the same quiet text color; only the small state indicator varies by status.
- There are no per-row action buttons, nested cards, strong separators, or special completed-title dimming.
- “标记已读” remains a secondary text control. Source logos and existing lightweight SVG action icons are reused.
- Unpaginated panels follow their content height (470px for five tasks). Paginated lists preserve a 320px minimum list height, keeping both pages at 506px total height. There is no nested scrolling. Existing themes and motion rules remain unchanged.

## Skill application

Impeccable's Operate and Craft Floor rules support consistent controls, quiet semantic color, and predictable scanning. The existing design context permits scoped refinement without a new product interview. Karpathy Guidelines keep the change limited to the task list, its data examples, and the corresponding design artifacts. No new framework or runtime dependency is required.

## Verification record for this revision

The self-hosted font loaded successfully: `document.fonts.check('500 15px "SpringCat Sans"')` returned true. The complete TTF is 17,773,244 bytes and its variable weight axis covers 100–900. Task titles measured 15px / 500 / 22px, metadata 13px / 20px, and every task row 64px.

At a 390px viewport the panel measured 323px, with zero horizontal or vertical overflow in the header and list. Sample titles remained fully visible and state labels stayed at 13px. Actual pointer hover in the light theme applied `rgba(20, 28, 38, .04)` without keyboard focus; title color remained `rgb(32, 37, 44)`, matching an ordinary row. Dark-theme title color likewise stayed `rgb(241, 242, 244)` while its hover background uses white at 6%.

The scoped typography detector and final combined detector each ran once. Findings remained limited to existing outer 11px labels, illustrative background, shell-size transitions, and the user-requested agent-ring glow. After regeneration, all 288 SVGs and three JSON files parsed, and four JavaScript sources passed syntax checks. Independent SVGs deliberately do not embed the font. The final dark-theme pointer check confirmed a 6% white background with identical normal/hover text color and no browser console errors.

Prior revisions remain a regression baseline for unchanged source identity, task state, page/focus restoration and docking.

## Integration limits

These are design prototypes. Real native-window motion, screen-edge constraints, safe areas, reduced-height page sizing, and cross-tool deep links require verification in Tauri. SVG artboards show static states; the prototype provides interaction examples. Reading a preview never grants approval to a source tool.
