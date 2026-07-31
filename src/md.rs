use unicode_bidi::{BidiInfo, Level};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthChar;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    Heading(u8),
    Bullet,
    Checkbox(bool),
    Ordered,
    Table,
    Divider,
    Quote,
    Blank,
    Text,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableSpan {
    pub raw_start: usize,
    pub raw_len: usize,
}

pub struct VisualLine {
    pub chars: Vec<char>,
    pub cursor: Option<usize>,
}

pub struct Reordered {
    pub visual: Vec<char>,
    pub visual_of_logical: Vec<usize>,
    pub logical_of_visual: Vec<usize>,
}

pub fn text_width(s: &str) -> usize {
    s.chars().map(|c| UnicodeWidthChar::width(c).unwrap_or(0)).sum()
}

pub fn chars_width(chars: &[char]) -> usize {
    text_width(&chars.iter().collect::<String>())
}

pub fn classify(line: &[char]) -> BlockKind {
    if line.is_empty() {
        return BlockKind::Blank;
    }
    if line[0] == '#' {
        let mut n = 0;
        while n < line.len() && n < 6 && line[n] == '#' {
            n += 1;
        }
        if n > 0 && line.get(n) == Some(&' ') {
            return BlockKind::Heading(n as u8);
        }
    }
    let joined: String = line.iter().collect();
    if matches!(joined.trim(), "---" | "***" | "___") {
        return BlockKind::Divider;
    }
    if line[0] == '>' {
        return BlockKind::Quote;
    }
    if line[0] == '|' {
        return BlockKind::Table;
    }
    let indent = line.iter().position(|c| !c.is_whitespace()).unwrap_or(line.len());
    if indent < line.len() {
        let rest = &line[indent..];
        match rest[0] {
            '-' | '+' | '*' if rest.get(1) == Some(&' ') => {
                if rest.len() >= 6
                    && rest[1] == ' '
                    && rest[2] == '['
                    && matches!(rest[3], ' ' | 'x' | 'X')
                    && rest[4] == ']'
                    && rest[5] == ' '
                {
                    return BlockKind::Checkbox(rest[3] != ' ');
                }
                return BlockKind::Bullet;
            }
            d if d.is_ascii_digit() => {
                let mut j = 1;
                while j < rest.len() && rest[j].is_ascii_digit() {
                    j += 1;
                }
                if j > 0
                    && j < rest.len()
                    && matches!(rest[j], '.' | ')')
                    && rest.get(j + 1) == Some(&' ')
                {
                    return BlockKind::Ordered;
                }
            }
            _ => {}
        }
    }
    BlockKind::Text
}

pub fn is_table_row(line: &[char]) -> bool {
    line.first() == Some(&'|')
}

pub fn table_cells(line: &[char]) -> Vec<TableSpan> {
    let mut cells = Vec::new();
    let mut start = 0;
    for (i, &c) in line.iter().enumerate() {
        if c == '|' {
            cells.push(TableSpan {
                raw_start: start,
                raw_len: i - start,
            });
            start = i + 1;
        }
    }
    if line.last() == Some(&'|') {
        if cells.last().map(|c| c.raw_len) == Some(0) {
            cells.pop();
        }
    } else {
        cells.push(TableSpan {
            raw_start: start,
            raw_len: line.len() - start,
        });
    }
    if cells.first().map(|c| c.raw_len) == Some(0) {
        cells.remove(0);
    }
    cells
}

pub fn is_separator_row(line: &[char]) -> bool {
    let cells = table_cells(line);
    if cells.is_empty() {
        return false;
    }
    cells.iter().all(|s| {
        let t: String = line[s.raw_start..s.raw_start + s.raw_len].iter().collect();
        let t = t.trim();
        !t.is_empty() && t.chars().all(|c| c == '-' || c == ':')
    })
}

pub fn table_col_widths(lines: &[&[char]]) -> Vec<usize> {    let cols = lines
        .iter()
        .map(|l| table_cells(l).len())
        .max()
        .unwrap_or(0);
    let mut widths = vec![0usize; cols];
    for l in lines {
        if is_separator_row(l) {
            continue;
        }
        let cells = table_cells(l);
        for (i, span) in cells.iter().enumerate().take(cols) {
            let t: String = l[span.raw_start..span.raw_start + span.raw_len].iter().collect();
            let w = text_width(t.trim());
            if w > widths[i] {
                widths[i] = w;
            }
        }
    }
    widths
}

