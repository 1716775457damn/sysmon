use git2::{Repository, Sort};
use std::collections::HashMap;
use std::sync::mpsc::Sender;

// ─── Data structures ──────────────────────────────────────────────────────────

#[derive(Clone, Default)]
pub struct AuthorStats {
    pub name: String,
    pub commits: usize,
    pub additions: usize,
    pub deletions: usize,
    pub files_touched: usize,
}

#[derive(Clone, Default)]
pub struct FileStats {
    pub path: String,
    pub changes: usize,   // times modified
    pub additions: usize,
    pub deletions: usize,
    pub authors: usize,   // distinct authors
}

#[derive(Clone, Default)]
pub struct GitStats {
    pub repo_path: String,
    pub total_commits: usize,
    pub total_authors: usize,
    pub total_files: usize,
    pub first_commit: String,
    pub last_commit: String,
    pub authors: Vec<AuthorStats>,       // sorted by commits desc
    pub hot_files: Vec<FileStats>,       // sorted by changes desc
    /// heatmap[weekday 0-6][hour 0-23] = commit count
    pub heatmap: [[usize; 24]; 7],
    /// commits per day for the last 52 weeks (364 days)
    pub activity: Vec<usize>,
}

pub enum GitMsg {
    Progress { done: usize, total: usize },
    Done(Box<GitStats>),
    Error(String),
}

// ─── Analysis ─────────────────────────────────────────────────────────────────

pub fn analyze(repo_path: String, tx: Sender<GitMsg>) {
    std::thread::spawn(move || {
        match run_analysis(&repo_path, &tx) {
            Ok(stats) => { let _ = tx.send(GitMsg::Done(Box::new(stats))); }
            Err(e)    => { let _ = tx.send(GitMsg::Error(e.to_string())); }
        }
    });
}

