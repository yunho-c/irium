# irium-cli

`irium-cli` provides the `irm` terminal UI for interactive, in-memory file rename planning.

## Run

```bash
cargo run --bin irm
```

The app scans the current working directory and opens a three-stage TUI:

1. `Scope`: choose files and filter sets.
2. `Naming`: edit overrides, style options, and preview proposed names.
3. `Apply`: simulate apply and keep session-local undo history.

## Keybindings

### Global

- `q`: quit
- `Ctrl+C`: quit immediately
- `?`: toggle help
- `L`: toggle logs modal
- `[` / `]`: previous / next stage
- `Tab` / `Shift+Tab`: next / previous focus pane

### Scope

- `t`: next scope tab (`Files`, `Category`, `Constraint`, `Preset`, `Marketplace`)
- `T`: previous scope tab
- `Up` / `Down`: move cursor
- `Space`: toggle select/filter item
- `A`: toggle select/deselect all visible files in the Files section (respects active Files filters/settings)
- `p`: open/close Files settings popup
- `v`: toggle `Show selected categories only` (Files section)
- `Right`: expand folder (Files tab)
- `Left`: collapse folder (Files tab)
- `Ctrl+Right`: expand subtree recursively (Files tab)
- `Ctrl+Left`: collapse subtree recursively (Files tab)
- `Ctrl+1..9`: save preset slot
- `1..9`: load preset slot (Preset tab)
- `n`: new custom category input (Category tab)

### Naming

- `Up` / `Down`: move row/option
- `Space`: toggle rename-row selection
- `1` / `2` / `3`: set per-row AI suggestion option for focused preview row
- `0`: clear per-row option override (fall back to global option)
- `e`: edit override for focused row
- `Tab` while editing override: autocomplete from previous overrides
- `g` then `1..9`: assign group to selected rows
- `t`: toggle `Suggestions` / `Style` panel
- `r`: refresh AI suggestions (manual regenerate)
- `m`: open/close Suggestions settings popup
- `p`: open/close prompt history picker (Naming stage)
- `1` / `2` / `3` in Suggestions pane: set global option index
- `!` / `@` / `#` in Suggestions pane: alternate global option hotkeys
- `Left` / `Right` in `Style`: cycle style values
- `/`: focus natural prompt input
- `Enter` in prompt input: run a fresh AI suggestion generation using your prompt
- `Up` / `Down` in prompt input: navigate previously submitted prompts
- In prompt history picker: `Up`/`Down` navigate, `Enter` load prompt into input
- In Suggestions settings popup:
  - `Tab` / `Shift+Tab`: move between settings fields/actions
  - `d`: discover OpenRouter models
  - `s`: save API key/model to config
  - `c`: clear API key

### Apply

- `Enter`: simulate apply and exit
- `Ctrl+Enter`: simulate apply and stay

## Current Limitations

- No real filesystem rename write is performed (apply is simulated).
- OpenRouter is the only provider exposed in v1.
- Model discovery uses OpenRouter endpoints and may return partial results depending on auth/network.
- API key and selected model are persisted in local config (`~/.config/irium/config.toml` on macOS/Linux) with best-effort secure permissions.
- Prompt history is persisted separately in local data storage (`.../irium/prompt_history.json`) and is not stored in `config.toml`.
- Undo history is in-memory for the current session only.
- Mouse support is basic (`click`, `scroll`) and does not include range multi-select gestures.
