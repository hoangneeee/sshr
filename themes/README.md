# sshr themes

Community-maintained color themes for [sshr](../README.md). Each `.toml` file in this folder is a ready-to-paste snippet for `~/.config/sshr/sshr.toml`.

## Available themes

| Theme         | File                                   | Source                                              |
| ------------- | -------------------------------------- | --------------------------------------------------- |
| Dracula       | [dracula.toml](./dracula.toml)         | https://draculatheme.com                            |
| Tokyo Night   | [tokyo-night.toml](./tokyo-night.toml) | https://github.com/folke/tokyonight.nvim            |
| Nord          | [nord.toml](./nord.toml)               | https://www.nordtheme.com                           |

## Using a theme

1. Open the `.toml` file you want and copy the entire `[[themes]]` block.
2. Paste it into `~/.config/sshr/sshr.toml` alongside any existing themes.
3. Set the active theme at the top of the file:

   ```toml
   default_theme = "dracula"
   ```

4. Restart `sshr`.

If `default_theme` doesn't match any theme name in the list, sshr falls back to the first one defined.

## Color roles

A theme defines 8 color roles. Colors are 6-digit hex strings; the leading `#` is optional. Invalid or missing values fall back to built-in defaults.

| Role         | Used for                                      |
| ------------ | --------------------------------------------- |
| `primary`    | Active panel border, selected row, success    |
| `secondary`  | Info text, secondary labels                   |
| `highlight`  | Search prompt, accent highlights              |
| `text`       | Normal text                                   |
| `error`      | Errors, fuzzy-match character highlight       |
| `warning`    | Reserved for future use                       |
| `success`    | Success status messages                       |
| `background` | Painted across the whole TUI                  |

> Tip: pick `primary` and `highlight` from the brighter end of your palette so selection and search stay visible against a dark terminal. Avoid colors close to your terminal background — they'll look invisible.

## Contributing a new theme

1. **Fork & branch** the repo and create `themes/<your-theme-name>.toml`. Use lowercase, hyphen-separated filenames (e.g. `gruvbox-dark.toml`, `catppuccin-mocha.toml`).
2. **Follow the file template** below — header comment with theme name and source URL, then a single `[[themes]]` block.
3. **Pick colors thoughtfully:**
   - `primary` — usually the theme's signature green, blue, or accent color
   - `secondary` — a softer accent, often cyan or muted blue
   - `highlight` — a contrasting yellow/orange for search
   - `error` — the theme's red
   - `success` — usually equals `primary` for green-tinted themes
   - `text` — the theme's default foreground
   - `background` — the theme's default background; sshr paints this across the full TUI. Omit (or use a transparent value) to inherit the terminal's background.
   - `warning` — reserved; use the theme's orange/amber
4. **Test it:** copy the snippet into your local `~/.config/sshr/sshr.toml`, set `default_theme`, and run `sshr`. Verify selected rows, search prompt, and status messages are all readable.
5. **Update this README:** add a row to the [Available themes](#available-themes) table.
6. **Open a PR** with a screenshot of the TUI under the new theme.

### File template

```toml
# <Theme Name> — <source URL>
# Paste this block into ~/.config/sshr/sshr.toml under your themes list,
# then set `default_theme = "<theme-name>"`.

[[themes]]
name = "<theme-name>"

[themes.colors]
primary    = "#......"
secondary  = "#......"
background = "#......"
text       = "#......"
highlight  = "#......"
error      = "#......"
warning    = "#......"
success    = "#......"
```

The `name` field must match the filename (without `.toml`) and the value users put in `default_theme`.
