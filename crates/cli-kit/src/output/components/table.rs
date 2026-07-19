use crate::output::figures;
use crate::output::tokens::TokenItem;

/// A typed cell value.
#[derive(Debug, Clone)]
pub struct Cell<T> {
    pub value: T,
    pub color: Option<TableColor>,
    pub alignment: CellAlignment,
}

impl<T> Cell<T> {
    pub fn new(value: T) -> Self {
        Self {
            value,
            color: None,
            alignment: CellAlignment::Left,
        }
    }

    pub fn with_color(mut self, color: TableColor) -> Self {
        self.color = Some(color);
        self
    }

    pub fn with_alignment(mut self, alignment: CellAlignment) -> Self {
        self.alignment = alignment;
        self
    }
}

impl<T: Into<String>> From<T> for Cell<String> {
    fn from(value: T) -> Self {
        Self::new(value.into())
    }
}

/// Color applied to a cell value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TableColor {
    Green,
    Yellow,
    Red,
    Dim,
    Cyan,
    Magenta,
}

impl TableColor {
    fn apply(self, text: &str, colors_enabled: bool) -> String {
        if !colors_enabled {
            return text.to_string();
        }
        match self {
            TableColor::Green => colored::Colorize::green(text).to_string(),
            TableColor::Yellow => colored::Colorize::yellow(text).to_string(),
            TableColor::Red => colored::Colorize::red(text).to_string(),
            TableColor::Dim => colored::Colorize::dimmed(text).to_string(),
            TableColor::Cyan => colored::Colorize::cyan(text).to_string(),
            TableColor::Magenta => colored::Colorize::magenta(text).to_string(),
        }
    }
}

/// Cell alignment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CellAlignment {
    Left,
    Right,
    Center,
}

/// A row of cells in a table.
#[derive(Debug, Clone)]
pub struct Row<T> {
    pub cells: Vec<Cell<T>>,
}

impl<T> Row<T> {
    pub fn new(cells: Vec<Cell<T>>) -> Self {
        Self { cells }
    }
}

/// A table with headers, rows, and per-column options.
#[derive(Debug, Clone)]
pub struct Table {
    pub headers: Vec<String>,
    pub rows: Vec<Row<String>>,
    pub column_colors: Vec<Option<TableColor>>,
}

impl Table {
    pub fn new(headers: Vec<String>) -> Self {
        Self {
            headers,
            rows: Vec::new(),
            column_colors: Vec::new(),
        }
    }

    pub fn with_rows(mut self, rows: Vec<Row<String>>) -> Self {
        self.rows = rows;
        self
    }

    pub fn with_column_colors(mut self, colors: Vec<Option<TableColor>>) -> Self {
        self.column_colors = colors;
        self
    }

    pub fn add_row(mut self, row: Row<String>) -> Self {
        self.rows.push(row);
        self
    }

    /// Render the table as a list of TokenItem lines.
    pub fn render(&self, colors_enabled: bool) -> Vec<TokenItem> {
        let num_cols = self
            .headers
            .len()
            .max(self.rows.iter().map(|r| r.cells.len()).max().unwrap_or(0));

        if num_cols == 0 {
            return Vec::new();
        }

        // Calculate column widths: max of header length and all cell lengths
        let column_widths: Vec<usize> = (0..num_cols)
            .map(|col| {
                let header_len = self.headers.get(col).map(|h| h.len()).unwrap_or(0);
                let max_cell = self
                    .rows
                    .iter()
                    .map(|row| row.cells.get(col).map(|c| c.value.len()).unwrap_or(0))
                    .max()
                    .unwrap_or(0);
                header_len.max(max_cell)
            })
            .collect();

        let mut items = Vec::new();

        // Header row
        self.render_separator(&column_widths, colors_enabled, &mut items);
        let header_line = self.render_header_line(&column_widths, colors_enabled);
        items.push(header_line);
        self.render_separator(&column_widths, colors_enabled, &mut items);

        // Data rows
        for row in &self.rows {
            let line = self.render_data_row(row, &column_widths, colors_enabled);
            items.push(line);
        }

        items
    }

