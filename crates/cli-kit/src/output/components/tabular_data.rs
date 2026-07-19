use crate::output::tokens::TokenItem;

/// A row in a tabular data grid.
#[derive(Debug, Clone)]
pub struct TabularRow {
    pub cells: Vec<String>,
}

impl TabularRow {
    pub fn new(cells: Vec<String>) -> Self {
        Self { cells }
    }
}

impl From<Vec<String>> for TabularRow {
    fn from(cells: Vec<String>) -> Self {
        Self::new(cells)
    }
}

/// Render a column-aligned grid of data.
/// `first_column_subdued` dims the first column of each row.
/// Returns a list of TokenItem lines.
pub fn render_tabular_data(
    data: &[TabularRow],
    first_column_subdued: bool,
    colors_enabled: bool,
) -> Vec<TokenItem> {
    if data.is_empty() {
        return Vec::new();
    }

    let num_cols = data.iter().map(|r| r.cells.len()).max().unwrap_or(0);
    if num_cols == 0 {
        return Vec::new();
    }

    let column_widths: Vec<usize> = (0..num_cols)
        .map(|col| {
            data.iter()
                .map(|row| row.cells.get(col).map(|c| c.len()).unwrap_or(0))
                .max()
                .unwrap_or(0)
        })
        .collect();

    let mut result = Vec::with_capacity(data.len());
    for row in data {
        let mut line = String::new();
        for (col, cell) in row.cells.iter().enumerate() {
            let display = if col == 0 && first_column_subdued {
                if colors_enabled {
                    colored::Colorize::dimmed(&cell[..]).to_string()
                } else {
                    cell.clone()
                }
            } else {
                cell.clone()
            };
            let padding = column_widths[col].saturating_sub(cell.len());
            line.push_str(&display);
            if col < row.cells.len() - 1 {
                line.push_str(&" ".repeat(padding + 2));
            }
        }
        result.push(TokenItem::raw(line));
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tabular_data_empty() {
        let result = render_tabular_data(&[], false, false);
        assert!(result.is_empty());
    }

    #[test]
    fn test_tabular_data_basic() {
        let data = vec![
            TabularRow::new(vec!["Name".into(), "Role".into()]),
            TabularRow::new(vec!["Alice".into(), "Admin".into()]),
        ];
        let result = render_tabular_data(&data, false, false);
        assert_eq!(result.len(), 2);
        assert!(result[0].render_plain().contains("Name"));
    }

    #[test]
    fn test_tabular_data_first_column_subdued() {
        colored::control::set_override(true);
        let data = vec![TabularRow::new(vec!["key".into(), "value".into()])];
        let result = render_tabular_data(&data, true, true);
        let out = result[0].render_ansi(true);
        assert!(out.contains("\x1b[2m"));
        assert!(out.contains("key"));
    }

    #[test]
    fn test_tabular_data_uneven_rows() {
        let data = vec![
            TabularRow::new(vec!["a".into(), "b".into()]),
            TabularRow::new(vec!["c".into()]),
        ];
        let result = render_tabular_data(&data, false, false);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_tabular_data_aligned() {
        let data = vec![
            TabularRow::new(vec!["A".into(), "B".into()]),
            TabularRow::new(vec!["LongName".into(), "C".into()]),
        ];
        let result = render_tabular_data(&data, false, false);
        assert_eq!(result.len(), 2);
    }
}
