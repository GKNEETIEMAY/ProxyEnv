# README illustrations

The four PNGs are explanatory illustrations, not application screenshots or
real-device acceptance evidence. Both language editions share the same artwork
and provide localized descriptions and alternative text. Short in-image labels
are English; the Chinese README explains the same relationships in Chinese.

- [Workflow](routespan-workflow.png): scattered network/AI/Skills setup versus a
  selected remote workspace. Checkmarks illustrate the intended workflow, not a
  guarantee that an upstream provider or remote network is available.
- [Architecture](routespan-architecture.png): local capabilities extended to a
  remote Linux account. Network and AI use SSH reverse forwarding; selected Skills
  use a separate file-synchronization path. Arrows show capability flow, not the
  direction of individual requests or SSH establishment.
- [Application launch](routespan-app-launch.png): choose a proxy or direct
  environment for a new process. Existing applications remain unchanged; launching
  does not establish successful network access.
- [Tool readiness](routespan-tool-readiness.png): installation discovery, prepared
  route settings and verified requests are separate facts. CLI and plugins are
  discovered independently; schematic checks are not real acceptance evidence.

## Presentation contract

This README extension uses **Read** mode: the illustrations explain the workflow
and capability paths alongside the existing product identity.

- Pair line-art people and computers in the workflow with a matching network
  architecture drawing. Keep the added illustrations on pure white, with round-headed
  line-art people, charcoal strokes and low-saturation
  orange, blue and teal accents.