pub fn table_block_widths(lines: &[Vec<char>], index: usize) -> Option<Vec<usize>> {
    if !is_table_row(&lines[index]) {
        return None;
    }
    let mut start = index;
    while start > 0 && is_table_row(&lines[start - 1]) {
        start -= 1;
    }
    let mut end = index;
    while end + 1 < lines.len() && is_table_row(&lines[end + 1]) {
        end += 1;
    }
    let block: Vec<&[char]> = lines[start..=end].iter().map(|v| v.as_slice()).collect();
    Some(table_col_widths(&block))
}

fn cell_trim_bounds(t: &str) -> (usize, usize) {
    let fs = t.find(|c: char| !c.is_whitespace()).unwrap_or(t.len());
    let fe = t
        .rfind(|c: char| !c.is_whitespace())
        .map(|i| i + 1)
        .unwrap_or(0);
    (fs, fe)
}

pub struct TableLayout {
    pub chars: Vec<char>,
    pub visual_pos: Vec<usize>,
}

pub fn table_layout(line: &[char], widths: &[usize]) -> TableLayout {
    let n = line.len();
    if is_separator_row(line) {
        let mut out = vec!['|'];
        for (i, w) in widths.iter().enumerate() {
            if i > 0 {
                out.push('|');
            }
            out.push(' ');
            out.extend(std::iter::repeat_n('─', *w));
            out.push(' ');
        }
        out.push('|');
        let visual_pos = (0..n).collect();
        return TableLayout { chars: out, visual_pos };
    }

    let cells = table_cells(line);
    let mut out: Vec<char> = vec!['|'];
    let mut visual_pos = vec![0usize; n];
    let mut cell_visual_start = Vec::with_capacity(cells.len());
    for (i, span) in cells.iter().enumerate() {
        if i == 0 {
            visual_pos[0] = 0;
        } else {
            let pipe_idx = span.raw_start - 1;
            if pipe_idx < n {
                visual_pos[pipe_idx] = out.len();
            }
        }
        if i > 0 {
            out.push('|');
        }
        out.push(' ');
        cell_visual_start.push(out.len());
        let t: String = line[span.raw_start..span.raw_start + span.raw_len].iter().collect();
        let (fs, fe) = cell_trim_bounds(&t);
        let trimmed = if fe > fs { &t[fs..fe] } else { "" };
        let tw = text_width(trimmed);
        let pad = widths.get(i).copied().unwrap_or(0).saturating_sub(tw);
        for local in 0..span.raw_len {
            let ri = span.raw_start + local;
            let vis = if local < fs {
                cell_visual_start[i]
            } else if local < fe {
                cell_visual_start[i] + text_width(&t[fs..local])
            } else {
                cell_visual_start[i] + tw
            };
            visual_pos[ri] = vis;
        }
        out.extend(trimmed.chars());
        out.extend(std::iter::repeat_n(' ', pad));
        out.push(' ');
    }
    if line.last() == Some(&'|') && n > 0 {
        visual_pos[n - 1] = out.len();
    }
    out.push('|');
    TableLayout { chars: out, visual_pos }
}

pub fn table_visual(line: &[char], widths: &[usize], cursor: Option<usize>) -> VisualLine {
    let layout = table_layout(line, widths);
    let cursor = cursor.map(|c| layout.visual_pos.get(c).copied().unwrap_or(0));
    VisualLine {
        chars: layout.chars,
        cursor,
    }
}

pub fn is_rtl(line: &[char]) -> bool {
    if line.is_empty() {
        return false;
    }
    let s: String = line.iter().collect();
    BidiInfo::new(&s, None)
        .paragraphs
        .first()
        .map(|p| p.level.is_rtl())
        .unwrap_or(false)
}

