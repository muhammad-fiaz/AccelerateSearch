//! AccelerateSearch benchmark suite
//!
//! Run with:
//!     cargo run --release
//!
//! Results are written to `benchmark-result.d/` as JSON files.

use std::fs;
use std::path::Path;
use std::time::Duration;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};

#[allow(dead_code)]
#[derive(serde::Serialize)]
struct BenchResult {
    name: String,
    mean_ns: f64,
    std_dev_ns: f64,
    throughput: f64,
}

#[allow(dead_code)]
fn save_results(results: &[BenchResult]) {
    let dir = Path::new("benchmark-result.d");
    fs::create_dir_all(dir).ok();

    let timestamp = format!(
        "{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    );
    let filename = dir.join(format!("bench_{}.json", timestamp));

    let json = serde_json::to_string_pretty(results).unwrap_or_default();

    fs::write(&filename, json).ok();
    println!("Results written to: {}", filename.display());
}

fn bench_tokenize(c: &mut Criterion) {
    let mut group = c.benchmark_group("indexing/tokenize");
    let corpus = create_corpus();
    group.throughput(Throughput::Elements(corpus.len() as u64));
    group.bench_function("tokenize_docs", |b| {
        b.iter(|| {
            for doc in &corpus {
                let _tokens: Vec<String> = doc
                    .split_whitespace()
                    .map(|w| w.to_lowercase())
                    .collect();
            }
        });
    });
    group.finish();
}

fn bench_index(c: &mut Criterion) {
    let mut group = c.benchmark_group("indexing/build");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(15));
    group.bench_function("build_index", |b| {
        let corpus = create_corpus();
        b.iter(|| {
            let mut index: std::collections::HashMap<String, Vec<usize>> = std::collections::HashMap::new();
            for (i, doc) in corpus.iter().enumerate() {
                for word in doc.split_whitespace() {
                    index.entry(word.to_lowercase()).or_default().push(i);
                }
            }
            index
        });
    });
    group.finish();
}

fn bench_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("search/query");
    let corpus = create_corpus();
    let index = build_index(&corpus);

    group.bench_function("single_term", |b| {
        b.iter(|| {
            index.get("search").cloned().unwrap_or_default()
        });
    });

    group.bench_function("multi_term", |b| {
        b.iter(|| {
            let terms = ["rust", "accelerate", "engine"];
            let mut results: Vec<usize> = Vec::new();
            for term in terms {
                if let Some(docs) = index.get(term) {
                    results.extend(docs);
                }
            }
            results.sort();
            results.dedup();
            results
        });
    });

    group.finish();
}

fn bench_filter(c: &mut Criterion) {
    let mut group = c.benchmark_group("filter/evaluate");
    let corpus = create_corpus();

    group.bench_function("filter_docs", |b| {
        b.iter(|| {
            corpus.iter().filter(|doc| doc.len() > 100).count()
        });
    });

    group.finish();
}

fn bench_highlight(c: &mut Criterion) {
    let mut group = c.benchmark_group("highlight/snippet");
    let corpus = create_corpus();
    let query = "search";

    group.bench_function("highlight_matches", |b| {
        b.iter(|| {
            for doc in &corpus {
                let lower = doc.to_lowercase();
                if lower.contains(query) {
                    let _highlighted = doc.replace(query, &format!("**{}**", query));
                }
            }
        });
    });

    group.finish();
}

fn bench_typo(c: &mut Criterion) {
    let mut group = c.benchmark_group("typo/correct");

    group.bench_function("typo_correction", |b| {
        b.iter(|| {
            let _ = damerau_levenshtein_distance("accelarate", "accelerate");
        });
    });

    group.finish();
}

fn bench_vector(c: &mut Criterion) {
    let mut group = c.benchmark_group("vector/knn");
    group.sample_size(10);

    group.bench_function("cosine_similarity", |b| {
        let a: Vec<f64> = (0..384).map(|i| (i as f64).sin()).collect();
        let b_vec: Vec<f64> = (0..384).map(|i| (i as f64).cos()).collect();
        b.iter(|| {
            cosine_similarity(&a, &b_vec)
        });
    });

    group.finish();
}

fn bench_hybrid(c: &mut Criterion) {
    let mut group = c.benchmark_group("hybrid/rrf");
    group.sample_size(10);

    group.bench_function("reciprocal_rank_fusion", |b| {
        let keyword_results = [1, 2, 3, 4, 5];
        let semantic_results = [3, 4, 5, 6, 7];
        b.iter(|| {
            let mut scores: std::collections::HashMap<usize, f64> = std::collections::HashMap::new();
            let k = 60.0;
            for (rank, doc) in keyword_results.iter().enumerate() {
                *scores.entry(*doc).or_default() += 1.0 / (k + rank as f64);
            }
            for (rank, doc) in semantic_results.iter().enumerate() {
                *scores.entry(*doc).or_default() += 1.0 / (k + rank as f64);
            }
            let mut results: Vec<(usize, f64)> = scores.into_iter().collect();
            results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            results
        });
    });

    group.finish();
}

fn create_corpus() -> Vec<String> {
    let topics = [
        "rust programming language systems performance memory safety",
        "search engine indexing inverted index full text search",
        "machine learning neural network deep learning artificial intelligence",
        "database storage engine btree hash map data structure",
        "web server http request response api rest graphql",
        "cloud computing kubernetes docker container orchestration",
        "security authentication authorization encryption cryptography",
        "testing unit test integration test benchmark performance",
        "documentation readme getting started tutorial guide",
        "open source community contribution pull request merge",
    ];

    let mut corpus = Vec::new();
    for i in 0..1000 {
        let topic = topics[i % topics.len()];
        let words: Vec<&str> = topic.split_whitespace().collect();
        let doc_words: Vec<String> = (0..50)
            .map(|j| {
                let idx = (i + j) % words.len();
                words[idx].to_string()
            })
            .collect();
        corpus.push(doc_words.join(" "));
    }
    corpus
}

fn build_index(corpus: &[String]) -> std::collections::HashMap<String, Vec<usize>> {
    let mut index: std::collections::HashMap<String, Vec<usize>> = std::collections::HashMap::new();
    for (i, doc) in corpus.iter().enumerate() {
        for word in doc.split_whitespace() {
            index.entry(word.to_lowercase()).or_default().push(i);
        }
    }
    index
}

fn cosine_similarity(a: &[f64], b: &[f64]) -> f64 {
    let dot: f64 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    let norm_b: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

fn damerau_levenshtein_distance(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let len_a = a_chars.len();
    let len_b = b_chars.len();

    let mut dp = vec![vec![0usize; len_b + 1]; len_a + 1];

    for (i, row) in dp.iter_mut().enumerate().take(len_a + 1) {
        row[0] = i;
    }
    if let Some(first_row) = dp.first_mut() {
        for (j, cell) in first_row.iter_mut().enumerate() {
            *cell = j;
        }
    }

    for i in 1..=len_a {
        for j in 1..=len_b {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };
            dp[i][j] = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
        }
    }

    dp[len_a][len_b]
}

criterion_group!(
    name = benches;
    config = Criterion::default()
        .sample_size(50)
        .measurement_time(Duration::from_secs(5));
    targets =
        bench_tokenize,
        bench_index,
        bench_search,
        bench_filter,
        bench_highlight,
        bench_typo,
        bench_vector,
        bench_hybrid
);
criterion_main!(benches);
