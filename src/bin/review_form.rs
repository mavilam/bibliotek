use std::{fs, path::PathBuf};

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

#[derive(Clone)]
struct AppState {
    reviews_dir: PathBuf,
}

#[derive(Serialize)]
struct Folder {
    path: String,
    name: String,
}

#[derive(Deserialize)]
struct SaveReviewRequest {
    title: String,
    author: String,
    year_published: u32,
    date_read: String,
    rating: u8,
    pages: u32,
    tags: Vec<String>,
    review_text: String,
    folder: String,
}

#[derive(Serialize)]
struct SaveReviewResponse {
    success: bool,
    filename: String,
    path: String,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

fn get_folders(reviews_dir: &PathBuf) -> Vec<Folder> {
    let mut folders = vec![Folder {
        path: String::new(),
        name: "reviews/".to_string(),
    }];

    if let Ok(entries) = std::fs::read_dir(reviews_dir) {
        let mut subdirs: Vec<_> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .filter(|e| {
                !e.file_name()
                    .to_str()
                    .map(|s| s.starts_with('.'))
                    .unwrap_or(true)
            })
            .collect();
        subdirs.sort_by_key(|e| e.file_name());

        for entry in subdirs {
            if let Some(name) = entry.file_name().to_str() {
                folders.push(Folder {
                    path: name.to_string(),
                    name: format!("reviews/{}/", name),
                });
            }
        }
    }

    folders
}

fn slugify(text: &str) -> String {
    text.nfkd()
        .filter(|c| c.is_ascii())
        .collect::<String>()
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' {
                c
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("_")
}

async fn index(State(state): State<AppState>) -> Html<String> {
    let folders = get_folders(&state.reviews_dir);
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();

    let html = fs::read_to_string("review_form/templates/index.html")
        .unwrap_or_else(|_| "Template not found".to_string());

    let folders_html: String = folders
        .iter()
        .map(|f| format!(r#"<option value="{}">{}</option>"#, f.path, f.name))
        .collect();

    let html = html
        .replace("{{ folders_options }}", &folders_html)
        .replace("{{ today }}", &today);

    Html(html)
}

async fn api_folders(State(state): State<AppState>) -> impl IntoResponse {
    let folders = get_folders(&state.reviews_dir);
    axum::Json(folders)
}

async fn save_review(
    State(state): State<AppState>,
    axum::Json(req): axum::Json<SaveReviewRequest>,
) -> Response {
    if req.title.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            axum::Json(ErrorResponse {
                error: "Title is required".to_string(),
            }),
        )
            .into_response();
    }
    if req.author.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            axum::Json(ErrorResponse {
                error: "Author is required".to_string(),
            }),
        )
            .into_response();
    }
    if req.date_read.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            axum::Json(ErrorResponse {
                error: "Date read is required".to_string(),
            }),
        )
            .into_response();
    }

    let tags_yaml = if req.tags.is_empty() {
        String::new()
    } else {
        req.tags
            .iter()
            .map(|t| format!(r#" - "{}""#, t))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let content = format!(
        r#"---
title: "{}"
author: "{}"
year_published: {}
date_read: {}
rating: {}
pages: {}
tags: 
{}
---
{}"#,
        req.title.trim(),
        req.author.trim(),
        req.year_published,
        req.date_read,
        req.rating,
        req.pages,
        tags_yaml,
        req.review_text.trim()
    );

    let slug = slugify(&req.title);
    let filename = format!("{}_{}.md", req.date_read, slug);

    let filepath = if req.folder.is_empty() {
        state.reviews_dir.join(&filename)
    } else {
        state.reviews_dir.join(&req.folder).join(&filename)
    };

    if filepath.exists() {
        return (
            StatusCode::BAD_REQUEST,
            axum::Json(ErrorResponse {
                error: format!("File already exists: {}", filename),
            }),
        )
            .into_response();
    }

    if let Some(parent) = filepath.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(ErrorResponse {
                    error: format!("Could not create directory: {}", e),
                }),
            )
                .into_response();
        }
    }

    if let Err(e) = std::fs::write(&filepath, content) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            axum::Json(ErrorResponse {
                error: format!("Could not write file: {}", e),
            }),
        )
            .into_response();
    }

    let relative_path = if req.folder.is_empty() {
        format!("reviews/{}", filename)
    } else {
        format!("reviews/{}/{}", req.folder, filename)
    };

    axum::Json(SaveReviewResponse {
        success: true,
        filename,
        path: relative_path,
    })
    .into_response()
}

#[tokio::main]
async fn main() {
    let reviews_dir = PathBuf::from("./reviews");

    if !reviews_dir.exists() {
        eprintln!("Error: reviews directory not found at {:?}", reviews_dir);
        eprintln!("Make sure to run this from the bibliotek root directory.");
        std::process::exit(1);
    }

    let state = AppState { reviews_dir };

    let app = Router::new()
        .route("/", get(index))
        .route("/api/folders", get(api_folders))
        .route("/api/save", post(save_review))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8787")
        .await
        .unwrap();

    println!("Review form running at http://localhost:8787");
    println!("Press Ctrl+C to stop.");

    axum::serve(listener, app).await.unwrap();
}
