# Logo prompts for Clipboard AI

Two prompts for an AI image / design model (e.g. Claude with image generation,
Midjourney, or a designer). Use **Prompt 1** first to get ten concepts, pick one,
then use **Prompt 2** with your chosen concept to produce every asset the app needs.

The app: a Windows tray utility. You select text anywhere, press a hotkey, and an
AI transforms it in place - pasted back over your selection. Tray-resident,
keyboard-driven, developer-ish audience. Brand palette already in the app: a teal
primary (oklch(0.511 0.096 186.391), roughly #1f8a7a / a muted deep teal), cool
neutral greys, on both light and dark. Font in the app is Roboto.

---

## Prompt 1 - concept exploration (run this first)

> Design **10 distinct logo concepts** for a desktop app called **Clipboard AI**.
>
> What the app does, so the mark can mean something: the user selects text in any
> application, presses a keyboard shortcut, and an AI rewrites or transforms that
> text and pastes the result back **in place, over the original selection**. It
> lives in the system tray, it is keyboard-first, and its whole personality is
> "quiet, fast, trustworthy tool" - not a flashy consumer chatbot.
>
> Give me ten genuinely different directions, not ten variations of one idea.
> Explore at least these angles across the ten:
> - the **clipboard** as the obvious base object, but treated in a fresh way
> - the **transform / replace-in-place** action (something becoming something else,
>   an arrow, a swap, a before/after)
> - the **cursor / text selection** (an I-beam, a highlighted run of text, a caret)
> - the **hotkey** idea (a key cap, a modifier glyph)
> - a **spark / intelligence** cue for the AI, used with restraint - no cliche robot
>   heads, no generic sparkles overload
> - at least one **pure wordmark / monogram** direction (the letters "C" + "A", or
>   "Clipboard AI" set cleverly)
> - at least one **abstract mark** that doesn't literally draw a clipboard
>
> Constraints for every concept:
> - Must read as an **app icon**: works inside a squircle/rounded-square, strong
>   silhouette, recognisable at **16x16 px** (the tray) as well as large.
> - **Flat, modern, geometric.** Minimal detail. No gradients-as-a-crutch, no long
>   drop shadows, no photorealism.
> - Palette: a **teal primary** (around a muted deep teal, ~#1f8a7a) with cool
>   neutral greys; must also work in a single flat color (for a monochrome tray
>   icon) and hold up on both light and dark backgrounds.
> - Timeless over trendy; it should look right in three years.
>
> For each of the ten, show the mark on a neutral background and give it a
> one-line name plus one sentence on the idea. Lay them out as a labelled 2x5 or
> 5x2 grid so I can compare them side by side. Keep them all in the same visual
> weight so the comparison is fair.

---

## Prompt 2 - production export (run this after choosing a concept)

Replace the bracketed part with the concept you picked from Prompt 1 (describe it in
a sentence, or paste the winning tile).

> I've chosen this logo concept for **Clipboard AI**: **[describe the chosen concept
> in one or two sentences - e.g. "concept 4: an I-beam text cursor forming the
> stem of a clipboard clip, teal on transparent"]**.
>
> Finalise it and produce the complete icon set an app actually needs. Keep the mark
> **identical** across every export - same geometry, same proportions, only size and
> format change.
>
> **Master:**
> - A clean vector **SVG**, on a transparent background, drawn on a square canvas
>   with even optical padding (the mark should not touch the edges).
> - The same mark as a **flat single-color (monochrome) SVG**, for the tray icon -
>   it must stay legible when everything is one color.
>
> **Windows app (Tauri) - the sizes this project consumes:**
> - PNG, transparent, square, exact pixel sizes: **32x32, 128x128, 256x256
>   (name it 128x128@2x), 512x512**.
> - A Windows **.ico** containing 16, 24, 32, 48, 64, 128, 256 px layers.
> - Tray icon: a **16x16** and **32x32** PNG optimised to still read at tray size -
>   simplify detail if needed at 16px, but keep the silhouette.
> - (If macOS is ever targeted: a 1024x1024 master PNG for the .icns; not needed for
>   Windows-only today.)
>
> **Store / social (for the README and a future release page):**
> - A **512x512** and **1024x1024** PNG of the icon on both a light and a dark
>   background (so I can pick per context).
> - One **wide banner**, ~1280x640, with the mark plus the wordmark "Clipboard AI"
>   set in a clean geometric sans (Roboto or similar), for the GitHub social preview.
>
> **Rules:**
> - Every raster export at **exact** pixel dimensions, transparent unless a
>   background is specified above, crisp edges (no blurry upscaling).
> - Deliver the small sizes (16, 32) checked at 100% - if the full-detail mark turns
>   to mush at 16px, give me a deliberately simplified 16px variant and say so.
> - Keep the teal primary consistent across all of them (give me the exact hex you
>   used so I can match it in the app).
