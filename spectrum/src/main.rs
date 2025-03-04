use ndarray::prelude::*;
use linfa::traits::Fit;
use linfa_clustering::KMedoids;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fs::File;
use std::io::{BufReader, BufRead};

// Helper function to compress status information
fn compress_status(status: &str) -> String {
    let status = status.to_lowercase();
    if status.contains("forum") {
        "forum_question".into()
    } else if status.contains("resource") {
        "resource".into()
    } else {
        "blog".into()
    }
}

// Helper function to process URLs
fn compress_url(url: &str) -> String {
    url.split('/')
        .nth(3)
        .unwrap_or("")
        .replace("https://", "")
        .to_lowercase()
}

// Tokenization and cleaning of titles
fn tokenize_title(title: &str) -> Vec<String> {
    title
        .split_whitespace()
        .map(|word| {
            word.chars()
                .filter(|c| c.is_alphanumeric() || *c == '-')
                .collect::<String>()
                .to_lowercase()
        })
        .filter(|word| !word.is_empty() && word.len() > 2)
        .collect()
}

fn main() -> Result<(), Box<dyn Error>> {
    // 1. Load and parse data manually
    let file = File::open("Articles-Pageviews.txt")?;
    let reader = BufReader::new(file);
    
    // Read headers
    let headers: Vec<String> = reader
        .lines()
        .next()
        .unwrap()?
        .split('\t')
        .map(|s| s.to_string())
        .collect();
    
    // Prepare data structures
    let mut titles = Vec::new();
    let mut statuses = Vec::new();
    let mut pageviews = Vec::new();
    let mut urls = Vec::new();
    let mut authors = Vec::new();

    // Read data rows
    for line in reader.lines() {
        let line = line?;
        let parts: Vec<&str> = line.split('\t').collect();
        
        if parts.len() == headers.len() {
            titles.push(parts[headers.iter().position(|r| r == "Title").unwrap_or(0)].to_string());
            statuses.push(parts[headers.iter().position(|r| r == "Status").unwrap_or(1)].to_string());
            pageviews.push(parts[headers.iter().position(|r| r == "Page views").unwrap_or(2)].parse::<f64>().unwrap_or(0.0));
            urls.push(parts[headers.iter().position(|r| r == "URL").unwrap_or(3)].to_string());
            authors.push(parts[headers.iter().position(|r| r == "Author").unwrap_or(4)].to_string());
        }
    }

    // 2. Initialize data structures
    let mut hash_words_count = HashMap::new();
    let mut hash_pv = HashMap::new();
    let mut hash_authors_count = HashMap::new();
    let mut arr_categories = Vec::with_capacity(titles.len());
    let mut article_features = Vec::with_capacity(titles.len());

    // 3. Detrend page views
    let t1 = 0.8;
    let t2 = 0.11;
    let len = pageviews.len();
    let detrended_pv: Vec<f64> = pageviews
        .iter()
        .enumerate()
        .map(|(k, &pv)| {
            let boost = t1 * (k as f64 + t2 * len as f64).sqrt();
            pv * (1.0 + boost)
        })
        .collect();

    // 4. Process articles and build features
    for idx in 0..titles.len() {
        let author = &authors[idx];
        *hash_authors_count.entry(author).or_insert(0) += 1;

        let status = compress_status(&statuses[idx]);
        let url = compress_url(&urls[idx]);
        let category = if hash_authors_count[author] > 50 {
            format!("{}~{}~{}", status, url, author)
        } else {
            format!("{}~{}", status, url)
        };
        arr_categories.push(category.clone());

        // Process title tokens
        let tokens = tokenize_title(&titles[idx]);
        let pv = detrended_pv[idx];
        
        let mut features = HashMap::new();
        for token in &tokens {
            *hash_words_count.entry(token.clone()).or_insert(0) += 1;
            *hash_pv.entry(token.clone()).or_insert(0.0) += pv;
            features.insert(token.clone(), pv);
        }
        article_features.push(features);
    }

    // 5. Calculate relative page views and filter words
    let mut hash_pv_rel: Vec<(String, f64)> = hash_pv
        .into_iter()
        .map(|(word, total)| {
            let count = *hash_words_count.get(&word).unwrap_or(&1) as f64;
            (word, total / count)
        })
        .collect();
    hash_pv_rel.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    // 6. Create shortlist of significant words
    let mut seen = HashSet::new();
    let mut short_list: Vec<String> = hash_pv_rel
        .into_iter()
        .filter(|(word, score)| {
            !seen.iter().any(|w: &String| w.contains(word)) 
            && *score > 1000.0
            && hash_words_count.get(word).map_or(false, |&count| count > 5)
        })
        .map(|(word, _)| {
            seen.insert(word.clone());
            word
        })
        .collect();
    short_list.truncate(50);

    // 7. Build co-occurrence matrix
    let n_words = short_list.len();
    let mut co_occurrence = Array2::zeros((n_words, n_words));
    let word_index: HashMap<_, _> = short_list
        .iter()
        .enumerate()
        .map(|(i, w)| (w.as_str(), i))
        .collect();

    for features in &article_features {
        let present_words: Vec<usize> = features
            .keys()
            .filter_map(|word| word_index.get(word.as_str()))
            .copied()
            .collect();

        for &a in &present_words {
            for &b in &present_words {
                co_occurrence[[a, b]] += 1;
            }
        }
    }

    // 8. Convert to distance matrix
    let mut dist_matrix = Array2::zeros((n_words, n_words));
    for i in 0..n_words {
        for j in 0..n_words {
            let intersection = co_occurrence[[i, j]] as f64;
            let total = co_occurrence[[i, i]] + co_occurrence[[j, j]] - intersection;
            dist_matrix[[i, j]] = 1.0 - (intersection / total.max(1.0));
        }
    }

    // 9. Perform clustering
    let n_clusters = 20;
    let kmedoids = KMedoids::params(n_clusters)
        .max_iter(100)
        .fit(&dist_matrix);

    // 10. Output cluster groups
    let mut clusters = HashMap::new();
    for (word_idx, &label) in kmedoids.labels().iter().enumerate() {
        clusters.entry(label)
            .or_insert_with(Vec::new)
            .push(short_list[word_idx].as_str());
    }

    for (cluster_id, words) in clusters {
        println!("Cluster {}: {:?}", cluster_id, words);
    }

    Ok(())
}