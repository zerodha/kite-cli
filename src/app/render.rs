use crate::app::errors::{MissingSession, error_type, format_error};
use crate::tui::format;
use serde_json::Value;
use std::io::{self, IsTerminal, Write};

#[derive(Clone, Copy, Default)]
pub(crate) struct Term {
    pub color: bool,
    pub width: Option<usize>,
}

impl Term {
    pub fn stdout() -> Self {
        Self::new(io::stdout().is_terminal(), true)
    }

    pub fn stderr() -> Self {
        Self::new(io::stderr().is_terminal(), false)
    }

    fn new(tty: bool, stdout: bool) -> Self {
        Self {
            color: use_color(
                tty,
                std::env::var_os("NO_COLOR").is_some(),
                std::env::var("TERM").as_deref() == Ok("dumb"),
            ),
            width: if tty && stdout {
                std::env::var("COLUMNS")
                    .ok()
                    .and_then(|value| value.parse().ok())
                    .filter(|width| *width > 0)
                    .or_else(|| {
                        crossterm::terminal::size()
                            .ok()
                            .map(|(width, _)| usize::from(width))
                    })
            } else {
                None
            },
        }
    }
}

fn use_color(tty: bool, no_color: bool, dumb: bool) -> bool {
    tty && !no_color && !dumb
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum Tone {
    #[default]
    Plain,
    Dim,
    Bold,
    Good,
    Bad,
    Warn,
}

#[derive(Clone)]
pub(crate) struct Cell {
    pub text: String,
    pub tone: Tone,
}

impl Cell {
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }
}

