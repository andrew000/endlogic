# Puzzle text format, versions 1 and 2

A puzzle file is plain UTF-8 text. Version 1 describes single-color
puzzles; version 2 describes puzzles of one to four colors. Both are read;
a puzzle is written in version 1 whenever it uses only the first color.

## Example, version 1

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

## Example, version 2

```text
endlogic 2
colors 2
grid
......
......
......
....a.
......
..a...
rows a 6 3 3 4 4 2
rows b 0 2 3 1 2 1
cols a 6 3 3 4 4 2
cols b 0 2 3 1 2 1
piece b
##
#.
piece a
#.#
###
```

## Lines

- Leading and trailing whitespace on every line is ignored, so `\n` and
  `\r\n` line endings both work.
- Blank lines are ignored and may appear between blocks.
- A line whose first non-blank characters are `//` is a comment and is
  ignored. Comments must be on their own line.

## Colors

The colors are named by letters, in this order: `a` (teal), `b` (orange),
`c` (violet), `d` (lime). A puzzle with `colors N` uses the first `N`
letters. Every piece and every locked cell has one color; dead cells have
none. A cell of a piece or a lock counts only towards the clues of its own
color, and pieces of different colors may not overlap either.

## Sections, version 1

Sections appear in this fixed order:

1. **Header**: the first significant line is `endlogic 1`.
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

All pieces and locks have the color `a`. A block ends at the next keyword
line (`grid`, `rows`, `cols`, `piece`, `endlogic`).

## Sections, version 2

1. **Header**: `endlogic 2`.
2. **`colors`**: `colors` and the number of colors, 1 to 4.
3. **`grid`**: as in version 1, except that a locked cell is the letter of
   its color (`a` to `d`, one of the declared colors) and `#` is not
   allowed.
4. **Clues**: for every declared color, one `rows` line and one `cols`
   line: `rows a 6 3 3 4 4 2` gives the counts of color `a` for every row,
   top to bottom; `cols a ...` for every column, left to right. Each count
   includes the locks of that color. The lines may come in any order; the
   canonical order is all `rows` lines by color, then all `cols` lines by
   color.
5. **`piece`** blocks: as in version 1, but the `piece` line names the
   piece's color: `piece b`.

A block ends at the next keyword line (`colors`, `grid`, `rows`, `cols`,
`piece`, `endlogic`).

## Errors

Structural problems are reported with a one-based line number:

| Error | Reported at |
| --- | --- |
| missing or malformed header | the first significant line |
| unsupported version (not 1 or 2) | the header line |
| input ends before a required section | number of lines + 1 |
| a line that does not start the expected section | that line |
| `colors` not followed by a number from 1 to 4 | the `colors` line |
| grid with no rows | the `grid` line |
| invalid grid character (version 2: also `#` and undeclared colors) | the grid row |
| grid row of a different length than the first | the grid row |
| grid wider or taller than 16 | the first row that exceeds the limit |
| missing or invalid color letter on a `rows`, `cols` or `piece` line | that line |
| color letter past the declared colors | that line |
| a second `rows` or `cols` line for the same color | the second line |
| a declared color without a `rows` or `cols` line | the line after the clues |
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

The serializer writes a puzzle that uses no color but `a` in version 1:
the header, `grid`, the rows, `rows`, `cols` and then each piece as its
bounding box. Any other puzzle is written in version 2 with as many colors
as its highest color needs: the header, `colors`, `grid`, the rows, the
`rows` lines and the `cols` lines of every color in order, and then each
piece with its color. There are no comments or blank lines. Parsing the
canonical form returns the same puzzle.
