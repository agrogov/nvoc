#[derive(Clone, Copy)]
pub enum Align {
    Left,
    Right,
}

pub struct Column {
    pub header: &'static str,
    pub align: Align,
    pub max_width: Option<usize>,
}

pub struct Table {
    columns: Vec<Column>,
    rows: Vec<Vec<String>>,
}

impl Table {
    pub fn new(columns: Vec<Column>) -> Self {
        Self { columns, rows: Vec::new() }
    }

    pub fn push_row<I, S>(&mut self, cells: I)
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.rows.push(cells.into_iter().map(Into::into).collect());
    }

    pub fn print(&self) {
        let widths = self.compute_widths();
        self.print_border(&widths);
        self.print_row(
            &widths,
            self.columns.iter().map(|c| c.header.to_string()).collect(),
            true,
        );
        self.print_border(&widths);
        for row in &self.rows {
            self.print_row(&widths, row.clone(), false);
        }
        self.print_border(&widths);
    }

    fn compute_widths(&self) -> Vec<usize> {
        let mut widths: Vec<usize> = self.columns.iter().map(|c| c.header.len()).collect();
        for row in &self.rows {
            for (i, cell) in row.iter().enumerate() {
                if i >= widths.len() {
                    break;
                }
                let w = cell.chars().count();
                if w > widths[i] {
                    widths[i] = w;
                }
            }
        }
        for (i, col) in self.columns.iter().enumerate() {
            if let Some(max) = col.max_width {
                widths[i] = widths[i].min(max);
            }
        }
        widths
    }

    fn print_border(&self, widths: &[usize]) {
        print!("+");
        for w in widths {
            print!("{}+", "-".repeat(*w + 2));
        }
        println!();
    }

    fn print_row(&self, widths: &[usize], mut row: Vec<String>, header: bool) {
        if row.len() < self.columns.len() {
            row.resize(self.columns.len(), String::new());
        }
        print!("|");
        for (i, col) in self.columns.iter().enumerate() {
            let mut cell = row.get(i).cloned().unwrap_or_default();
            cell = truncate(&cell, widths[i]);
            let pad = widths[i].saturating_sub(cell.chars().count());
            match col.align {
                Align::Left => print!(" {}{} |", cell, " ".repeat(pad)),
                Align::Right => print!(" {}{} |", " ".repeat(pad), cell),
            }
        }
        println!();
        if header {
            // no-op
        }
    }
}

fn truncate(s: &str, max_chars: usize) -> String {
    let len = s.chars().count();
    if len <= max_chars {
        return s.to_string();
    }
    if max_chars == 0 {
        return String::new();
    }
    if max_chars == 1 {
        return "…".to_string();
    }
    let take = max_chars - 1;
    let mut out = s.chars().take(take).collect::<String>();
    out.push('…');
    out
}