pub fn reorder_line_visual(line: &[char]) -> Reordered {
    let n = line.len();
    if n == 0 {
        return Reordered {
            visual: Vec::new(),
            visual_of_logical: Vec::new(),
            logical_of_visual: Vec::new(),
        };
    }
    let s: String = line.iter().collect();
    let info = BidiInfo::new(&s, None);
    let Some(para) = info.paragraphs.first() else {
        return Reordered {
            visual: line.to_vec(),
            visual_of_logical: (0..n).collect(),
            logical_of_visual: (0..n).collect(),
        };
    };
    let range = para.range.clone();
    let levels = info.reordered_levels_per_char(para, range);

    let clusters: Vec<&str> = s.graphemes(true).collect();
    let mut cluster_char_starts = Vec::with_capacity(clusters.len());
    let mut idx = 0;
    for c in &clusters {
        cluster_char_starts.push(idx);
        idx += c.chars().count();
    }
    let cluster_levels: Vec<Level> = clusters
        .iter()
        .enumerate()
        .map(|(ci, _)| levels[cluster_char_starts[ci]])
        .collect();
    let index_map = BidiInfo::reorder_visual(&cluster_levels);

    let mut visual: Vec<char> = Vec::with_capacity(n);
    let mut visual_of_logical = vec![0usize; n];
    let mut logical_of_visual = Vec::with_capacity(n);
    for &log_cluster in &index_map {
        let start = cluster_char_starts[log_cluster];
        for (j, ch) in clusters[log_cluster].chars().enumerate() {
            let vis = visual.len();
            visual.push(ch);
            visual_of_logical[start + j] = vis;
            logical_of_visual.push(start + j);
        }
    }
    Reordered {
        visual,
        visual_of_logical,
        logical_of_visual,
    }
}

pub fn line_to_visual(
    line: &[char],
    cursor: Option<usize>,
    table_widths: Option<&[usize]>,
    content_width: usize,
) -> VisualLine {
    if let Some(widths) = table_widths {
        return table_visual(line, widths, cursor);
    }
    if is_rtl(line) {
        let r = reorder_line_visual(line);
        let w = chars_width(&r.visual);
        let mut pad = content_width.saturating_sub(w);
        if cursor == Some(line.len()) {
            pad = pad.saturating_sub(1);
        }
        let mut chars = Vec::with_capacity(r.visual.len() + pad);
        chars.extend(std::iter::repeat_n(' ', pad));
        chars.extend_from_slice(&r.visual);
        let cursor = cursor.map(|c| {
            let v = r.visual_of_logical.get(c).copied().unwrap_or(r.visual.len());
            v + pad
        });
        return VisualLine { chars, cursor };
    }
    VisualLine {
        chars: line.to_vec(),
        cursor,
    }
}

