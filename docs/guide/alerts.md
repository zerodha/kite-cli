# Price alerts

This guide covers viewing and managing account price alerts from the CLI. You can create and modify simple price alerts, or enable, disable, and delete both simple and alert-to-order (ATO) alerts. To create or edit ATO order baskets interactively, use the [Terminal UI (TUI)](tui.md#price-alerts).

## Read alerts

```bash
kite alerts list
kite alerts list --status enabled
kite alerts get <UUID>
kite alerts history <UUID>
kite --json alerts list
```

Replace `<UUID>` with the alert identifier returned by Kite. All alert read commands require an active session.

## Create a simple alert

Alert write commands run as dry-run previews by default:

```bash
kite alerts create --name 'INFY price' --lhs NSE:INFY --operator '>=' --value 1600
```

Pass `--confirm` to create the alert on your Kite account:

```bash
kite alerts create --name 'INFY price' --lhs NSE:INFY --operator '>=' --value 1600 --confirm
```

- **Left-hand attribute (`--attribute`)** — Defaults to `LastTradedPrice`. Pass `--attribute <NAME>` to compare a different field (run `kite alerts create --help` for accepted attribute names).
- **Shell quoting** — Always quote `--operator` (for example, `'>='` or `'>'`) so your shell does not interpret `<` or `>` as file redirection.
- **Comparing two instruments (`--rhs`)** — To compare one instrument field against another instead of a constant `--value`, pass `--rhs` and `--rhs-attribute`:
  ```bash
  kite alerts create --name 'INFY exchanges' \
      --lhs NSE:INFY --attribute ClosePrice \
      --operator '>' \
      --rhs BSE:INFY --rhs-attribute ClosePrice
  ```
- **Instrument resolution** — Symbols are resolved through the local instrument cache. Index instruments are automatically mapped to Kite's `INDICES` exchange.

## Modify an alert

```bash
kite alerts modify <UUID> --value 1650
kite alerts modify <UUID> --name 'New name' --confirm
kite alerts modify <UUID> --rhs BSE:INFY
```

- `kite alerts modify` fetches the existing alert from Kite first (including during a dry run, which therefore requires a session) so that omitted fields and the alert's current status are preserved.
- Changing `--rhs` preserves the stored right-hand attribute unless you also pass `--rhs-attribute`.
- You must specify at least one field to update. To edit an ATO alert's order basket, use the [TUI alert editor](tui.md#alert-to-order-ato).

## Enable, disable, or delete

```bash
kite alerts enable <UUID>
kite alerts disable <UUID> --confirm
kite alerts delete <UUID1> <UUID2> --confirm
```

- All three commands run as dry runs unless `--confirm` is passed (`--dry-run` always takes precedence over `--confirm`).
- `enable` and `disable` fetch the stored alert first (requiring a session even for dry runs) and preserve its condition, alert type, and order basket.

> [!WARNING]
> Enabling an **alert-to-order (ATO)** alert arms its order basket on Kite's servers; when the condition triggers, Kite will submit those orders. Always review the alert's condition and basket before passing `--confirm`.

See [Orders — Dry run by default](orders.md#dry-run-by-default) for write safety rules and [Scripting](scripting.md) for exit codes and `--json` output.

---

[Back to the user guide](README.md)
