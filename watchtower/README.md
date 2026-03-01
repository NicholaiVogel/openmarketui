# the observation deck

a terminal UI for monitoring the prediction market garden.

## overview

the observation deck provides a real-time view into the garden's health:

- **garden overview**: see all beds (strategy families) and their specimens (individual strategies), whether they're blooming (active) or dormant (paused)
- **current harvest**: monitor active positions in the market
- **harvest history**: review past trades and their yields (P&L)
- **greenhouse controls**: manage specimen lifecycle (toggle status, adjust weights)

## running

```bash
bun dev
```

## configuration

set the pm-server websocket endpoint:

```bash
PM_SERVER_URL=ws://localhost:3030/ws bun dev
```

## keybindings

| key | action |
|-----|--------|
| `1` | garden overview |
| `2` | current harvest |
| `3` | harvest history |
| `4` | greenhouse controls |
| `r` | reconnect to server |
| `q` | quit |

## terminology

this project uses the garden metaphor throughout:

| concept | garden term |
|---------|-------------|
| strategy | specimen |
| enabled | blooming |
| disabled | dormant |
| trade fills | harvests |
| P&L | yield |
| strategy family | bed |
| filters | immune system |

## architecture

built with [opentui](https://github.com/sst/opentui), a react-based TUI framework.

connects to pm-server via websocket for real-time updates. when disconnected,
displays demo data to show the UI structure.
