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
- `[` / `]`: previous / next stage
- `Tab` / `Shift+Tab`: next / previous focus pane

### Scope

- `t`: next scope tab (`Files`, `Category`, `Constraint`, `Preset`, `Marketplace`)
- `T`: previous scope tab
- `Up` / `Down`: move cursor
- `Space`: toggle select/filter item
- `A`: toggle select/deselect all files in the Files section (recursive)
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
- `e`: edit override for focused row
- `Tab` while editing override: autocomplete from previous overrides
- `g` then `1..9`: assign group to selected rows
- `t`: toggle `Suggestions` / `Style` panel
- `Left` / `Right` in `Style`: cycle style values
- `/`: focus natural command input
- `Enter` in command input: apply deterministic tokens (`short`, `long`, `title`, `lower`, `upper`, `dash`, `underscore`, `space`, `no-colon`, `keep-ext`)

### Apply

- `Enter`: simulate apply and exit
- `Ctrl+Enter`: simulate apply and stay

## Current Limitations

- No real filesystem rename write is performed (apply is simulated).
- No AI categorization/name suggestion integration is implemented.
- Undo history is in-memory for the current session only.
- Mouse support is basic (`click`, `scroll`) and does not include range multi-select gestures.
