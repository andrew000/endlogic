# Puzzle codes, version 1

A puzzle code is a short string that describes a whole puzzle (grid, clues,
pieces and colors) and is safe to put into a URL. It encodes the puzzle
itself rather than a generator seed, so codes keep working when the
generator changes and also cover hand-made puzzles.

Example: the reference puzzle `puzzles/wln006-2.txt` is
`AWYAG92sqX2JVQAAAAAAAURshXEtRHzP5EwZdQ` (38 characters).

## Text layer

The code is [base64url](https://www.rfc-editor.org/rfc/rfc4648#section-5)
(`A`–`Z`, `a`–`z`, `0`–`9`, `-`, `_`) without `=` padding. Unused bits in the
last character must be zero. Whitespace around the code is ignored.

## Byte layer

```text
+---------+----------------------+-------------------+
| version | payload              | checksum          |
| 1 byte  | bit stream, n bytes  | 2 bytes, big end. |
+---------+----------------------+-------------------+
```

- `version` is `1`.
- `checksum` is CRC-16/CCITT-FALSE (polynomial `0x1021`, initial value
  `0xFFFF`, no reflection, no final XOR) over the version byte and the
  payload.

## Payload

The payload is a bit stream written most significant bit first, padded with
zero bits to a whole byte. Fields, in order:

| Field | Bits | Meaning |
| --- | --- | --- |
| width − 1 | 4 | grid width 1–16 |
| height − 1 | 4 | grid height 1–16 |
| colors − 1 | 3 | palette size 1–8 |
| color id | 8 each | palette, strictly increasing |
| per palette color: has clues | 1 | `1` if clues follow |
| — row clues | `bits(width)` each | `height` values, top to bottom |
| — column clues | `bits(height)` each | `width` values, left to right |
| cells | 1–2 (+ index) each | row-major; see below |
| pieces | 6 | number of pieces, 0–32 |
| per piece: color index | `ibits` | index into the palette |
| — shape width − 1 | 4 | bounding box width |
| — shape height − 1 | 4 | bounding box height |
| — shape bitmap | width × height | row-major, `1` = cell of the piece |

- `bits(n)` is the number of bits needed for the values `0..=n` (3 for 7,
  5 for 16). A clue larger than its line cannot be encoded.
- `ibits` is the number of bits needed for an index below the palette size:
  0 for a single color, 1 for two, up to 3 for eight.
- Cells: `0` free; `10` followed by an `ibits` palette index — locked;
  `11` — dead.
- The palette is exactly the set of colors used by clues, pieces and locks
  (sorted). A puzzle that uses no color at all stores the single color 0
  with no clues.
- A shape's bitmap has no empty border row or column.

## Decoding errors

Decoding never panics. It reports, in this order of checks: an empty code; a
character outside the alphabet (or stray bits in the last character); an
impossible length; a code too short to hold version and checksum; an
unknown version; a checksum mismatch; and, for a code with a valid checksum,
truncated or inconsistent data (bad color index, clue longer than its line,
disconnected piece, trailing bytes, and so on).