fn run_analysis(repo_path: &str, tx: &Sender<GitMsg>) -> Result<GitStats, git2::Error> {
    let repo = Repository::open(repo_path)?;
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    revwalk.set_sorting(Sort::TIME)?;

    // Collect all oids first for progress reporting
    let oids: Vec<_> = revwalk.filter_map(|r| r.ok()).collect();
    let total = oids.len();

    let mut author_map: HashMap<String, AuthorStats> = HashMap::new();
    let mut file_map: HashMap<String, (usize, usize, usize, std::collections::HashSet<String>)> = HashMap::new();
    let mut heatmap = [[0usize; 24]; 7];
    let mut activity_map: HashMap<i64, usize> = HashMap::new(); // day_epoch -> count
    let mut first_ts = i64::MAX;
    let mut last_ts  = i64::MIN;
    let mut first_msg = String::new();
    let mut last_msg  = String::new();

    for (i, oid) in oids.iter().enumerate() {
        if i % 200 == 0 {
            let _ = tx.send(GitMsg::Progress { done: i, total });
        }

        let commit = match repo.find_commit(*oid) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let author = commit.author();
        let name = author.name().unwrap_or("Unknown").to_string();
        let ts = commit.time().seconds();

        // Time stats
        if ts < first_ts { first_ts = ts; first_msg = commit.summary().unwrap_or("").to_string(); }
        if ts > last_ts  { last_ts  = ts; last_msg  = commit.summary().unwrap_or("").to_string(); }

        // Heatmap: use local time approximation
        let secs_in_day = 86400i64;
        let hour = ((ts % secs_in_day + secs_in_day) % secs_in_day / 3600) as usize;
        // weekday: epoch day % 7, offset so Mon=0
        let day_epoch = (ts / secs_in_day + 4) % 7; // 1970-01-01 was Thursday=3
        let weekday = ((day_epoch + 3) % 7) as usize; // Mon=0
        if hour < 24 && weekday < 7 {
            heatmap[weekday][hour] += 1;
        }

        // Activity (day bucket)
        let day_key = ts / secs_in_day;
        *activity_map.entry(day_key).or_insert(0) += 1;

        // Author stats (without diff for speed on large repos)
        let entry = author_map.entry(name.clone()).or_insert_with(|| AuthorStats {
            name: name.clone(), ..Default::default()
        });
        entry.commits += 1;

        // Diff stats — compare to parent
        let diff = if commit.parent_count() > 0 {
            let parent = commit.parent(0)?;
            let old_tree = parent.tree()?;
            let new_tree = commit.tree()?;
            repo.diff_tree_to_tree(Some(&old_tree), Some(&new_tree), None)?
        } else {
            let new_tree = commit.tree()?;
            repo.diff_tree_to_tree(None, Some(&new_tree), None)?
        };

        let stats = diff.stats()?;
        entry.additions += stats.insertions();
        entry.deletions  += stats.deletions();

        // Per-file stats via delta iteration (single closure)
        let mut file_deltas: Vec<(String, usize, usize)> = Vec::new();
        diff.foreach(
            &mut |delta, _| {
                if let Some(path) = delta.new_file().path()
                    .or_else(|| delta.old_file().path())
                    .and_then(|p| p.to_str())
                {
                    file_deltas.push((path.to_string(), 0, 0));
                }
                true
            },
            None, None, None,
        ).ok();
        // Count hunk lines per file using stats per-file
        let _ = diff.foreach(
            &mut |_, _| true,
            None,
            Some(&mut |delta, hunk| {
                let path = delta.new_file().path()
                    .or_else(|| delta.old_file().path())
                    .and_then(|p| p.to_str())
                    .unwrap_or("");
                if let Some(fd) = file_deltas.iter_mut().find(|(p, _, _)| p == path) {
                    fd.1 += hunk.new_lines() as usize;
                    fd.2 += hunk.old_lines() as usize;
                }
                true
            }),
            None,
        ).ok();
        for (path, add, del) in file_deltas {
            let e = file_map.entry(path)
                .or_insert((0, 0, 0, std::collections::HashSet::new()));
            e.0 += 1;
            e.1 += add;
            e.2 += del;
            e.3.insert(name.clone());
        }
    }

    // Build author list
    let mut authors: Vec<AuthorStats> = author_map.into_values().collect();
    authors.sort_by(|a, b| b.commits.cmp(&a.commits));
    for a in &mut authors {
        // Count files touched per author (approximate via file_map)
        a.files_touched = file_map.values()
            .filter(|(_, _, _, auth)| auth.contains(&a.name))
            .count();
    }

    // Build hot files list
    let mut hot_files: Vec<FileStats> = file_map.into_iter().map(|(path, (changes, add, del, auth))| {
        FileStats { path, changes, additions: add, deletions: del, authors: auth.len() }
    }).collect();
    hot_files.sort_by(|a, b| b.changes.cmp(&a.changes));
    hot_files.truncate(100);

    // Build 52-week activity
    let now_day = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64 / 86400)
        .unwrap_or(0);
    let activity: Vec<usize> = (0..364)
        .map(|i| *activity_map.get(&(now_day - 363 + i)).unwrap_or(&0))
        .collect();

    let fmt_ts = |ts: i64| -> String {
        if ts == i64::MAX || ts == i64::MIN { return "-".to_string(); }
        let d = ts / 86400;
        // Simple date from epoch
        let days_since_epoch = d;
        // Use chrono-free approximation
        let year = 1970 + days_since_epoch / 365;
        let month = (days_since_epoch % 365) / 30 + 1;
        let day   = (days_since_epoch % 365) % 30 + 1;
        format!("{}-{:02}-{:02}", year, month, day)
    };

    Ok(GitStats {
        repo_path: repo_path.to_string(),
        total_commits: total,
        total_authors: authors.len(),
        total_files: hot_files.len(),
        first_commit: format!("{} {}", fmt_ts(first_ts), first_msg.chars().take(40).collect::<String>()),
        last_commit:  format!("{} {}", fmt_ts(last_ts),  last_msg.chars().take(40).collect::<String>()),
        authors,
        hot_files,
        heatmap,
        activity,
    })
}
