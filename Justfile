set shell := ["zsh", "-cu"]

# Run the CSV -> Markdown converter.
csv-to-reviews input="goodreads_library_export.csv" output="reviews":
    cargo run --bin csv_to_reviews -- "{{input}}" "{{output}}"

# Build the HTML site from reviews.
build-site:
    cargo run --bin bibliotek

# Open the generated site in the default browser.
open:
    open output/index.html
