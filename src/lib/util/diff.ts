/**
 * Minimal, dependency-free line diff (LCS-based) for the agent diff viewer.
 *
 * Agent files are small (tens–hundreds of lines), so an O(m·n) LCS table is
 * fine. For anything pathologically large we bail to a coarse "replace all"
 * rather than allocate a huge table.
 */

export type DiffTag = " " | "+" | "-";

export interface DiffRow {
  tag: DiffTag;
  text: string;
  /** 1-based line number in the OLD text (null for added lines). */
  oldNo: number | null;
  /** 1-based line number in the NEW text (null for removed lines). */
  newNo: number | null;
}

const MAX_LINES = 4000; // guard the O(m·n) table

/**
 * Diff `oldText` → `newText` line by line. `-` = present in old, removed;
 * `+` = present in new, added; ` ` = unchanged context.
 */
export function diffLines(oldText: string, newText: string): DiffRow[] {
  const a = oldText.split("\n");
  const b = newText.split("\n");
  const m = a.length;
  const n = b.length;

  if (m + n > MAX_LINES) {
    // Too big for the table — show it as a wholesale replacement.
    const rows: DiffRow[] = [];
    a.forEach((t, i) => rows.push({ tag: "-", text: t, oldNo: i + 1, newNo: null }));
    b.forEach((t, j) => rows.push({ tag: "+", text: t, oldNo: null, newNo: j + 1 }));
    return rows;
  }

  // lcs[i][j] = length of the longest common subsequence of a[i..] and b[j..].
  const lcs: number[][] = Array.from({ length: m + 1 }, () => new Array<number>(n + 1).fill(0));
  for (let i = m - 1; i >= 0; i--) {
    for (let j = n - 1; j >= 0; j--) {
      lcs[i][j] = a[i] === b[j] ? lcs[i + 1][j + 1] + 1 : Math.max(lcs[i + 1][j], lcs[i][j + 1]);
    }
  }

  const rows: DiffRow[] = [];
  let i = 0;
  let j = 0;
  let oldNo = 1;
  let newNo = 1;
  while (i < m && j < n) {
    if (a[i] === b[j]) {
      rows.push({ tag: " ", text: a[i], oldNo, newNo });
      i++; j++; oldNo++; newNo++;
    } else if (lcs[i + 1][j] >= lcs[i][j + 1]) {
      rows.push({ tag: "-", text: a[i], oldNo, newNo: null });
      i++; oldNo++;
    } else {
      rows.push({ tag: "+", text: b[j], oldNo: null, newNo });
      j++; newNo++;
    }
  }
  while (i < m) { rows.push({ tag: "-", text: a[i], oldNo, newNo: null }); i++; oldNo++; }
  while (j < n) { rows.push({ tag: "+", text: b[j], oldNo: null, newNo }); j++; newNo++; }
  return rows;
}

/** Count added / removed lines for a one-line summary. */
export function diffStat(rows: DiffRow[]): { added: number; removed: number } {
  let added = 0;
  let removed = 0;
  for (const r of rows) {
    if (r.tag === "+") added++;
    else if (r.tag === "-") removed++;
  }
  return { added, removed };
}