    fn render_header_line(&self, widths: &[usize], colors_enabled: bool) -> TokenItem {
        let mut line = String::new();
        for (i, header) in self.headers.iter().enumerate() {
            let cell_text = if colors_enabled {
                colored::Colorize::bold(&header[..]).to_string()
            } else {
                header.clone()
            };
            line.push_str(&pad_cell(&cell_text, widths[i], CellAlignment::Left));
            if i < self.headers.len() - 1 {
                line.push_str("  ");
            }
        }
        TokenItem::raw(line)
    }

    fn render_separator(
        &self,
        widths: &[usize],
        colors_enabled: bool,
        items: &mut Vec<TokenItem>,
    ) {
        let parts: Vec<String> = widths
            .iter()
            .map(|w| figures::HORIZONTAL_LINE.repeat(*w))
            .collect();
        let line = parts.join("──");
        let styled = if colors_enabled {
            colored::Colorize::dimmed(&line[..]).to_string()
        } else {
            line
        };
        items.push(TokenItem::raw(styled));
    }

    fn render_data_row(
        &self,
        row: &Row<String>,
        widths: &[usize],
        colors_enabled: bool,
    ) -> TokenItem {
        let mut line = String::new();
        for (i, cell) in row.cells.iter().enumerate() {
            let color = cell
                .color
                .or(self.column_colors.get(i).copied().flatten());
            let text = match color {
                Some(c) => c.apply(&cell.value, colors_enabled),
                None => cell.value.clone(),
            };
            let padded = pad_cell(&text, widths[i], cell.alignment);
            line.push_str(&padded);
            if i < row.cells.len() - 1 {
                line.push_str("  ");
            }
        }
        TokenItem::raw(line)
    }
}

fn pad_cell(text: &str, width: usize, alignment: CellAlignment) -> String {
    let visual_len = text.len();
    match alignment {
        CellAlignment::Left => format!("{:<width$}", text),
        CellAlignment::Right => format!("{:>width$}", text),
        CellAlignment::Center => {
            let pad = width.saturating_sub(visual_len);
            let left = pad / 2;
            let right = pad - left;
            format!("{}{}{}", " ".repeat(left), text, " ".repeat(right))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_table_empty() {
        let table = Table::new(vec![]);
        let items = table.render(false);
        assert!(items.is_empty());
    }

    #[test]
    fn test_table_basic() {
        let table = Table::new(vec!["Name".into(), "Role".into()])
            .with_rows(vec![
                Row::new(vec!["Alice".into(), "Admin".into()]),
                Row::new(vec!["Bob".into(), "Editor".into()]),
            ]);
        let items = table.render(false);
        let text: Vec<String> = items.iter().map(|t| t.render_plain()).collect();
        assert!(text.iter().any(|l| l.contains("Name")));
        assert!(text.iter().any(|l| l.contains("Alice")));
        assert!(text.iter().any(|l| l.contains("Bob")));
        assert!(text.iter().any(|l| l.contains("─")));
    }

    #[test]
    fn test_table_with_colors() {
        let table = Table::new(vec!["Status".into()])
            .with_column_colors(vec![Some(TableColor::Green)])
            .with_rows(vec![Row::new(vec!["OK".into()])]);
        let items = table.render(true);
        assert!(!items.is_empty());
    }

    #[test]
    fn test_cell_conversion() {
        let cell: Cell<String> = "test".into();
        assert_eq!(cell.value, "test");
    }

    #[test]
    fn test_cell_with_color() {
        let cell = Cell::new("value").with_color(TableColor::Yellow);
        assert_eq!(cell.color, Some(TableColor::Yellow));
    }

    #[test]
    fn test_table_color_apply() {
        assert_eq!(TableColor::Green.apply("text", false), "text");
        let result = TableColor::Green.apply("text", true);
        assert!(result.contains("text"));
    }

    #[test]
    fn test_pad_cell() {
        let padded = pad_cell("hi", 5, CellAlignment::Left);
        assert_eq!(padded, "hi   ");
    }

    #[test]
    fn test_table_add_row() {
        let table = Table::new(vec!["A".into()])
            .add_row(Row::new(vec!["1".into()]));
        assert_eq!(table.rows.len(), 1);
    }
}