impl<T: Into<String>> From<T> for Cell {
    fn from(text: T) -> Self {
        Self {
            text: clean(&text.into()),
            tone: Tone::Plain,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Align {
    Left,
    Right,
    Decimal,
}

pub(crate) struct Col {
    head: String,
    align: Align,
    drop: u8,
    flex: bool,
}

impl Col {
    pub fn text(head: &str) -> Self {
        Self {
            head: clean(head),
            align: Align::Left,
            drop: 0,
            flex: false,
        }
    }

    pub fn number(head: &str) -> Self {
        Self {
            align: Align::Right,
            ..Self::text(head)
        }
    }

    pub fn decimal(head: &str) -> Self {
        Self {
            align: Align::Decimal,
            ..Self::text(head)
        }
    }

    pub fn drop(mut self, rank: u8) -> Self {
        self.drop = rank;
        self
    }
    pub fn flex(mut self) -> Self {
        self.flex = true;
        self
    }
}

pub(crate) struct Table {
    cols: Vec<Col>,
    rows: Vec<(Vec<Cell>, Tone)>,
    total: Option<Vec<Cell>>,
}

impl Table {
    pub fn new(cols: Vec<Col>) -> Self {
        Self {
            cols,
            rows: Vec::new(),
            total: None,
        }
    }
    pub fn row(&mut self, cells: Vec<Cell>) {
        self.rows.push((cells, Tone::Plain));
    }
    pub fn dim_row(&mut self, cells: Vec<Cell>) {
        self.rows.push((cells, Tone::Dim));
    }
    pub fn total(&mut self, cells: Vec<Cell>) {
        self.total = Some(cells);
    }

    fn write(&self, out: &mut impl Write, term: Term) -> io::Result<Vec<String>> {
        let mut widths: Vec<usize> = self
            .cols
            .iter()
            .map(|col| format::display_width(&col.head))
            .collect();
        let mut fractions = vec![0; self.cols.len()];
        let rows = || {
            self.rows
                .iter()
                .map(|(cells, _)| cells)
                .chain(self.total.iter())
        };
        for cells in rows() {
            for (index, cell) in cells.iter().enumerate().take(widths.len()) {
                widths[index] = widths[index].max(format::display_width(&cell.text));
                if matches!(self.cols[index].align, Align::Decimal) {
                    fractions[index] = fractions[index].max(fraction_width(&cell.text));
                }
            }
        }
        for (index, col) in self.cols.iter().enumerate() {
            if matches!(col.align, Align::Decimal) {
                for cells in rows() {
                    if let Some(cell) = cells.get(index) {
                        widths[index] = widths[index].max(
                            format::display_width(&cell.text) + fractions[index]
                                - fraction_width(&cell.text),
                        );
                    }
                }
            }
        }
        let mut visible: Vec<usize> = (0..self.cols.len()).collect();
        let mut hidden = Vec::new();
        if let Some(limit) = term.width {
            while table_width(&visible, &widths) > limit {
                let candidate = visible
                    .iter()
                    .copied()
                    .filter(|index| self.cols[*index].drop > 0)
                    .max_by_key(|index| self.cols[*index].drop);
                let Some(index) = candidate else { break };
                visible.retain(|other| *other != index);
                hidden.push(self.cols[index].head.as_str());
            }
            if let Some(index) = visible.iter().copied().find(|index| self.cols[*index].flex) {
                let excess = table_width(&visible, &widths).saturating_sub(limit);
                widths[index] = widths[index]
                    .saturating_sub(excess)
                    .max(format::display_width(&self.cols[index].head).max(4));
            }
        }
        if term
            .width
            .is_some_and(|limit| table_width(&visible, &widths) > limit)
        {
            self.write_vertical(out, term, &visible)?;
            return Ok(hidden.into_iter().map(str::to_string).collect());
        }
        let headers: Vec<Cell> = self
            .cols
            .iter()
            .map(|col| Cell::from(col.head.clone()).tone(Tone::Dim))
            .collect();
        self.write_row(
            out,
            &headers,
            Tone::Plain,
            term,
            &visible,
            &widths,
            &vec![0; self.cols.len()],
        )?;
        for (cells, tone) in &self.rows {
            self.write_row(out, cells, *tone, term, &visible, &widths, &fractions)?;
        }
        if let Some(total) = &self.total {
            writeln!(
                out,
                "{}",
                paint(&"─".repeat(table_width(&visible, &widths)), Tone::Dim, term)
            )?;
            self.write_row(out, total, Tone::Bold, term, &visible, &widths, &fractions)?;
        }
        Ok(hidden.into_iter().map(str::to_string).collect())
    }

    fn write_vertical(
        &self,
        out: &mut impl Write,
        term: Term,
        visible: &[usize],
    ) -> io::Result<()> {
        for (row, _) in &self.rows {
            let fields = visible
                .iter()
                .map(|&index| {
                    (
                        self.cols[index].head.clone(),
                        row.get(index).cloned().unwrap_or_else(|| "–".into()),
                    )
                })
                .collect::<Vec<_>>();
            write_fields(&fields, out, term)?;
            writeln!(out)?;
        }
        if let Some(total) = &self.total {
            let fields = visible
                .iter()
                .map(|&index| (self.cols[index].head.clone(), total[index].clone()))
                .collect::<Vec<_>>();
            write_fields(&fields, out, term)?;
        }
        Ok(())
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "one table row uses the table's calculated widths and visible columns"
    )]
    fn write_row(
        &self,
        out: &mut impl Write,
        cells: &[Cell],
        row_tone: Tone,
        term: Term,
        visible: &[usize],
        widths: &[usize],
        fractions: &[usize],
    ) -> io::Result<()> {
        let mut rendered = Vec::new();
        for &index in visible {
            let cell = cells.get(index).cloned().unwrap_or_else(|| Cell::from("–"));
            let text = if self.cols[index].flex && format::display_width(&cell.text) > widths[index]
            {
                format!(
                    "{}…",
                    format::pad_or_clip(&cell.text, widths[index].saturating_sub(1))
                )
            } else {
                cell.text
            };
            let trailing = if matches!(self.cols[index].align, Align::Decimal) && !text.is_empty() {
                fractions[index].saturating_sub(fraction_width(&text))
            } else {
                0
            };
            let padding = widths[index].saturating_sub(format::display_width(&text) + trailing);
            let aligned = match self.cols[index].align {
                Align::Left => format!("{text}{}", " ".repeat(padding)),
                Align::Right | Align::Decimal => {
                    format!("{}{text}{}", " ".repeat(padding), " ".repeat(trailing))
                }
            };
            let tone = if row_tone == Tone::Plain {
                cell.tone
            } else {
                row_tone
            };
            rendered.push(paint(
                if index == *visible.last().unwrap_or(&index) {
                    aligned.trim_end()
                } else {
                    &aligned
                },
                tone,
                term,
            ));
        }
        writeln!(out, "{}", rendered.join("  "))
    }
}

fn fraction_width(text: &str) -> usize {
    text.split_once('.')
        .map_or(0, |(_, fraction)| format::display_width(fraction))
}

fn table_width(visible: &[usize], widths: &[usize]) -> usize {
    visible.iter().map(|index| widths[*index]).sum::<usize>() + visible.len().saturating_sub(1) * 2
}

enum Block {
    Line(Vec<Cell>),
    Fields(Vec<(String, Cell)>),
    Table(Table),
    Note(&'static str, String),
}

#[derive(Default)]
pub struct Doc {
    blocks: Vec<Block>,
}

impl Doc {
    pub(crate) fn line(&mut self, cells: Vec<Cell>) -> &mut Self {
        self.blocks.push(Block::Line(cells));
        self
    }
    pub(crate) fn title(&mut self, text: &str) -> &mut Self {
        self.line(vec![Cell::from(text).tone(Tone::Bold)])
    }
    pub(crate) fn blank(&mut self) -> &mut Self {
        self.line(Vec::new())
    }
    pub(crate) fn fields(&mut self, rows: Vec<(&str, Cell)>) -> &mut Self {
        self.blocks.push(Block::Fields(
            rows.into_iter()
                .map(|(key, cell)| (clean(key), cell))
                .collect(),
        ));
        self
    }
    pub(crate) fn table(&mut self, table: Table) -> &mut Self {
        self.blocks.push(Block::Table(table));
        self
    }
    pub(crate) fn success(&mut self, headline: &str) -> &mut Self {
        self.line(vec![Cell::from("✓").tone(Tone::Good), Cell::from(headline)])
    }
    pub(crate) fn empty(&mut self, text: &str) -> &mut Self {
        self.blocks.push(Block::Note("", clean(text)));
        self
    }
    pub(crate) fn warn(&mut self, text: &str) -> &mut Self {
        self.blocks.push(Block::Note("warning", clean(text)));
        self
    }
    pub(crate) fn hint(&mut self, text: &str) -> &mut Self {
        self.blocks.push(Block::Note("hint", clean(text)));
        self
    }

    pub(crate) fn dry_run(&mut self, summary: &str, value: &Value) {
        self.line(vec![
            Cell::from("DRY RUN ").tone(Tone::Warn),
            Cell::from("nothing was sent to Kite"),
        ]);
        if !summary.is_empty() {
            self.line(vec![summary.into()]);
        }
        self.blank().line(vec![
            format!("{} {}", s(value, "method"), s(value, "path")).into(),
        ]);
        let mut fields = Vec::new();
        for pair in value["query"].as_array().into_iter().flatten() {
            if let [key, value] = pair.as_array().map_or(&[][..], Vec::as_slice) {
                fields.push((clean(&scalar(key)), scalar(value).into()));
            }
        }
        if let Some(form) = value.pointer("/body/form").and_then(Value::as_object) {
            let ordered = [
                "exchange",
                "tradingsymbol",
                "transaction_type",
                "order_type",
                "quantity",
                "product",
                "price",
                "validity",
            ];
            for key in ordered.into_iter().chain(
                form.keys()
                    .map(String::as_str)
                    .filter(|key| !ordered.contains(key)),
            ) {
                if let Some(value) = form.get(key) {
                    fields.push((clean(key), scalar(value).into()));
                }
            }
        } else if let Some(body) = value.pointer("/body/json") {
            fields.push(("body".into(), body.to_string().into()));
        }
        self.blocks.push(Block::Fields(fields));
        if let Some(next) = value.get("next").and_then(Value::as_str) {
            self.hint(next);
        }
    }

    pub(crate) fn write(
        &self,
        out: &mut impl Write,
        out_term: Term,
        err: &mut impl Write,
        err_term: Term,
    ) -> io::Result<()> {
        let mut hidden = Vec::new();
        for block in &self.blocks {
            match block {
                Block::Line(cells) => writeln!(
                    out,
                    "{}",
                    cells
                        .iter()
                        .map(|cell| paint(&cell.text, cell.tone, out_term))
                        .collect::<Vec<_>>()
                        .join(" ")
                )?,
                Block::Fields(rows) => write_fields(rows, out, out_term)?,
                Block::Table(table) => {
                    for column in table.write(out, out_term)? {
                        if !hidden.contains(&column) {
                            hidden.push(column);
                        }
                    }
                }
                Block::Note(prefix, text) => note(err, err_term, prefix, text)?,
            }
        }
        if !hidden.is_empty() {
            note(
                err,
                err_term,
                "hint",
                &format!(
                    "hidden {}; widen the terminal or use --json",
                    hidden.join(", ")
                ),
            )?;
        }
        Ok(())
    }
}

fn write_fields(rows: &[(String, Cell)], out: &mut impl Write, term: Term) -> io::Result<()> {
    let width = rows
        .iter()
        .map(|(key, _)| format::display_width(key))
        .max()
        .unwrap_or(0)
        + 2;
    for (key, cell) in rows {
        let padding = " ".repeat(width.saturating_sub(format::display_width(key)));
        let available = term
            .width
            .filter(|_| key != "body")
            .map_or(usize::MAX, |limit| limit.saturating_sub(width).max(1));
        let mut lines = vec![cell.text.clone()];
        if format::display_width(&cell.text) > available {
            lines[0].clear();
            for word in cell.text.split_whitespace() {
                let line = lines.last_mut().expect("one field line exists");
                if !line.is_empty()
                    && format::display_width(line) + 1 + format::display_width(word) > available
                {
                    lines.push(word.into());
                } else {
                    if !line.is_empty() {
                        line.push(' ');
                    }
                    line.push_str(word);
                }
            }
        }
        for (index, line) in lines.iter().enumerate() {
            let prefix = if index == 0 {
                format!("{}{padding}", paint(key, Tone::Dim, term))
            } else {
                " ".repeat(width)
            };
            writeln!(out, "{prefix}{}", paint(line, cell.tone, term))?;
        }
    }
    Ok(())
}

fn note(out: &mut impl Write, term: Term, prefix: &str, text: &str) -> io::Result<()> {
    let tone = match prefix {
        "warning" => Tone::Warn,
        "error" => Tone::Bad,
        _ => Tone::Dim,
    };
    if prefix.is_empty() {
        writeln!(out, "{text}")
    } else {
        writeln!(out, "{} {text}", paint(&format!("{prefix}:"), tone, term))
    }
}

fn paint(text: &str, tone: Tone, term: Term) -> String {
    let code = match tone {
        Tone::Plain => return text.into(),
        Tone::Dim => "2",
        Tone::Bold => "1",
        Tone::Good => "32",
        Tone::Bad => "31",
        Tone::Warn => "33",
    };
    if term.color {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.into()
    }
}

fn clean(text: &str) -> String {
    text.chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect()
}

pub(crate) fn s<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("–")
}
pub(crate) fn f(value: &Value, key: &str) -> f64 {
    value.get(key).and_then(Value::as_f64).unwrap_or(0.0)
}
pub(crate) fn instrument(value: &Value) -> String {
    format::instrument_label(s(value, "exchange"), s(value, "tradingsymbol"))
}
pub(crate) fn scalar(value: &Value) -> String {
    match value {
        Value::String(text) => clean(text),
        Value::Array(items) if !items.is_empty() => {
            items.iter().map(scalar).collect::<Vec<_>>().join(", ")
        }
        Value::Null => "–".into(),
        _ => value.to_string(),
    }
}
pub(crate) fn field(value: &Value, key: &str) -> Cell {
    scalar(&value[key]).into()
}
pub(crate) fn price(value: f64, exchange: &str) -> Cell {
    if matches!(exchange, "CDS" | "BCD") {
        let fixed = format!("{:.4}", value.abs());
        let (whole, fraction) = fixed.split_once('.').unwrap_or((&fixed, "0000"));
        let grouped = whole
            .parse::<u64>()
            .map_or_else(|_| whole.into(), format::group_indian);
        format!(
            "{}{grouped}.{fraction}",
            if value < 0.0 && fixed != "0.0000" {
                "-"
            } else {
                ""
            }
        )
        .into()
    } else {
        format::price(value).into()
    }
}
pub(crate) fn price_field(value: &Value, key: &str) -> Cell {
    nullable_price(&value[key], s(value, "exchange"))
}
pub(crate) fn nullable_price(value: &Value, exchange: &str) -> Cell {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
        .map_or_else(|| "–".into(), |number| price(number, exchange))
}
pub(crate) fn qty(value: i64) -> Cell {
    format!(
        "{}{}",
        if value < 0 { "-" } else { "" },
        format::group_indian(value.unsigned_abs())
    )
    .into()
}
pub(crate) fn qty_field(value: &Value, key: &str) -> Cell {
    value[key]
        .as_i64()
        .map_or_else(|| nullable_price(&value[key], ""), qty)
}
pub(crate) fn change(value: f64) -> Cell {
    if format::price(value) == "0.00" {
        "0.00".into()
    } else {
        Cell::from(format::signed_price(value)).tone(if value > 0.0 {
            Tone::Good
        } else {
            Tone::Bad
        })
    }
}
pub(crate) fn change_pct(value: f64) -> Cell {
    if value.abs() < 0.005 {
        "0.00%".into()
    } else {
        Cell::from(format::percent(value)).tone(if value > 0.0 { Tone::Good } else { Tone::Bad })
    }
}
pub(crate) fn rupees(value: f64) -> String {
    let text = format::price(value);
    text.strip_prefix('-')
        .map_or_else(|| format!("₹{text}"), |number| format!("-₹{number}"))
}
pub(crate) fn status(word: &str) -> Cell {
    Cell::from(word).tone(match word {
        "BUY" | "COMPLETE" | "triggered" => Tone::Good,
        "SELL" | "REJECTED" | "failed" => Tone::Bad,
        "OPEN" | "TRIGGER PENDING" | "AMO REQ RECEIVED" | "active" => Tone::Warn,
        "CANCELLED" | "cancelled" | "expired" | "deleted" => Tone::Dim,
        _ => Tone::Plain,
    })
}

fn flatten(prefix: &str, value: &Value, fields: &mut Vec<(String, Cell)>) {
    match value {
        Value::Object(object) if !object.is_empty() => {
            for (key, child) in object {
                let separator = if prefix.is_empty() { "" } else { "." };
                flatten(&format!("{prefix}{separator}{}", clean(key)), child, fields);
            }
        }
        Value::Array(items) if !items.is_empty() => {
            for (index, child) in items.iter().enumerate() {
                flatten(&format!("{prefix}[{index}]"), child, fields);
            }
        }
        _ => fields.push((prefix.into(), scalar(value).into())),
    }
}

pub(crate) fn records(value: &Value, columns: &[&str], doc: &mut Doc) {
    let value = value
        .get("matches")
        .or_else(|| value.get("records"))
        .unwrap_or(value);
    let rows = value
        .as_array()
        .map_or_else(|| std::slice::from_ref(value), Vec::as_slice);
    if rows.is_empty() {
        doc.empty("No results.");
        return;
    }
    let money = |key: &str| {
        key.ends_with("price")
            || key.ends_with("amount")
            || matches!(
                key,
                "pnl" | "strike" | "tick_size" | "minimum_redemption_quantity"
            )
    };
    let mut table = Table::new(
        columns
            .iter()
            .map(|key| {
                let label = match *key {
                    "tradingsymbol" => "SYMBOL".into(),
                    "exchange" => "EXCH".into(),
                    "quantity" => "QTY".into(),
                    "average_price" => "AVG".into(),
                    "last_price" => "LTP".into(),
                    "pnl" => "P&L".into(),
                    "instrument_token" => "TOKEN".into(),
                    "instrument_type" => "TYPE".into(),
                    "transaction_type" => "SIDE".into(),
                    "lot_size" => "LOT".into(),
                    "tick_size" => "TICK".into(),
                    _ => key.replace('_', " ").to_uppercase(),
                };
                let column = if money(key) || rows.iter().any(|row| row[*key].is_number()) {
                    Col::number(&label)
                } else {
                    Col::text(&label)
                };
                match *key {
                    "tradingsymbol" | "fund" => column.flex(),
                    "order_id" | "instrument_token" | "quantity" | "last_price" | "pnl" => column,
                    "name" => column.flex().drop(3),
                    "average_price" | "exchange" | "transaction_type" | "status" => column.drop(1),
                    _ => column.drop(2),
                }
            })
            .collect(),
    );
    for row in rows {
        table.row(
            columns
                .iter()
                .map(|key| match *key {
                    "quantity" | "completed_instalments" | "lot_size" => qty_field(row, key),
                    _ if money(key) => price_field(row, key),
                    _ => field(row, key),
                })
                .collect(),
        );
    }
    doc.table(table);
}

pub(crate) fn amounts(value: &Value, doc: &mut Doc) {
    fn format_floats(value: &mut Value) {
        match value {
            Value::Object(fields) => fields.values_mut().for_each(format_floats),
            Value::Array(items) => items.iter_mut().for_each(format_floats),
            Value::Number(number) if number.is_f64() => {
                if let Some(number) = number.as_f64() {
                    *value = Value::String(format!("{number:.2}"));
                }
            }
            _ => {}
        }
    }
    let mut value = value.clone();
    format_floats(&mut value);
    generic(&value, doc);
}

pub(crate) fn generic(value: &Value, doc: &mut Doc) {
    if let Some(rows) = value.get("candles").and_then(Value::as_array) {
        if rows.is_empty() {
            doc.empty("No candles.");
            return;
        }
        let mut table = Table::new(vec![
            Col::text("TIME"),
            Col::decimal("OPEN"),
            Col::decimal("HIGH"),
            Col::decimal("LOW"),
            Col::decimal("CLOSE"),
            Col::number("VOLUME"),
            Col::number("OI"),
        ]);
        for row in rows {
            table.row(vec![
                scalar(&row[0]).into(),
                nullable_price(&row[1], ""),
                nullable_price(&row[2], ""),
                nullable_price(&row[3], ""),
                nullable_price(&row[4], ""),
                row[5].as_i64().map_or_else(|| "–".into(), qty),
                row[6].as_i64().map_or_else(|| "–".into(), qty),
            ]);
        }
        doc.table(table);
        return;
    }
    if value.as_array().is_some_and(Vec::is_empty) {
        doc.empty("No results.");
        return;
    }
    if !value.is_object() && !value.is_array() {
        doc.line(vec![scalar(value).into()]);
        return;
    }
    let mut fields = Vec::new();
    flatten("", value, &mut fields);
    doc.blocks.push(Block::Fields(fields));
}

pub(crate) fn path(value: &Value, doc: &mut Doc) {
    doc.line(vec![s(value, "path").into()]);
}

pub(crate) fn error(error: &anyhow::Error, doc: &mut Doc) {
    let message = match error.downcast_ref::<kiteconnect::Error>() {
        Some(kiteconnect::Error::Api(api)) => format!(
            "{} ({}, HTTP {})",
            kiteconnect::error::sanitize_error(&api.message),
            api.kind.as_str(),
            api.status
        ),
        _ => format_error(error),
    };
    doc.blocks.push(Block::Note("error", clean(&message)));
    if error.downcast_ref::<MissingSession>().is_some() || error_type(error) == "TokenException" {
        doc.hint("run `kite auth login` to sign in again");
    }
}

pub(crate) fn profile(value: &Value, doc: &mut Doc) {
    doc.title(&format!(
        "{} ({})",
        s(value, "user_name"),
        s(value, "user_id")
    ));
    doc.fields(
        [
            ("Email", "email"),
            ("Type", "user_type"),
            ("Broker", "broker"),
            ("Exchanges", "exchanges"),
            ("Products", "products"),
            ("Order types", "order_types"),
        ]
        .into_iter()
        .map(|(label, key)| (label, field(value, key)))
        .chain(std::iter::once((
            "Demat",
            field(&value["meta"], "demat_consent"),
        )))
        .collect(),
    );
}

pub(crate) fn margins(value: &Value, segment: Option<&str>, doc: &mut Doc) {
    let segments: Vec<(&str, &Value)> = match segment {
        Some(name) => vec![(name, value)],
        None => ["equity", "commodity"]
            .into_iter()
            .map(|name| (name, &value[name]))
            .collect(),
    };
    let mut cols = vec![Col::text("")];
    cols.extend(
        segments
            .iter()
            .map(|(name, _)| Col::decimal(&name.to_uppercase())),
    );
    let mut table = Table::new(cols);
    for (label, pointer) in [
        ("Available margin", "/net"),
        ("Used margin", "/utilised/debits"),
        ("Opening balance", "/available/opening_balance"),
        ("Cash", "/available/cash"),
        ("Collateral", "/available/collateral"),
        ("Payin", "/available/intraday_payin"),
        ("SPAN", "/utilised/span"),
        ("Exposure", "/utilised/exposure"),
        ("Options premium", "/utilised/option_premium"),
        ("Delivery", "/utilised/delivery"),
        ("M2M realised", "/utilised/m2m_realised"),
        ("M2M unrealised", "/utilised/m2m_unrealised"),
    ] {
        let mut cells = vec![Cell::from(label)];
        cells.extend(segments.iter().map(|(_, segment)| {
            if segment["enabled"] == false {
                "–".into()
            } else {
                segment
                    .pointer(pointer)
                    .and_then(Value::as_f64)
                    .map_or_else(|| "–".into(), |number| price(number, ""))
            }
        }));
        if pointer == "/net" {
            cells = cells
                .into_iter()
                .map(|cell| cell.tone(Tone::Bold))
                .collect();
        }
        table.row(cells);
    }
    doc.table(table);
}

pub(crate) fn positions(value: &Value, doc: &mut Doc) {
    let mut emitted = false;
    for (key, title) in [("net", "Net positions"), ("day", "Day positions")] {
        let Some(rows) = value[key].as_array().filter(|rows| !rows.is_empty()) else {
            doc.empty(&format!("No {key} positions."));
            continue;
        };
        if emitted {
            doc.blank();
        }
        emitted = true;
        doc.title(title);
        let mut table = Table::new(vec![
            Col::text("PRODUCT"),
            Col::text("INSTRUMENT"),
            Col::number("QTY"),
            Col::decimal("AVG").drop(1),
            Col::decimal("LTP"),
            Col::decimal("P&L"),
        ]);
        let mut total = 0.0;
        for row in rows {
            let cells = vec![
                field(row, "product"),
                instrument(row).into(),
                qty_field(row, "quantity"),
                price_field(row, "average_price"),
                price_field(row, "last_price"),
                row["pnl"].as_f64().map_or_else(|| "–".into(), change),
            ];
            if row["quantity"] == 0 {
                table.dim_row(cells);
            } else {
                table.row(cells);
            }
            total += f(row, "pnl");
        }
        table.total(vec![
            "Total".into(),
            "".into(),
            "".into(),
            "".into(),
            "".into(),
            change(total),
        ]);
        doc.table(table);
    }
}

pub(crate) fn time_column(stamps: &[&str]) -> Vec<String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let today = format::datetime_ist_string(now);
    let all_today = stamps
        .iter()
        .all(|stamp| stamp.get(..10) == today.get(..10));
    stamps
        .iter()
        .map(|stamp| {
            if all_today {
                stamp.get(11..19).unwrap_or(stamp).into()
            } else {
                stamp.get(..16).unwrap_or(stamp).into()
            }
        })
        .collect()
}

pub(crate) fn orders(value: &Value, empty: &str, doc: &mut Doc) {
    let Some(rows) = value.as_array().filter(|rows| !rows.is_empty()) else {
        doc.empty(empty);
        return;
    };
    let times = time_column(
        &rows
            .iter()
            .map(|row| s(row, "order_timestamp"))
            .collect::<Vec<_>>(),
    );
    let mut table = Table::new(vec![
        Col::text("TIME"),
        Col::text("SIDE"),
        Col::text("INSTRUMENT"),
        Col::text("TYPE").drop(1),
        Col::text("PRODUCT").drop(2),
        Col::number("QTY"),
        Col::decimal("PRICE"),
        Col::text("STATUS"),
        Col::text("ORDER ID"),
    ]);
    for (row, time) in rows.iter().zip(times) {
        let filled = row["filled_quantity"].as_i64();
        let price = if filled.is_some_and(|quantity| quantity > 0) {
            price_field(row, "average_price")
        } else if s(row, "order_type") == "MARKET" {
            "MKT".into()
        } else {
            price_field(row, "price")
        };
        let cells = vec![
            time.into(),
            status(s(row, "transaction_type")),
            instrument(row).into(),
            field(row, "order_type"),
            field(row, "product"),
            format!(
                "{}/{}",
                qty_field(row, "filled_quantity").text,
                qty_field(row, "quantity").text
            )
            .into(),
            price,
            status(s(row, "status")),
            field(row, "order_id"),
        ];
        if s(row, "status") == "CANCELLED" {
            table.dim_row(cells);
        } else {
            table.row(cells);
        }
    }
    doc.table(table);
    let rejected: Vec<&Value> = rows
        .iter()
        .filter(|row| s(row, "status") == "REJECTED")
        .collect();
    if !rejected.is_empty() {
        doc.blank()
            .line(vec![Cell::from("Rejected orders").tone(Tone::Dim)]);
        for row in rejected {
            let reason = row["status_message"]
                .as_str()
                .or_else(|| row["status_message_raw"].as_str())
                .unwrap_or("–");
            doc.line(vec![format!("{}  {reason}", s(row, "order_id")).into()]);
        }
    }
}

pub(crate) fn alerts(value: &Value, doc: &mut Doc) {
    let rows = value
        .as_array()
        .map_or_else(|| std::slice::from_ref(value), Vec::as_slice);
    if rows.is_empty() {
        doc.empty("No alerts.");
        return;
    }
    let mut table = Table::new(vec![
        Col::text("UUID"),
        Col::text("NAME"),
        Col::text("TYPE").drop(1),
        Col::text("CONDITION").flex(),
        Col::text("STATUS"),
        Col::number("TRIGGERS"),
    ]);
    for row in rows {
        let rhs = if s(row, "rhs_type") == "instrument" {
            format!(
                "{}({}:{})",
                s(row, "rhs_attribute"),
                s(row, "rhs_exchange"),
                s(row, "rhs_tradingsymbol")
            )
        } else {
            nullable_price(&row["rhs_constant"], s(row, "lhs_exchange")).text
        };
        table.row(vec![
            field(row, "uuid"),
            field(row, "name"),
            field(row, "type"),
            format!(
                "{}({}:{}) {} {rhs}",
                s(row, "lhs_attribute"),
                s(row, "lhs_exchange"),
                s(row, "lhs_tradingsymbol"),
                s(row, "operator")
            )
            .into(),
            status(s(row, "status")),
            qty_field(row, "alert_count"),
        ]);
    }
    doc.table(table);
}

pub(crate) fn gtt(value: &Value, doc: &mut Doc) {
    let rows = value
        .as_array()
        .map_or_else(|| std::slice::from_ref(value), Vec::as_slice);
    if rows.is_empty() {
        doc.empty("No GTT triggers.");
        return;
    }
    let mut table = Table::new(vec![
        Col::text("ID"),
        Col::text("INSTRUMENT"),
        Col::text("TYPE"),
        Col::number("TRIGGER"),
        Col::text("ORDERS").flex(),
        Col::text("STATUS"),
        Col::text("CREATED").drop(1),
        Col::text("EXPIRES").drop(2),
    ]);
    let mut failures = Vec::new();
    for row in rows {
        let condition = &row["condition"];
        let exchange = s(condition, "exchange");
        let triggers = condition["trigger_values"].as_array().map_or_else(
            || "–".into(),
            |values| {
                values
                    .iter()
                    .map(|value| {
                        value
                            .as_f64()
                            .map_or_else(|| "–".into(), |number| price(number, exchange).text)
                    })
                    .collect::<Vec<_>>()
                    .join(" / ")
            },
        );
        let orders = row["orders"].as_array().map_or_else(
            || "–".into(),
            |orders| {
                orders
                    .iter()
                    .enumerate()
                    .map(|(index, order)| {
                        if let Some(result) = order
                            .pointer("/result/order_result")
                            .filter(|result| result["status"] == "failed")
                        {
                            failures.push(format!(
                                "{}  leg {}: {}",
                                scalar(&row["id"]),
                                index + 1,
                                s(result, "rejection_reason")
                            ));
                        }
                        format!(
                            "{} {} @ {}",
                            s(order, "transaction_type"),
                            qty_field(order, "quantity").text,
                            price_field(order, "price").text
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" / ")
            },
        );
        table.row(vec![
            field(row, "id"),
            instrument(condition).into(),
            field(row, "type"),
            triggers.into(),
            orders.into(),
            status(s(row, "status")),
            s(row, "created_at").get(..10).unwrap_or("–").into(),
            s(row, "expires_at").get(..10).unwrap_or("–").into(),
        ]);
    }
    doc.table(table);
    if !failures.is_empty() {
        doc.blank()
            .line(vec![Cell::from("Failed GTT orders").tone(Tone::Dim)]);
        for failure in failures {
            doc.line(vec![failure.into()]);
        }
    }
}

fn checklist(doc: &mut Doc, label: &str, pass: Option<bool>, detail: &str) {
    let (glyph, tone) = match pass {
        Some(true) => ("✓", Tone::Good),
        Some(false) => ("✗", Tone::Bad),
        None => ("–", Tone::Dim),
    };
    let detail = if detail.is_empty() {
        if pass == Some(true) { "set" } else { "not set" }
    } else {
        detail
    };
    doc.line(vec![
        Cell::from(glyph).tone(tone),
        format!("{label:<10} {detail}").into(),
    ]);
}

pub(crate) fn doctor(value: &Value, doc: &mut Doc) {
    doc.title(&format!("kite {}", s(value, "version"))).blank();
    let api = &value["api"];
    let config = &value["config"];
    let auth = &value["auth"];
    let exists = config["exists"].as_bool();
    let path = s(config, "path");
    checklist(doc, "Config", exists, path);
    checklist(doc, "API key", Some(!auth["api_key"].is_null()), "");
    let user = auth["user_id"]["present"].as_bool();
    checklist(doc, "User ID", user, "");
    let session = auth["access_token"]["present"] == true;
    let detail = if session { "set" } else { "not logged in" };
    checklist(doc, "Session", Some(session), detail);
    let checked = api["checked"] == true;
    let detail = if checked && api["ok"] == true {
        "/user/margins answered".into()
    } else if checked {
        s(api, "error").to_string()
    } else {
        format!(
            "skipped ({})",
            if s(api, "reason") == "skipped" {
                "--no-live"
            } else {
                s(api, "reason")
            }
        )
    };
    let ok = checked.then(|| api["ok"].as_bool()).flatten();
    checklist(doc, "Kite API", ok, &detail);
    doc.blank().fields(vec![
        ("API root", field(value, "api_root")),
        ("Ticker", field(value, "ticker_url")),
    ]);
    doc.blank().line(vec![
        if value["ok"] == true {
            "All checks passed."
        } else {
            "Setup or API check failed."
        }
        .into(),
    ]);
    if value["setup"]
        .as_array()
        .is_some_and(|steps| steps.iter().any(|step| step == "run `kite auth login`"))
    {
        doc.hint("run `kite auth login`");
    }
}

pub(crate) fn order_summary(value: &Value) -> String {
    if value.get("tradingsymbol").is_none() || value.get("old_product").is_some() {
        return String::new();
    }
    let price = value
        .get("price")
        .map_or_else(String::new, |number| format!(" @ {}", scalar(number)));
    let trigger = value
        .get("trigger_price")
        .map_or_else(String::new, |number| format!(" trigger {}", scalar(number)));
    let validity = value
        .get("validity")
        .map_or_else(String::new, |value| format!("  {}", scalar(value)));
    format!(
        "{} {} {}  {}{price}{trigger}  {}{validity}",
        s(value, "transaction_type"),
        scalar(&value["quantity"]),
        instrument(value),
        s(value, "order_type"),
        s(value, "product"),
    )
}

pub(crate) fn write_result(value: &Value, headline: &str, summary: &str, doc: &mut Doc) {
    if value["dry_run"] == true {
        doc.dry_run(summary, value);
        return;
    }
    doc.success(&if summary.is_empty() {
        headline.into()
    } else {
        format!("{headline}  {summary}")
    });
    if let Some(id) = value.get("order_id") {
        doc.fields(vec![("  Order ID", scalar(id).into())]);
        doc.hint(&format!(
            "check its status with `kite orders get {}`",
            scalar(id)
        ));
    } else if let Some(id) = value.get("trigger_id") {
        doc.fields(vec![("  Trigger ID", scalar(id).into())]);
    } else if !value.is_boolean() && !value.is_null() {
        generic(value, doc);
    }
}

pub(crate) fn tick_header(out: &mut impl Write, symbol_width: usize, term: Term) -> io::Result<()> {
    writeln!(
        out,
        "{}",
        paint(
            &format!(
                "TIME      {:<symbol_width$}  {:>10}  {:>10}  {:>8}  {:>12}  {:>10}  {:>10}",
                "INSTRUMENT", "LTP", "CHG", "CHG%", "VOLUME", "BUY QTY", "SELL QTY"
            ),
            Tone::Dim,
            term
        )
    )
}

pub(crate) fn tick_line(
    out: &mut impl Write,
    tick: &kiteconnect::ticker::Tick,
    symbol: &str,
    symbol_width: usize,
    received: u64,
    term: Term,
) -> io::Result<()> {
    let symbol = clean(symbol);
    let symbol_text = format::pad_or_clip(&symbol, symbol_width);
    let exchange = symbol.split_once(':').map_or_else(
        || match tick.instrument_token & 0xff {
            3 => "CDS",
            6 => "BCD",
            _ => "",
        },
        |(exchange, _)| exchange,
    );
    let last = tick
        .last_price
        .map_or_else(|| "–".into(), |number| price(number, exchange));
    let delta = tick
        .last_price
        .zip(tick.close)
        .map(|(last, close)| last - close);
    let change = delta.map_or_else(|| "–".into(), change);
    let percent = delta
        .zip(tick.close)
        .filter(|(_, close)| *close > 0.0)
        .map_or_else(
            || "–".into(),
            |(delta, close)| change_pct(delta / close * 100.0),
        );
    let volume = |number: Option<u64>| number.map_or_else(|| "–".into(), format::group_indian);
    let padded = |cell: &Cell, width: usize| {
        format!(
            "{}{}",
            " ".repeat(width.saturating_sub(format::display_width(&cell.text))),
            paint(&cell.text, cell.tone, term)
        )
    };
    writeln!(
        out,
        "{}  {symbol_text}  {}  {}  {}  {:>12}  {:>10}  {:>10}",
        format::clock_ist(tick.exchange_time.map_or(received, u64::from)),
        padded(&last, 10usize),
        padded(&change, 10usize),
        padded(&percent, 8usize),
        volume(tick.volume),
        volume(tick.buy_quantity),
        volume(tick.sell_quantity)
    )?;
    out.flush()
}

pub(crate) struct Status(bool);

impl Status {
    pub fn start(message: &str, json: bool) -> Self {
        let active = !json && io::stderr().is_terminal();
        if active {
            let _ = write!(io::stderr().lock(), "{}", clean(message));
            let _ = io::stderr().flush();
        }
        Self(active)
    }
}

impl Drop for Status {
    fn drop(&mut self) {
        if self.0 {
            let _ = write!(io::stderr().lock(), "\r\x1b[K");
        }
    }
}

pub(crate) fn prompt(message: &str) -> io::Result<()> {
    write!(
        io::stderr().lock(),
        "{}",
        paint(&clean(message), Tone::Bold, Term::stderr())
    )?;
    io::stderr().flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticker_fixture_prints_a_fixed_width_line_and_reuses_quote_arithmetic() {
        let fixture: Value =
            serde_json::from_str(include_str!("../../mock_responses/ticker_quote.json")).unwrap();
        let mut tick = kiteconnect::ticker::Tick::default();
        tick.instrument_token = 408_065;
        tick.last_price = fixture["last_price"].as_f64();
        tick.close = fixture["ohlc"]["close"].as_f64();
        tick.volume = fixture["volume"].as_u64();
        tick.buy_quantity = fixture["buy_quantity"].as_u64();
        tick.sell_quantity = fixture["sell_quantity"].as_u64();
        let mut stdout = Vec::new();
        tick_line(&mut stdout, &tick, "NSE:INFY", 10, 0, Term::default()).unwrap();
        assert_eq!(
            String::from_utf8(stdout).unwrap(),
            "05:30:00  NSE:INFY      1,573.15       +5.35    +0.34%     11,75,986    2,56,511    3,60,503\n"
        );
    }

    #[test]
    fn numeric_currency_ticks_keep_segment_precision() {
        for token in [259, 262] {
            let mut tick = kiteconnect::ticker::Tick::default();
            tick.instrument_token = token;
            tick.last_price = Some(72.1234);
            let mut out = Vec::new();
            tick_line(&mut out, &tick, &token.to_string(), 10, 0, Term::default()).unwrap();
            assert_eq!(
                String::from_utf8(out).unwrap(),
                format!(
                    "05:30:00  {token:<10}     72.1234           –         –             –           –           –\n"
                )
            );
        }
    }

    #[test]
    fn narrow_fields_keep_every_word_and_sanitize_labels() {
        let mut doc = Doc::default();
        doc.fields(vec![("a\u{1b}[31m", "one two three four".into())]);
        let mut out = Vec::new();
        let mut err = Vec::new();
        doc.write(
            &mut out,
            Term {
                width: Some(15),
                color: false,
            },
            &mut err,
            Term::default(),
        )
        .unwrap();
        assert_eq!(out, b"a [31m  one two\n        three\n        four\n");
        assert_eq!(err, b"");
    }

    #[test]
    fn narrow_dry_runs_preserve_whitespace_inside_json_strings() {
        let mut doc = Doc::default();
        doc.dry_run("", &serde_json::json!({"method":"POST", "path":"/raw", "body":{"json":{"text":"two  spaces", "array":[1,true,null]}}}));
        let mut out = Vec::new();
        let mut err = Vec::new();
        doc.write(
            &mut out,
            Term {
                width: Some(20),
                color: false,
            },
            &mut err,
            Term::default(),
        )
        .unwrap();
        assert_eq!(out, b"DRY RUN  nothing was sent to Kite\n\nPOST /raw\nbody  {\"array\":[1,true,null],\"text\":\"two  spaces\"}\n");
        assert_eq!(err, b"");
    }

    #[test]
    fn trade_prices_use_exchange_precision_and_numeric_columns_align_right() {
        let mut doc = Doc::default();
        records(
            &serde_json::json!([
                {"exchange":"CDS","quantity":2,"average_price":72.755},
                {"exchange":"MCX","quantity":1001,"average_price":4852}
            ]),
            &["exchange", "quantity", "average_price"],
            &mut doc,
        );
        let mut out = Vec::new();
        let mut err = Vec::new();
        doc.write(&mut out, Term::default(), &mut err, Term::default())
            .unwrap();
        assert_eq!(
            out,
            b"EXCH    QTY       AVG\nCDS       2   72.7550\nMCX   1,001  4,852.00\n"
        );
        assert_eq!(err, b"");
    }

    #[test]
    fn only_typed_session_errors_get_login_hints() {
        for (kind, hint) in [
            (kiteconnect::error::ErrorKind::Token, true),
            (kiteconnect::error::ErrorKind::Input, false),
        ] {
            let failure = anyhow::Error::from(kiteconnect::Error::Api(
                kiteconnect::error::ApiError::new(kind, "denied", 403, None),
            ));
            let mut doc = Doc::default();
            error(&failure, &mut doc);
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            doc.write(&mut stdout, Term::default(), &mut stderr, Term::default())
                .unwrap();
            assert_eq!(stdout, b"");
            assert_eq!(
                String::from_utf8(stderr).unwrap(),
                if hint {
                    "error: denied (TokenException, HTTP 403)\nhint: run `kite auth login` to sign in again\n"
                } else {
                    "error: denied (InputException, HTTP 403)\n"
                }
            );
        }
    }

    #[test]
    fn zero_changes_are_plain_and_quantities_and_ids_stay_distinct() {
        assert_eq!(change(-0.001).text, "0.00");
        assert_eq!(change_pct(-0.001).text, "0.00%");
        assert!(change(-0.001).tone == Tone::Plain);
        assert_eq!(qty(-7_360_198).text, "-73,60,198");
        assert_eq!(
            field(
                &serde_json::json!({"id": 1_953_367_517_686_685_697_u64}),
                "id"
            )
            .text,
            "1953367517686685697"
        );
    }

    #[test]
    fn color_requires_a_terminal_and_respects_no_color() {
        assert!(use_color(true, false, false));
        assert!(!use_color(false, false, false));
        assert!(!use_color(true, true, false));
        assert!(!use_color(true, false, true));
    }

    #[test]
    fn generic_fields_escape_controls_and_distinguish_null_from_zero() {
        let mut doc = Doc::default();
        generic(
            &serde_json::json!({"nested":{"empty":null,"zero":0,"name":"a\u{1b}[31m\nb"}}),
            &mut doc,
        );
        let mut out = Vec::new();
        let mut err = Vec::new();
        doc.write(&mut out, Term::default(), &mut err, Term::default())
            .unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "nested.empty  –\nnested.name   a [31m b\nnested.zero   0\n"
        );
        assert_eq!(err, b"");
    }

    #[test]
    fn narrow_tables_drop_ranked_columns_and_keep_ids() {
        let mut table = Table::new(vec![
            Col::text("ID"),
            Col::text("TYPE").drop(1),
            Col::text("PRODUCT").drop(2),
        ]);
        table.row(vec!["123456789".into(), "LIMIT".into(), "CNC".into()]);
        let mut doc = Doc::default();
        doc.table(table);
        let mut out = Vec::new();
        let mut err = Vec::new();
        doc.write(
            &mut out,
            Term {
                width: Some(10),
                color: false,
            },
            &mut err,
            Term::default(),
        )
        .unwrap();
        assert_eq!(out, b"ID\n123456789\n");
        assert_eq!(
            err,
            b"hint: hidden PRODUCT, TYPE; widen the terminal or use --json\n"
        );
    }
    #[test]
    fn a_table_that_cannot_fit_keeps_the_symbol_in_vertical_fields() {
        let mut doc = Doc::default();
        records(
            &serde_json::json!([{"tradingsymbol":"A_LONG_SYMBOL","order_id":"123456789"}]),
            &["tradingsymbol", "order_id"],
            &mut doc,
        );
        let mut out = Vec::new();
        let mut err = Vec::new();
        doc.write(
            &mut out,
            Term {
                width: Some(12),
                color: false,
            },
            &mut err,
            Term::default(),
        )
        .unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("SYMBOL"), "{text}");
        assert!(text.contains("ORDER ID"), "{text}");
        assert!(
            text.lines()
                .map(str::trim)
                .collect::<String>()
                .contains("A_LONG_SYMBOL"),
            "{text}"
        );
    }
}
