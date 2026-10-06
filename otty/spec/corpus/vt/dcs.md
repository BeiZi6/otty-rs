<!-- source: https://docs.otty.sh/vt/dcs -->

# DCS Sequences

Device Control String — `ESC P` (or the 8-bit `0x90`) followed by parameters, an intermediate/final selector, a string payload, and a string terminator. DCS frames carry longer, structured payloads than CSI or OSC: sixel images, terminfo queries, and the DECRQSS setting queries.

## Structure

```
ESC P <params> <selector> <payload> ST
```

- **`selector`**: intermediate + final bytes identifying the function (e.g. `$ q` for DECRQSS)
- **`payload`**: function-specific data
- **`ST`**: string terminator — `ESC \` (preferred) or `0x9C`

## Otty support

`vte` does not dispatch general DCS payloads, so most DCS-framed functions are inert. Otty scans the byte stream for the ones it answers:

- **DECRQSS cursor-style query** (`DCS $ q SP q ST`) — replied with the current cursor code. See below.
- **XTVERSION reply** — Otty *emits* `DCS > | otty(<version>) ST` in response to the `CSI > q` request (so nvim / kakoune can gate features).

Sixel graphics and XTGETTCAP (`DCS + q`) are not supported.

## Detail pages

- [DECRQSS — Request Selection or Setting](/vt/dcs/decrqss)
