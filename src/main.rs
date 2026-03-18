use std::{
    cmp::Reverse,
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use gray_matter::{Matter, ParsedEntity, engine::YAML};
use pulldown_cmark::{Parser, html};
use serde::{Deserialize, Serialize};
use tera::{Context, Tera};
use walkdir::WalkDir;

// ============================================================
// Models
// ============================================================

#[derive(Deserialize, Debug)]
struct ReviewMetadata {
    title: String,
    author: String,
    year_published: u32,
    date_read: String,
    rating: u8,
    tags: Vec<String>,
}

#[derive(Deserialize, Debug)]
struct SectionMetadata {
    title: String,
    topic: Option<String>,
    tag: String,
}

#[derive(Debug, Clone, Serialize)]
struct ReviewInfo {
    title: String,
    author: String,
    year_published: u32,
    year_read: u32,
    date_read: String,
    rating: u8,
    tags: Vec<String>,
    path: String,
    filename: String,
}

impl ReviewInfo {
    fn new(path: String, filename: String, metadata: ReviewMetadata) -> Self {
        let year_read = metadata
            .date_read
            .split('-')
            .nth(0)
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1993);
        Self {
            title: metadata.title,
            author: metadata.author,
            year_published: metadata.year_published,
            year_read,
            date_read: metadata.date_read,
            rating: metadata.rating,
            tags: metadata.tags,
            path,
            filename,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct SectionInfo {
    name: String,
    topic: String,
    path: String,
    reviews: Vec<ReviewInfo>,
}

const BASE_URL: &str = "https://mavilam.github.io/bibliotek";
// Until 2023 I was not consistent writing up reviews.
const START_YEAR: u32 = 2023;

static OUTPUT_DIR: OnceLock<PathBuf> = OnceLock::new();

static INPUT_DIR: OnceLock<PathBuf> = OnceLock::new();

fn output_dir() -> &'static Path {
    OUTPUT_DIR.get_or_init(|| PathBuf::from("output")).as_path()
}

fn input_dir() -> &'static Path {
    INPUT_DIR
        .get_or_init(|| PathBuf::from("./reviews"))
        .as_path()
}

// ============================================================
// Main Function
// ============================================================

fn main() {
    let tera = Tera::new("templates/**/*.html").expect("Error loading templates");

    fs::create_dir_all(output_dir()).expect("Could not create output directory");

    // Copy the stylesheet into the output directory
    fs::copy("templates/index.css", output_dir().join("index.css"))
        .expect("Could not copy index.css to output");

    // Copy all the assets into the output directory
    let assets_out = output_dir().join("assets");
    fs::create_dir_all(&assets_out).expect("Could not create assets directory");
    for entry in fs::read_dir("templates/assets").expect("Could not read assets directory") {
        let entry = entry.expect("Could not read assets entry");
        fs::copy(entry.path(), assets_out.join(entry.file_name()))
            .expect("Could not copy asset file");
    }

    let reviews_root = PathBuf::from(input_dir());
    let mut sections_map: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
    for entry in WalkDir::new(&reviews_root) {
        match entry {
            Ok(entry) => {
                let path = entry.path();
                if path.is_dir() && !sections_map.contains_key(path) {
                    sections_map.insert(path.to_path_buf(), vec![]);
                } else if path.is_file() && !path.ends_with("index.md") {
                    let current_dir = path.parent().unwrap().to_path_buf();
                    sections_map
                        .get_mut(&current_dir)
                        .unwrap()
                        .push(path.to_path_buf());
                }
            }
            Err(err) => eprintln!("There was an error: {err}"),
        }
    }

    let mut all_reviews: Vec<ReviewInfo> = Vec::new();
    let mut all_sections: Vec<SectionInfo> = Vec::new();

    for (section, reviews) in &sections_map {
        let mut infos = Vec::new();
        for review in reviews {
            if let Some(info) = render_review(review, &tera) {
                infos.push(info);
            }
        }
        if *section != input_dir() {
            let section_entrance = section.join("index.md");
            if let Some(section_info) = render_section(&section_entrance, &tera, &infos) {
                all_sections.push(section_info);
            }
        }
        all_reviews.extend(infos);
    }

    // Render main entrance
    render_all_reviews_section(&tera, &all_reviews);
    render_analytics(&tera, &all_reviews);
    render_index(&tera, &all_sections);
}

// ============================================================
// Rendering Functions
// ============================================================

fn render_index(tera: &Tera, all_sections: &[SectionInfo]) {
    let css_path = css_path_for_output(input_dir());
    let raw = fs::read_to_string(input_dir().join("index.md")).unwrap();
    let matter = Matter::<YAML>::new();
    let result: ParsedEntity<SectionMetadata> = matter.parse(&raw).unwrap();
    let Some(metadata) = result.data else {
        println!("{} had no content", input_dir().join("index.md").display());
        return;
    };

    let body_html = markdown_to_html(&result.content);
    let page_url = format!("{}/", BASE_URL);
    let mut context = Context::new();
    context.insert("css_path", &css_path);
    context.insert("base_url", BASE_URL);
    context.insert("page_url", &page_url);
    context.insert("title", &metadata.title);
    context.insert("tag", &metadata.tag);
    context.insert("content", &body_html);
    context.insert("sections", &all_sections);
    let content = tera
        .render("index.html", &context)
        .expect("Error rendering template");
    write_file(&input_dir().join("index.html"), content);
}

// This is not present in the reviews directory, but it is a section that contains all the reviews
// and will be linked to from the index page.
fn render_all_reviews_section(tera: &Tera, dir_reviews: &[ReviewInfo]) {
    let grouped = group_reviews_by_year(dir_reviews);
    let css_path = css_path_for_output(input_dir());
    let page_url = format!("{}/all_reviews.html", BASE_URL);
    let mut context = Context::new();
    context.insert("css_path", &css_path);
    context.insert("base_url", BASE_URL);
    context.insert("page_url", &page_url);
    context.insert("title", "Reseñas");
    context.insert("dir_reviews", &grouped);

    let content = tera
        .render("all_reviews.html", &context)
        .expect("Error rendering template");
    write_file(&input_dir().join("all_reviews.html"), content);
}

fn render_section(path: &Path, tera: &Tera, dir_reviews: &[ReviewInfo]) -> Option<SectionInfo> {
    let css_path = css_path_for_output(path);
    let raw = fs::read_to_string(path).unwrap();
    let matter = Matter::<YAML>::new();

    let result: ParsedEntity<SectionMetadata> = matter.parse(&raw).unwrap();
    let Some(metadata) = result.data else {
        println!("{} had no content", path.display());
        return None;
    };
    let body_html = markdown_to_html(&result.content);

    let mut local_reviews: Vec<ReviewInfo> = dir_reviews
        .iter()
        .map(|r| ReviewInfo {
            path: r.filename.clone(),
            ..r.clone()
        })
        .collect();
    local_reviews.sort_by_key(|r| Reverse(r.date_read.clone()));

    let section_topic = metadata.topic.unwrap_or_default();
    let page_url = format!("{}/{}", BASE_URL, relative_html_path(path));
    let mut context = Context::new();
    context.insert("css_path", &css_path);
    context.insert("base_url", BASE_URL);
    context.insert("page_url", &page_url);
    context.insert("title", &metadata.title);
    context.insert("topic", &section_topic);
    context.insert("tag", &metadata.tag);
    context.insert("content", &body_html);
    context.insert("dir_reviews", &local_reviews);

    let content = tera
        .render("section.html", &context)
        .expect("Error rendering template");
    write_file(&path, content);

    let relative_path = relative_html_path(path);

    Some(SectionInfo {
        name: metadata.title,
        topic: section_topic,
        path: format!("./{}", relative_path),
        reviews: dir_reviews.to_vec(),
    })
}

fn render_review(path: &Path, tera: &Tera) -> Option<ReviewInfo> {
    let css_path = css_path_for_output(path);
    let raw = fs::read_to_string(path).unwrap();
    let matter = Matter::<YAML>::new();

    let result: Result<ParsedEntity<ReviewMetadata>, gray_matter::Error> = matter.parse(&raw);
    if let Err(error) = result {
        // If there is a file that cannot be parsed, it panics to surface the error
        panic!("Error parsing review {}: {error:?}", path.display());
    };
    let result = result.unwrap();
    let Some(metadata) = result.data else {
        println!("{} had no content", path.display());
        return None;
    };
    let body_html = markdown_to_html(&result.content);

    let page_url = format!("{}/{}", BASE_URL, relative_html_path(path));
    let mut context = Context::new();
    context.insert("css_path", &css_path);
    context.insert("base_url", BASE_URL);
    context.insert("page_url", &page_url);
    context.insert("title", &metadata.title);
    context.insert("author", &metadata.author);
    context.insert("year_published", &metadata.year_published);
    context.insert("date_read", &metadata.date_read);
    context.insert("rating", &metadata.rating);
    context.insert("tags", &metadata.tags);
    context.insert("content", &body_html);

    let content = tera
        .render("review.html", &context)
        .expect("Error rendering template");

    write_file(path, content);

    let filename = path
        .with_extension("html")
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_else(|| path.to_str().unwrap_or(""))
        .to_owned();
    let path_str = relative_html_path(path);

    Some(ReviewInfo::new(path_str, filename, metadata))
}

fn render_analytics(tera: &Tera, reviews: &[ReviewInfo]) {
    let css_path = css_path_for_output(input_dir());
    let page_url = format!("{}/analytics.html", BASE_URL);

    let total_books = reviews.len();
    let current_year = reviews
        .iter()
        .map(|r| r.year_read)
        .max()
        .unwrap_or(START_YEAR);

    let (per_year_labels, per_year_counts, per_year_avgs) =
        books_per_year(reviews, current_year);
    let (pub_decade_labels, pub_decade_counts) = books_per_decade(reviews);
    let (tag_labels, tag_counts) = top_tags(reviews);

    let mut context = Context::new();
    context.insert("css_path", &css_path);
    context.insert("base_url", BASE_URL);
    context.insert("page_url", &page_url);
    context.insert("total_books", &total_books);
    context.insert("first_year", &START_YEAR);
    context.insert("last_year", &current_year);
    context.insert("per_year_labels", &per_year_labels);
    context.insert("per_year_counts", &per_year_counts);
    context.insert("per_year_avgs", &per_year_avgs);
    context.insert("pub_decade_labels", &pub_decade_labels);
    context.insert("pub_decade_counts", &pub_decade_counts);
    context.insert("tag_labels", &tag_labels);
    context.insert("tag_counts", &tag_counts);

    let content = tera
        .render("stats.html", &context)
        .expect("Error rendering analytics template");
    write_file(&input_dir().join("stats.html"), content);
}

// ============================================================
// Analytics Helper Functions
// ============================================================

fn books_per_year(
    reviews: &[ReviewInfo],
    current_year: u32,
) -> (Vec<String>, Vec<usize>, Vec<String>) {
    let mut by_year: BTreeMap<u32, Vec<u8>> = BTreeMap::new();
    for r in reviews {
        by_year.entry(r.year_read).or_default().push(r.rating);
    }
    by_year.retain(|y, _| *y >= START_YEAR && *y <= current_year);
    let labels = by_year.keys().map(|y| y.to_string()).collect();
    let counts = by_year.values().map(|v| v.len()).collect();
    let avgs = by_year
        .values()
        .map(|v| {
            let avg = v.iter().map(|&r| r as f64).sum::<f64>() / v.len() as f64;
            format!("{:.2}", avg)
        })
        .collect();
    (labels, counts, avgs)
}

fn books_per_decade(reviews: &[ReviewInfo]) -> (Vec<String>, Vec<usize>) {
    let mut by_decade: BTreeMap<u32, usize> = BTreeMap::new();
    for r in reviews {
        // Integer division truncates toward zero (e.g. 1993 → 1990)
        let decade = (r.year_published / 10) * 10;
        *by_decade.entry(decade).or_default() += 1;
    }
    let labels = by_decade.keys().map(|d| d.to_string()).collect();
    let counts = by_decade.values().copied().collect();
    (labels, counts)
}

fn top_tags(reviews: &[ReviewInfo]) -> (Vec<String>, Vec<usize>) {
    let mut tag_map: BTreeMap<String, usize> = BTreeMap::new();
    for r in reviews {
        for tag in &r.tags {
            *tag_map.entry(tag.clone()).or_default() += 1;
        }
    }
    let mut tag_vec: Vec<(String, usize)> = tag_map.into_iter().collect();
    tag_vec.sort_by(|a, b| b.1.cmp(&a.1));
    tag_vec.truncate(12);
    // Chart.js horizontal bars look best with highest value at the top,
    // but for readability with indexAxis:'y' we reverse so the largest
    // bar appears at the top of the canvas.
    tag_vec.reverse();
    let labels = tag_vec.iter().map(|(t, _)| t.clone()).collect();
    let counts = tag_vec.iter().map(|(_, c)| *c).collect();
    (labels, counts)
}

// ============================================================
// Helper Functions
// ============================================================

fn markdown_to_html(markdown: &str) -> String {
    let parser = Parser::new(markdown);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}

fn write_file(path: &Path, rendered: String) {
    // Mirror the reviews/ directory structure inside output/
    let relative = path.strip_prefix(input_dir()).unwrap_or(path);
    let dest = output_dir().join(relative).with_extension("html");

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).expect("Could not create subdirectory");
    }

    fs::write(&dest, rendered).unwrap();
    println!("Generated: {}", dest.display());
}

fn css_path_for_output(path: &Path) -> String {
    let relative = path.strip_prefix(input_dir()).unwrap_or(path);
    let depth = relative.parent().map_or(0, |p| p.components().count());
    std::iter::repeat("../")
        .take(depth)
        .chain(std::iter::once("index.css"))
        .collect()
}

fn relative_html_path(path: &Path) -> String {
    path.strip_prefix(input_dir())
        .unwrap_or(path)
        .with_extension("html")
        .to_string_lossy()
        .into_owned()
}

fn group_reviews_by_year(reviews: &[ReviewInfo]) -> Vec<(u32, Vec<ReviewInfo>)> {
    let mut by_year: BTreeMap<u32, Vec<ReviewInfo>> = BTreeMap::new();
    for review in reviews {
        by_year
            .entry(review.year_read)
            .or_default()
            .push(review.clone());
    }
    let grouped: Vec<(u32, Vec<ReviewInfo>)> = by_year
        .into_iter()
        .rev()
        .map(|(year, mut reviews)| {
            reviews.sort_by_key(|r| Reverse(r.date_read.clone()));
            (year, reviews)
        })
        .collect();
    grouped
}