pub fn visual_to_logical(
    line: &[char],
    visual_x: usize,
    table_widths: Option<&[usize]>,
    content_width: usize,
) -> usize {
    if let Some(widths) = table_widths {
        let layout = table_layout(line, widths);
        if line.is_empty() {
            return 0;
        }
        layout
            .visual_pos
            .partition_point(|&v| v <= visual_x)
            .saturating_sub(1)
            .min(line.len().saturating_sub(1))
    } else if is_rtl(line) {
        let r = reorder_line_visual(line);
        let pad = content_width.saturating_sub(chars_width(&r.visual));
        if visual_x < pad {
            return 0;
        }
        r.logical_of_visual
            .get(visual_x - pad)
            .copied()
            .unwrap_or(line.len().saturating_sub(1))
    } else {
        visual_x.min(line.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    #[test]
    fn test_classify_heading() {
        assert_eq!(classify(&chars("# Title")), BlockKind::Heading(1));
        assert_eq!(classify(&chars("### Title")), BlockKind::Heading(3));
        assert_eq!(classify(&chars("####### Title")), BlockKind::Text);
        assert_eq!(classify(&chars("#NoSpace")), BlockKind::Text);
    }

    #[test]
    fn test_classify_lists() {
        assert_eq!(classify(&chars("- item")), BlockKind::Bullet);
        assert_eq!(classify(&chars("* item")), BlockKind::Bullet);
        assert_eq!(classify(&chars("+ item")), BlockKind::Bullet);
        assert_eq!(classify(&chars("-item")), BlockKind::Text);
        assert_eq!(classify(&chars("1. item")), BlockKind::Ordered);
        assert_eq!(classify(&chars("12) item")), BlockKind::Ordered);
        assert_eq!(classify(&chars("1.item")), BlockKind::Text);
        assert_eq!(classify(&chars("- [ ] todo")), BlockKind::Checkbox(false));
        assert_eq!(classify(&chars("- [x] done")), BlockKind::Checkbox(true));
    }

    #[test]
    fn test_classify_misc() {
        assert_eq!(classify(&chars("")), BlockKind::Blank);
        assert_eq!(classify(&chars("---")), BlockKind::Divider);
        assert_eq!(classify(&chars("***")), BlockKind::Divider);
        assert_eq!(classify(&chars("> quote")), BlockKind::Quote);
        assert_eq!(classify(&chars("| a | b |")), BlockKind::Table);
        assert_eq!(classify(&chars("hello")), BlockKind::Text);
    }

    #[test]
    fn test_table_cells() {
        let cells = table_cells(&chars("| a | bb |"));
        assert_eq!(cells.len(), 2);
        assert_eq!(cells[0].raw_len, 3);
        assert_eq!(cells[1].raw_len, 4);
        assert!(table_cells(&chars("|")).is_empty());
    }

    #[test]
    fn test_table_widths() {
        let l1 = chars("| a | bb |");
        let l2 = chars("| long | c |");
        let l3 = chars("|---|------|");
        let rows: Vec<&[char]> = vec![&l1, &l3, &l2];
        assert_eq!(table_col_widths(&rows), vec![4, 2]);
    }

    #[test]
    fn test_table_visual_aligns() {
        let l1 = chars("| a | bb |");
        let l2 = chars("| long | c |");
        let widths = table_col_widths(&[&l1, &l2]);
        assert_eq!(widths, vec![4, 2]);
        let v = table_visual(&l1, &widths, None);
        assert_eq!(v.chars.iter().collect::<String>(), "| a    | bb |");
        let v = table_visual(&l2, &widths, None);
        assert_eq!(v.chars.iter().collect::<String>(), "| long | c  |");
    }

    #[test]
    fn test_table_cursor_mapping() {
        let l = chars("| ab | c |");
        let widths = table_col_widths(&[&l]);
        for col in [2, 3, 7, 8] {
            let v = table_visual(&l, &widths, Some(col));
            let vis = v.cursor.unwrap();
            assert_eq!(visual_to_logical(&l, vis, Some(&widths), 80), col, "col {col}");
        }
    }

    #[test]
    fn test_table_empty_cells() {
        let header = chars("| a | b |");
        let data = chars("|   |   |");
        let widths = table_col_widths(&[&header, &data]);
        assert_eq!(widths, vec![1, 1]);
        let v = table_visual(&data, &widths, Some(2));
        assert_eq!(v.chars.iter().collect::<String>(), "|   |   |");
        assert_eq!(visual_to_logical(&data, 2, Some(&widths), 80), 3);
        assert_eq!(table_layout(&data, &widths).visual_pos.len(), data.len());
    }

    #[test]
    fn test_is_rtl() {
        assert!(is_rtl(&chars("مرحبا بالعالم")));
        assert!(!is_rtl(&chars("hello world")));
        assert!(!is_rtl(&chars("")));
        assert!(is_rtl(&chars("سلام 123")));
    }

    #[test]
    fn test_reorder_rtl() {
        let line = chars("مرحبا");
        let r = reorder_line_visual(&line);
        let visual: String = r.visual.iter().collect();
        let logical: String = r.logical_of_visual.iter().map(|&i| line[i]).collect();
        assert_eq!(logical, visual);
        // round-trip mapping: visual[visual_of_logical[i]] == line[i]
        for (i, &c) in line.iter().enumerate() {
            assert_eq!(r.visual[r.visual_of_logical[i]], c);
        }
    }

    #[test]
    fn test_reorder_ltr_identity() {
        let line = chars("hello");
        let r = reorder_line_visual(&line);
        assert_eq!(r.visual, line);
    }

    #[test]
    fn test_reorder_mixed() {
        // logical = [h e l l o space م ر ح ب ا]
        let line = chars("hello مرحبا");
        let r = reorder_line_visual(&line);
        let visual: String = r.visual.iter().collect();
        assert_eq!(visual, "hello ابحرم");
        for (i, &c) in line.iter().enumerate() {
            assert_eq!(r.visual[r.visual_of_logical[i]], c);
        }
    }
}