- Center semantic HTML images at a reading-width ceiling of 880px
  (`width="880"`, constrained by the renderer's responsive image sizing). Link each
  image to its full-size PNG so small labels remain available on narrow screens.
- Share short English artwork labels across both editions; localize the adjoining
  prose, image alt text and Windows/MIT badge alt text. Both READMEs carry the same
  Windows platform and MIT license badges.
- Keep conceptual meaning explicit: workflow checks describe intended setup;
  architecture arrows describe capability flow. Show network/AI reverse forwarding
  separately from selected Skills file synchronization, as explained above.

Generated with the built-in image generation tool on 2026-10-12. Charcoal line
art, warm white ground and restrained orange/blue/teal accents follow the requested
reference. These are documentation assets only. They must not replace the
user-supplied RouteSpan brand mark or native icons.

The two added illustrations were edited against `routespan-workflow.png` after
visual review: the final style removes broad color washes and cartoon clothing,
returning to the original white-background line art. Images remain illustrative;
the adjoining localized prose supplies the behavior boundaries.

## Final generation prompts

### Application launch (initial generation)

Use case: illustration-story. Asset type: second set of explanatory GitHub README illustrations for RouteSpan. Draw a refined wide hand-drawn editorial line-art infographic, roughly 2.4:1, warm white background, smooth charcoal strokes with pale terracotta, sky-blue and teal accent washes. Keep the same friendly stick-figure developers at desks and substantial monitor scale as a clean software explainer. Topic is LOCAL application launch choice, not remote SSH. At left a friendly developer chooses a small application window beside a desktop monitor, short label exactly "Choose an app". In the center a compact terminal/utility window titled exactly "RouteSpan". Two unambiguous branches lead to two new application windows at right, arranged side by side or vertically with balanced room: upper branch labeled exactly "Proxy launch" passes through a small globe with label "Active proxy"; lower branch labeled exactly "Direct launch" bypasses that globe entirely. Each newly created window shows only a simple terminal prompt and has the label "New process". At the bottom left a small existing app window stays gray and undisturbed, with exact explanatory text "Running apps stay unchanged". No arrow should imply injecting settings into this existing app. Large bottom summary exactly "Choose the route for a new app". All wording large and readable at README width 880px. No green success checks for actual internet access, no guaranteed connectivity, no system proxy/TUN toggles, no VPN, no product logos, no invented RouteSpan emblem, no metrics. No screenshot, no dashboard, no watermarks, no outer frame. This is a conceptual choice diagram, not a diagram of confirmed requests.

### Tool readiness (initial generation)

Use case: infographic-diagram. Asset type: second set of explanatory GitHub README illustrations for RouteSpan. Refined wide editorial line-art, about 2.4:1; warm white background, smooth charcoal strokes and pale terracotta/sky-blue/teal washes. Friendly developer and simple terminal/extension drawings. Topic: installed, configured and verified are different facts, not one universal success state. Three well-aligned stations from left to right. Station ONE large exact label "Installed"; simple terminal icon and separate editor-plugin icon, two small green checks indicating illustrative detection. Short sublabel exactly "CLI / VS Code plugin". Station TWO large label "Configured"; document with settings lines and a single small green check indicating illustrative configuration prepared. Short sublabel exactly "Route settings". Station THREE large label "Verified"; friendly developer at monitor with terminal prompt and a hollow gray status circle; sublabel exactly "Not tested" to demonstrate that installation/configuration do not establish a real request. A small pointer hand / conversation icon below with label exactly "Use the real client". Use thin dashed arrows as conceptual progression, not auto-execution arrows. Large bottom summary exactly "Setup is not a successful request". Checks are schematic illustrative states only, not real product test results. Keep all text concise, readable, no fine print. Do NOT say all models are auto-tested, do not imply paid AI requests happen automatically, do not imply CLI is required for a compatible plugin-only setup. No locks/root isolation/no model credentials, no third-party logos or invented RouteSpan emblem. No actual UI screenshot, no dashboard, no metrics, no frame or watermark. Same polished gentle hand-drawn style as the earlier developer and desktop illustrations.

### Final style correction for both additions

Edit target: each newly generated illustration. Style reference:
`routespan-workflow.png`. The final PNGs above use this corrected white style.

STYLE CORRECTION, not a new content design. Image 1 is the edit target. Image 2 is the authoritative style reference: match it closely. Preserve Image 1's exact words, diagram topology, labels, subject positions, whitespace and wide landscape ratio. Repaint the complete background PURE WHITE (#ffffff), not cream or paper-colored. Remove ALL broad watercolor strokes, colored header backgrounds, shaded colored clothing, colored monitor backplates, underlines and decorative broad background blobs. Header words should sit directly on white, crisp humanist sans-serif rather than large brush calligraphy. Change the developer into the reference's simple ROUND-HEADED HAIRLESS STICK FIGURE: white circle face, two small black oval eyes, simple smile/thoughtful mouth, thin charcoal line body and arms; no hair, no skin coloring, no solid shirt, no realistic cartoon face. Match the reference's smooth, elegant charcoal outline weight and white object interiors. Retain only small muted pastel-blue/orange/teal touches on connectors, status circles, window control dots and tiny icons, at the reference's low saturation. Dark terminal screens may stay as in the reference. Remove doodle rays around the large bottom sentence; keep the sentence itself clean. No new frames, decorations, props, logos or claims. The target must feel like it belongs beside Image 2 on one clean white README page.

### Workflow

Use case: illustration-story. Asset type: final wide GitHub README illustration for RouteSpan, not a screenshot. Draw a polished minimalist hand-drawn editorial line-art scene on clean warm white, aspect about 2.4:1, dark charcoal consistent smooth strokes, tiny pale terracotta, sky blue and teal highlights, no gradients or heavy shadows. Two side-by-side developer-at-desk scenes, left BEFORE, right WITH ROUTESPAN. Left: small friendly stick-figure developer looks puzzled at monitor with three separate rows labeled "Network", "AI route", "Skills"; a faded broken connector and small amber question marks indicate mismatched or missing setup. Right: same developer contentedly working, monitor repeats the same three rows with neat green checkmarks; small separate switches indicate chosen capabilities, not an all-or-nothing global network switch. Connecting the scenes at the middle: a small clean terminal window labeled exactly "RouteSpan", with simple bridging linework. Heading labels exactly "Scattered setup" at left and "One remote workspace" at right in large readable humanist sans letters. Use names only, no product logos, no OpenAI logo, no Claude logo, no made-up RouteSpan emblem. The illustration expresses reduced repeated setup, NOT guaranteed internet/model success. No claims about metrics, automatic installation, root isolation or zero configuration. Breathing room but keep subjects substantial at README width 880px. No other text. No outer frame, no UI dashboard, no watermark.

### Architecture

Use case: infographic-diagram. Asset type: final wide GitHub README architecture illustration for RouteSpan. Same polished hand-drawn editorial illustration style: clean warm white background, charcoal linework, pale terracotta local highlights, sky blue bridge highlights, teal AI highlights; simple monitor and server drawings, precise readable short English labels, no gradients or heavy shadows. Landscape about 2.3:1. Three columns with accurate explicit connectors. Left a desktop monitor titled "Windows PC" with three separated sources labeled exactly "Local proxy", "CC Switch AI route", "Managed Skills". Center a small desktop utility/terminal drawing labeled "RouteSpan". Right a server and terminal drawing titled "Remote Linux account", containing exact labels "Terminal", "VS Code Remote", "Codex", "Claude Code". Important: show TWO horizontal arrows from center to remote: one blue labeled "SSH reverse forwarding" for network and AI route; a second fine teal arrow BELOW it labeled "Skills sync" for selected managed files. RouteSpan takes the three selected local inputs; CC Switch remains the owner of provider routing, RouteSpan does not replace it. All sources point to RouteSpan; capability flow then goes to the remote account. Diagram is a capability-flow illustration, not literal SSH initiation or TCP packet-direction diagram. Small caption under center exactly "Choose network, AI, or both". No product logos/emblems, no invented logo, no cloud-provider brands, no locks or claims of per-user isolation, no blanket VPN tunnel covering all local files/network, no automatic installer, no provider secrets traveling to remote. Keep text large enough to read at 880px total width, align labels and connectors, ample whitespace, no crossed arrows, no outer frame, no watermark.
