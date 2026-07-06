# Product

## Register

product

## Users

MOONMOON viewers catching up on streams they missed.
They arrive knowing roughly what they want ("last night's stream", "the Elden Ring run", "what was he playing in March"), find it, and then spend hours in the player.
Sessions are long and lean-back; discovery is quick and purposeful.
A large share of catch-up viewing happens on phones (in bed, commuting), where the player page is effectively the whole product.

## Product Purpose

An unofficial community archive for browsing and watching MOONMOON's Twitch VODs: browse by game, date, or calendar; watch with synced chat replay and emotes; resume across devices.
Success is a viewer finding the right VOD in seconds and the player getting out of the way for the next three hours.

## Brand Personality

Broadcast-tech, focused, utilitarian.
Twitch-adjacent dark UI with a purple accent; crisp control-room energy from the display font (Chakra Petch), but the stream is always the star.
Chrome earns its place or disappears.

## Anti-references

- Generic SaaS dashboard styling: hero metrics, card grids for everything, gradient decor.
- Cluttered fansite energy: no meme wallpaper, no busy sidebars competing with the player.
- Streaming-platform bloat: no autoplaying previews, no engagement chrome between the viewer and the VOD.

## Design Principles

- **The player is the product.** On the watch page every other element is subordinate to video + chat; controls collapse before content does.
- **Fast to the VOD.** Every surface exists to shorten the path from "I want to watch X" to playback at the right timestamp.
- **Server-rendered calm.** htmx partial swaps, no spinners-as-personality; the UI feels instant and stays still.
- **One system, two themes.** All styling flows through the token set in base.css; dark is the native mood, light must stay first-class.

## Accessibility & Inclusion

No formal target.
Keep the existing good habits where they're free: reduced-motion media queries, keyboard-reachable controls, aria-live on player/chat status.
