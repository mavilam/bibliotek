use std::{env, fs, path::PathBuf};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct GoodreadsRow {
    #[serde(rename = "Title")]
    title: String,
    #[serde(rename = "Author")]
    author: String,
    #[serde(rename = "My Rating")]
    my_rating: String,
    #[serde(rename = "Number of Pages")]
    number_of_pages: String,
    #[serde(rename = "Original Publication Year")]
    original_publication_year: String,
    #[serde(rename = "Date Read")]
    date_read: String,
    #[serde(rename = "Exclusive Shelf")]
    exclusive_shelf: String,
    #[serde(rename = "My Review")]
    my_review: String,
}

impl GoodreadsRow {
    fn build_review_markdown(&self) -> Option<String> {
        let year = parse_i32(&self.original_publication_year).unwrap_or_default();
        let rating = parse_u8(&self.my_rating).unwrap_or_default();
        let pages = parse_i32(&self.number_of_pages).unwrap_or_default();
        let date_read = normalize_date(&self.date_read);
        let Some(review_body) = normalize_review_text(&self.my_review) else {
            return None;
        };

        Some(format!(
            "---\n\
    title: \"{}\"\n\
    author: \"{}\"\n\
    year_published: {}\n\
    date_read: {}\n\
    rating: {}\n\
    pages: {}\n\
    tags: []\n\
    ---\n\n\
    {}\n",
            yaml_escape(&self.title),
            yaml_escape(&self.author),
            year,
            date_read,
            rating,
            pages,
            review_body
        ))
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let input_csv = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("library_example.csv"));
    let output_dir = args
        .get(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("reviews"));

    if !input_csv.exists() {
        return Err(format!("CSV file not found: {}", input_csv.display()).into());
    }

    fs::create_dir_all(&output_dir)?;

    let mut reader = csv::Reader::from_path(&input_csv)?;
    let mut written_files = 0usize;

    for result in reader.deserialize::<GoodreadsRow>() {
        let row = result?;

        if !row.exclusive_shelf.eq_ignore_ascii_case("read") {
            continue;
        }

        let filename = review_filename(&row.title, &row.date_read);
        let output_path = output_dir.join(filename);

        let Some(markdown) = row.build_review_markdown() else {
            continue;
        };

        fs::write(&output_path, markdown)?;
        written_files += 1;
    }

    println!(
        "Generated {} markdown reviews in {}",
        written_files,
        output_dir.display()
    );

    Ok(())
}

fn review_filename(title: &str, date_read: &str) -> String {
    let safe_date = normalize_date(date_read);
    let slug = slugify(title);
    format!("{}_{}.md", safe_date, slug)
}

fn slugify(input: &str) -> String {
    let mut out = String::new();
    let mut prev_was_sep = false;

    for ch in input.trim().to_lowercase().chars() {
        if ch.is_alphanumeric() {
            out.push(ch);
            prev_was_sep = false;
        } else if !prev_was_sep {
            out.push('_');
            prev_was_sep = true;
        }
    }

    while out.ends_with('_') {
        out.pop();
    }
    while out.starts_with('_') {
        out.remove(0);
    }

    out
}

fn normalize_date(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        "1970-01-01".to_string()
    } else {
        trimmed.replace('/', "-")
    }
}

fn normalize_review_text(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(
            trimmed
                .replace("<br/><br/>", "\n\n")
                .replace("<br />", "\n")
                .replace("<br/>", "\n"),
        )
    }
}

fn yaml_escape(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}

fn parse_i32(input: &str) -> Option<i32> {
    let cleaned = strip_wrapped_quotes(input).replace(',', "");
    cleaned.trim().parse::<i32>().ok()
}

fn parse_u8(input: &str) -> Option<u8> {
    let cleaned = strip_wrapped_quotes(input).replace(',', "");
    cleaned.trim().parse::<u8>().ok()
}

fn strip_wrapped_quotes(input: &str) -> String {
    let s = input.trim();
    let s = s.strip_prefix("=\"").unwrap_or(s);
    let s = s.strip_suffix('"').unwrap_or(s);
    s.to_string()
}
