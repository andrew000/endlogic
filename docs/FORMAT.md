# Puzzle text format, version 1

A puzzle file is plain UTF-8 text. Version 1 describes single-color puzzles.

## Example

```text
endlogic 1
// Reference puzzle from the game.
grid
.#####.
.......
.......
.......
.......
.......
.......
rows 5 7 3 5 5 3 1
cols 2 4 5 7 5 4 2

piece
#.
##

piece
.#.
###
```

## Lines

- Leading and trailing whitespace on every line is ignored, so `\n` and
  `\r\n` line endings both work.
- Blank lines are ignored and may appear between blocks.
- A line whose first non-blank characters are `//` is a comment and is
  ignored. Comments must be on their own line.

## Sections

Sections appear in this fixed order:

1. **Header**: the first significant line is `endlogic 1`. Any other version
   number is rejected.
2. **`grid`**: a line containing only `grid`, followed by the rows of the
   board, top to bottom. Each row is a string of:
   - `.` — free cell;
   - `#` — locked cell (pre-filled; counts as filled);
   - `x` — dead cell (cannot be covered, never counts).

   All rows must have the same length. The grid size is taken from these
   rows and must be between 1×1 and 16×16.
3. **`rows`**: `rows` followed by one non-negative integer per grid row, top
   to bottom, separated by spaces.
4. **`cols`**: `cols` followed by one non-negative integer per grid column,
   left to right.

   Each number is the total count of filled cells in that line, locked
   cells included (as shown in the game).
5. **`piece`** blocks, zero or more: a line containing only `piece`, followed
   by the rows of the piece. `#` is a cell of the piece and `.` is empty.
   Rows may have different lengths; missing cells are empty. Empty rows and
   columns around the piece are allowed and trimmed. The piece must have at
   least one cell, its cells must be 4-connected (edge-adjacent), and its
   bounding box must fit in 16×16. At most 32 pieces are allowed. The order of
   the blocks is the order of the pieces; when solutions are printed, the
   first piece is `A`, the second `B`, and so on.

A block ends at the next keyword line (`grid`, `rows`, `cols`, `piece`,
`endlogic`).

## Errors

Structural problems are reported with a one-based line number:

| Error | Reported at |
| --- | --- |
| missing or malformed header | the first significant line |
| unsupported version | the header line |
| input ends before a required section | number of lines + 1 |
| a line that does not start the expected section | that line |
| grid with no rows | the `grid` line |
| invalid grid character | the grid row |
| grid row of a different length than the first | the grid row |
| grid wider or taller than 16 | the first row that exceeds the limit |
| clue that is not a non-negative integer | the `rows`/`cols` line |
| wrong number of clues | the `rows`/`cols` line |
| invalid piece character | the piece row |
| piece with no cells | the `piece` line |
| disconnected piece | the `piece` line |
| piece larger than 16×16 | the `piece` line |
| more than 32 pieces | the 33rd `piece` line |

Logical inconsistencies, such as clue sums that do not match the area of the
pieces plus the locks, are not parse errors; such a puzzle has no solutions.

## Canonical form

The serializer writes the header, `grid`, the rows, `rows`, `cols` and then
each piece as its bounding box, with no comments or blank lines. Parsing the
canonical form returns the same puzzle.
